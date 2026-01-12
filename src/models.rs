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
    // TODO implement annotations
    pub annotations: Vec<String>,
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
pub struct Tag {
    pub id: Id,
    pub label: String,
    pub slug: String,
}
