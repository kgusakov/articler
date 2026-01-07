use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_with::{BoolFromInt, serde_as};
use url::Url;

type Id = i64;

// TODO investigate the good default traits to derive
#[serde_as]
#[derive(Serialize)]
pub struct Entry {
    pub id: Id,
    pub url: Url,
    pub hashed_url: Option<String>,
    pub given_url: Option<Url>,
    pub hashed_given_url: Option<String>,
    pub title: String,
    pub content: String,
    #[serde_as(as = "BoolFromInt")]
    pub is_archived: bool,
    pub archived_at: Option<DateTime<Utc>>,
    #[serde_as(as = "BoolFromInt")]
    pub is_starred: bool,
    pub starred_at: Option<DateTime<Utc>>,
    pub tags: Vec<Tag>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub annotations: Vec<Annotation>,
    pub mimetype: Option<String>,
    pub language: Option<String>,
    pub reading_time: i32,
    pub domain_name: String,
    pub preview_picture: Option<Url>,
    pub origin_url: Option<Url>,
    pub published_at: Option<DateTime<Utc>>,
    pub published_by: Option<Vec<String>>,
    pub is_public: Option<bool>,
    pub uid: Option<String>,
}

#[derive(Serialize)]
pub struct Annotation {
    pub id: Id,
    pub annotator_schema_version: String,
    pub text: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub quote: String,
    pub ranges: Vec<Range>,
}

#[derive(Serialize)]
pub struct Range {
    pub id: Id,
    pub start: String,
    pub end: String,
    #[serde(rename(serialize = "startOffset"))]
    pub start_offset: i64,
    #[serde(rename(serialize = "endOffset"))]
    pub end_offset: i64,
}

#[derive(Serialize)]
pub struct Tag {
    pub id: Id,
    pub label: String,
    pub slug: String,
}
