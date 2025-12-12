use crate::models::Entry;
use actix_web::{
    get,
    web::{Json, Path},
};
use serde::{Deserialize, Serialize};
use url::Url;

// TODO /api pref should be moved as a base prefix
#[get("/api/entries")]
pub async fn entries(request: Path<EntriesRequest>) -> Json<Entries> {
    todo!();
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
    total: i32,
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
