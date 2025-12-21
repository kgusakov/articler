use crate::{
    models::{Entry, Tag},
    storage::repository::{
        self, AllEntriesParams, EntryRepository, EntryRow, SortColumn, SortOrder,
        SqliteEntryRepository, SqliteTagRepository, TagRepository, TagRow,
    },
};
use actix_web::{
    App, HttpServer,
    dev::Server,
    error::ErrorInternalServerError,
    get,
    mime::Params,
    web::{self, Json, Path, Query},
};
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::BoolFromInt;
use serde_with::StringWithSeparator;
use serde_with::formats::CommaSeparator;
use serde_with::serde_as;
use sqlx::{Pool, Sqlite};
use std::{str::FromStr, sync::Arc};
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
    request: Query<EntriesRequest>,
) -> actix_web::Result<Json<Entries>> {
    let params = AllEntriesParams {
        archive: request.archive,
        starred: request.starred,
        sort: Some(request.sort.clone().into()),
        order: Some(request.order.clone().into()),
        page: Some(request.page),
        per_page: Some(request.per_page),
        tags: request.tags.clone(),
        since: Some(request.since),
        public: request.public,
        detail: Some(request.detail.clone().into()),
        domain_name: request.domain_name.clone(),
    };
    // TODO implement all needed request filters and etc
    let entries = data
        .entry_repository
        // TODO remove clones
        .find_all(&params)
        .await
        .map_err(ErrorInternalServerError)?;

    let mut ents = vec![];

    for (e, tags) in entries {
        let mapped_tags: Vec<Tag> = tags.into_iter().map(|tr| tr.into()).collect();
        ents.push(Entry::try_from((e, mapped_tags)).map_err(ErrorInternalServerError)?);
    }

    let url = Url::from_str("http://example.com").unwrap();

    let count_without_paging = data
        .entry_repository
        .count(&params)
        .await
        .map_err(ErrorInternalServerError)?;

    Ok(web::Json(Entries {
        page: request.page,
        limit: request.per_page,
        pages: (count_without_paging as f64 / request.per_page as f64).ceil() as i64,
        total: count_without_paging as i64,
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
    page: i64,
    limit: i64,
    pages: i64,
    total: i64,
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

#[derive(Deserialize, Debug, PartialEq, Clone)]
enum FindSortEnum {
    #[serde(rename(deserialize = "created"))]
    Created,
    #[serde(rename(deserialize = "updated"))]
    Updated,
    #[serde(rename(deserialize = "archived"))]
    Archived,
}

impl Default for FindSortEnum {
    fn default() -> Self {
        FindSortEnum::Created
    }
}

impl Into<SortColumn> for FindSortEnum {
    fn into(self) -> SortColumn {
        match self {
            FindSortEnum::Created => SortColumn::Created,
            FindSortEnum::Updated => SortColumn::Updated,
            FindSortEnum::Archived => SortColumn::Archived,
        }
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
enum FindSortOrder {
    #[serde(rename(deserialize = "asc"))]
    Asc,
    #[serde(rename(deserialize = "desc"))]
    Desc,
}

impl Default for FindSortOrder {
    fn default() -> Self {
        FindSortOrder::Desc
    }
}

impl Into<SortOrder> for FindSortOrder {
    fn into(self) -> SortOrder {
        match self {
            FindSortOrder::Asc => SortOrder::Asc,
            FindSortOrder::Desc => SortOrder::Desc,
        }
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
enum Detail {
    #[serde(rename(deserialize = "metadata"))]
    Metadata,
    #[serde(rename(deserialize = "full"))]
    Full,
}

impl Default for Detail {
    fn default() -> Self {
        Detail::Full
    }
}

impl Into<repository::Detail> for Detail {
    fn into(self) -> repository::Detail {
        match self {
            Detail::Full => repository::Detail::Full,
            Detail::Metadata => repository::Detail::Metadata,
        }
    }
}

fn default_page() -> i64 {
    1
}

fn default_per_page() -> i64 {
    30
}

#[serde_as]
#[derive(Deserialize, Debug, PartialEq)]
pub struct EntriesRequest {
    #[serde_as(as = "Option<BoolFromInt>")]
    archive: Option<bool>,
    #[serde_as(as = "Option<BoolFromInt>")]
    starred: Option<bool>,
    #[serde(default)]
    sort: FindSortEnum,
    #[serde(default)]
    order: FindSortOrder,
    #[serde(default = "default_page")]
    page: i64,
    #[serde(rename(deserialize = "perPage"))]
    #[serde(default = "default_per_page")]
    per_page: i64,
    #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
    tags: Option<Vec<String>>,
    #[serde(default)]
    since: i64,
    #[serde_as(as = "Option<BoolFromInt>")]
    public: Option<bool>,
    #[serde(default)]
    detail: Detail,
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
                sort: FindSortEnum::Created,
                order: FindSortOrder::Asc,
                page: 0,
                per_page: 10,
                tags: Some(vec!["api".to_string(), "rest".to_string()]),
                since: 0,
                public: Some(true),
                detail: Detail::Full,
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
