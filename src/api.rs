use std::{
    env, i32,
    sync::{Arc, LazyLock},
};

use crate::{
    models::Entry,
    storage::repository::{
        EntryRepository, SqliteEntryRepository, SqliteTagRepository, TagRepository,
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
use sqlx::{Pool, Sqlite};
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
        .find_all(i32::MAX, 0)
        .await
        .map_err(ErrorInternalServerError)?;

    let url = Url::parse("https://example.com").map_err(ErrorInternalServerError)?;

    Ok(web::Json(Entries {
        page: 1,
        limit: 30,
        pages: 1,
        total: entries.len(),
        embedded: Embedded { items: entries },
        links: Links {
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
struct Entries {
    page: i32,
    limit: i32,
    pages: i32,
    total: usize,
    embedded: Embedded,
    links: Links,
}

#[derive(Deserialize)]
struct EntriesRequest {
    archive: Option<i32>,
    starred: Option<i32>,
    // TODO: must be a enum of created, updated, archived
    sort: Option<String>,
    // TODO: must be a enum of asc, desc
    order: Option<String>,
    page: Option<i32>,
    #[serde(rename(serialize = "perPage"))]
    per_page: Option<i32>,
    // TODO: must be an array of comma separated strings
    tags: Option<String>,
    since: Option<i32>,
    public: Option<i32>,
    //TODO: must be an enum of metadata, full
    detail: Option<String>,
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
