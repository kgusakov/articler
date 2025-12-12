use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_with::{BoolFromInt, serde_as};
use url::Url;

#[serde_as]
#[derive(Serialize, sqlx::FromRow)]
pub struct Entry {
    id: i32,
    url: Url,
    hashed_url: Option<String>,
    given_url: Option<Url>,
    hashed_given_url: Option<String>,
    title: String,
    content: String,
    #[serde_as(as = "BoolFromInt")]
    is_archived: bool,
    archived_at: Option<DateTime<Utc>>,
    #[serde_as(as = "BoolFromInt")]
    is_starred: bool,
    starred_at: Option<DateTime<Utc>>,
    tags: Vec<Tag>,
    created_at: DateTime<Utc>,
    update_at: DateTime<Utc>,
    annotations: Option<Vec<Annotation>>,
    mimetype: Option<String>,
    language: Option<String>,
    reading_time: i32,
    domain_name: String,
    preview_picture: Option<String>,
    origin_url: Option<Url>,
    published_at: Option<DateTime<Utc>>,
    published_by: Option<String>,
    is_public: Option<bool>,
    uid: Option<String>,
}

#[derive(sqlx::FromRow, Serialize)]
pub struct Annotation {
    id: i32,
    annotator_schema_version: String,
    text: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    quote: String,
    ranges: Vec<Range>,
}

#[derive(sqlx::FromRow, Serialize)]
pub struct Range {
    start: String,
    end: String,
    #[serde(rename(serialize = "startOffset"))]
    start_offset: i64,
    #[serde(rename(serialize = "endOffset"))]
    end_offset: i64,
}

#[derive(sqlx::FromRow, Serialize)]
pub struct Tag {
    id: i32,
    label: String,
    slug: String,
}
