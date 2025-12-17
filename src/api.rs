use crate::{
    models::Entry,
    storage::repository::{
        AllEntriesParams, EntryRepository, SqliteEntryRepository, SqliteTagRepository,
        TagRepository,
    },
};
use actix_web::{
    App, HttpServer,
    dev::Server,
    error::{self, ErrorInternalServerError},
    get,
    web::{self, Json, Path},
};
use serde::{Deserialize, Serialize};
use serde_with::BoolFromInt;
use serde_with::StringWithSeparator;
use serde_with::formats::CommaSeparator;
use serde_with::serde_as;
use sqlx::{Pool, Sqlite};
use std::sync::Arc;
use url::Url;

pub struct AppState {
    tag_repository: Arc<dyn TagRepository>,
    entry_repository: Arc<dyn EntryRepository>,
}

pub fn app_state_init(pool: Arc<Pool<Sqlite>>) -> AppState {
    let tag_repo = Arc::new(SqliteTagRepository::new(pool.clone()));

    AppState {
        tag_repository: tag_repo.clone(),
        entry_repository: Arc::new(SqliteEntryRepository::new(pool.clone(), tag_repo.clone())),
    }
}

pub fn http_server(port: u16, app_state: AppState) -> std::io::Result<Server> {
    let app_data = web::Data::new(app_state);

    Ok(HttpServer::new(move || {
        App::new()
            .app_data(app_data.clone())
            .service(web::scope("/").service(entries))
    })
    .bind(format!("0.0.0.0:{}", port))?
    .run())
}

// TODO /api pref should be moved as a base prefix
#[get("/api/entries")]
pub async fn entries(
    data: web::Data<AppState>,
    request: Path<EntriesRequest>,
) -> actix_web::Result<Json<Entries>> {
    // TODO implement all needed request filters and etc
    let entries = data
        .entry_repository
        .find_all(AllEntriesParams {
            ..Default::default()
        })
        .await
        .map_err(ErrorInternalServerError)?;

    let url = Url::parse("https://example.com").map_err(ErrorInternalServerError)?;

    todo!();

    // Ok(web::Json(Entries {
    //     page: 1,
    //     limit: 30,
    //     pages: 1,
    //     total: entries.len(),
    //     embedded: Embedded { items: entries },
    //     links: Links {
    //         _self: Link { href: url.clone() },
    //         first: Link { href: url.clone() },
    //         last: Link { href: url.clone() },
    //         next: Link { href: url.clone() },
    //     },
    // }))
}

#[derive(Serialize)]
struct Embedded {
    items: Vec<Entry>,
}

#[derive(Serialize)]
struct Entries {
    page: i32,
    limit: i32,
    pages: i32,
    total: usize,
    embedded: Embedded,
    links: Links,
}

#[derive(Deserialize, Debug, PartialEq)]
enum FindSortEnum {
    #[serde(rename(deserialize = "created"))]
    Created,
    #[serde(rename(deserialize = "updated"))]
    Updated,
    #[serde(rename(deserialize = "archived"))]
    Archived,
}

#[derive(Deserialize, Debug, PartialEq)]
enum FindSortOrder {
    #[serde(rename(deserialize = "asc"))]
    Asc,
    #[serde(rename(deserialize = "desc"))]
    Desc,
}

#[derive(Deserialize, Debug, PartialEq)]
enum Detail {
    #[serde(rename(deserialize = "metadata"))]
    Metadata,
    #[serde(rename(deserialize = "full"))]
    Full,
}

#[serde_as]
#[derive(Deserialize, Debug, PartialEq)]
struct EntriesRequest {
    #[serde_as(as = "Option<BoolFromInt>")]
    archive: Option<bool>,
    #[serde_as(as = "Option<BoolFromInt>")]
    starred: Option<bool>,
    sort: Option<FindSortEnum>,
    order: Option<FindSortOrder>,
    page: Option<i32>,
    #[serde(rename(deserialize = "perPage"))]
    per_page: Option<i32>,
    #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
    tags: Option<Vec<String>>,
    since: Option<u32>,
    #[serde_as(as = "Option<BoolFromInt>")]
    public: Option<bool>,
    detail: Option<Detail>,
    domain_name: Option<String>,
}

#[derive(Serialize)]
struct Links {
    #[serde(flatten)]
    _self: Link,
    #[serde(flatten)]
    first: Link,
    #[serde(flatten)]
    last: Link,
    #[serde(flatten)]
    next: Link,
}

#[derive(Serialize)]
struct Link {
    href: Url,
}

#[cfg(test)]
mod tests {
    use crate::api::{Detail, EntriesRequest, FindSortEnum, FindSortOrder};

    #[test]
    fn test() {
        assert_eq!(
            EntriesRequest {
                archive: Some(true),
                starred: Some(false),
                sort: Some(FindSortEnum::Created),
                order: Some(FindSortOrder::Asc),
                page: Some(0),
                per_page: Some(10),
                tags: Some(vec!["api".to_string(), "rest".to_string()]),
                since: Some(0),
                public: Some(true),
                detail: Some(Detail::Full),
                domain_name: Some("example.com".to_string())
            },
            serde_json::from_str::<EntriesRequest>(
                r#"{
                    "archive":1,
                    "starred":0,
                    "sort":"created",
                    "order":"asc",
                    "page":0,
                    "perPage":10,
                    "tags":"api,rest",
                    "since":0,
                    "public":1,
                    "detail":"full",
                    "domain_name":"example.com"
                    }"#,
            )
            .unwrap()
        );
    }
}
