use crate::{
    models::{Entry, Tag},
    storage::repository::{
        AllEntriesParams, EntryRepository, EntryRow, SqliteEntryRepository, SqliteTagRepository,
        TagRepository, TagRow,
    },
};
use actix_web::{
    App, HttpServer,
    dev::Server,
    error::{self, ErrorInternalServerError},
    get,
    web::{self, Json, Path},
};
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::BoolFromInt;
use serde_with::StringWithSeparator;
use serde_with::formats::CommaSeparator;
use serde_with::serde_as;
use sqlx::{Pool, Sqlite};
use std::{error::Error, str::FromStr, sync::Arc};
use url::{ParseError, Url};

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

    let mut ents = vec![];

    for (e, tags) in entries {
        let mapped_tags: Vec<Tag> = tags.into_iter().map(|tr| tr.into()).collect();
        ents.push(Entry::try_from((e, mapped_tags)).map_err(ErrorInternalServerError)?);
    }

    let url = Url::from_str("http://example.com").unwrap();

    Ok(web::Json(Entries {
        page: 1,
        limit: 30,
        pages: 1,
        total: ents.len(),
        embedded: Embedded { items: ents },
        _links: Links {
            _self: Link { href: url.clone() },
            first: Link { href: url.clone() },
            last: Link { href: url.clone() },
            next: Link { href: url.clone() },
        },
    }))
}

#[derive(Serialize)]
struct Embedded {
    items: Vec<Entry>,
}

#[derive(Serialize)]
pub struct Entries {
    page: i32,
    limit: i32,
    pages: i32,
    total: usize,
    embedded: Embedded,
    _links: Links,
}

fn try_parse_url(s: Option<String>) -> Result<Option<Url>, ParseError> {
    s.map(|u| Url::parse(&u)).transpose()
}

fn try_parse_timestamp_opt(s: Option<i64>) -> anyhow::Result<Option<DateTime<Utc>>> {
    match s {
        Some(t) => match DateTime::from_timestamp_secs(t) {
            Some(r) => Ok(Some(r)),
            None => Err(anyhow!("Can't parse timestamp")),
        },
        None => Ok(None),
    }
}

fn try_parse_timestamp(s: i64) -> anyhow::Result<DateTime<Utc>> {
    match DateTime::from_timestamp_secs(s) {
        Some(r) => Ok(r),
        None => Err(anyhow!("Can't parse timestamp")),
    }
}

impl From<TagRow> for Tag {
    fn from(value: TagRow) -> Self {
        Self {
            id: value.id,
            label: value.label,
            slug: value.slug,
        }
    }
}

impl TryFrom<(EntryRow, Vec<Tag>)> for Entry {
    type Error = anyhow::Error;

    fn try_from((e, tags): (EntryRow, Vec<Tag>)) -> Result<Self, Self::Error> {
        Ok(Entry {
            id: e.id,
            url: Url::parse(&e.url)?,
            hashed_url: e.hashed_url,
            given_url: try_parse_url(e.given_url)?,
            hashed_given_url: e.hashed_given_url,
            title: e.title,
            content: e.content,
            is_archived: e.is_archived,
            archived_at: try_parse_timestamp_opt(e.archived_at)?,
            is_starred: e.is_starred,
            starred_at: try_parse_timestamp_opt(e.starred_at)?,
            tags: tags,
            created_at: try_parse_timestamp(e.created_at)?,
            updated_at: try_parse_timestamp(e.updated_at)?,
            annotations: None,
            mimetype: e.mimetype,
            language: e.language,
            reading_time: e.reading_time,
            domain_name: e.domain_name,
            preview_picture: try_parse_url(e.preview_picture)?,
            origin_url: try_parse_url(e.origin_url)?,
            published_at: try_parse_timestamp_opt(e.published_at)?,
            published_by: e.published_by,
            is_public: e.is_public,
            uid: e.uid,
        })
    }
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
pub struct EntriesRequest {
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
    #[serde(rename(serialize = "self"))]
    _self: Link,
    first: Link,
    last: Link,
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
