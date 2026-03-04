pub mod error;
mod helpers;
mod html;
mod pdf;
pub mod scraper;

use std::fmt::Display;

use chrono::{DateTime, Utc};
pub use scraper::*;
use types::ReadingTime;
use url::Url;

#[derive(Debug, PartialEq)]
pub struct Document {
    pub title: String,
    pub content_html: String,
    pub content_text: String,
    pub image_url: Option<Url>,
    pub mime_type: Option<String>,
    pub language: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub reading_time: ReadingTime,
}

enum ArticleMimeType {
    Pdf,
    Html,
}

impl Display for ArticleMimeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArticleMimeType::Pdf => write!(f, "application/pdf"),
            ArticleMimeType::Html => write!(f, "text/html"),
        }
    }
}

impl ArticleMimeType {
    fn from(s: &str) -> Option<ArticleMimeType> {
        if s.starts_with("text/html") {
            return Some(ArticleMimeType::Html);
        }

        if s.starts_with("application/pdf") {
            return Some(ArticleMimeType::Pdf);
        }

        None
    }
}
