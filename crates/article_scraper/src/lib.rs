mod html;
pub mod scraper;

use std::fmt::Display;

use chrono::{DateTime, Utc};
use icu_segmenter::{WordSegmenter, options::WordBreakInvariantOptions};
use result::ArticlerResult;
pub use scraper::*;
use types::ReadingTime;
use url::Url;

const AVERAGE_READING_SPEED: i32 = 230;

fn reading_time(text: &str) -> ArticlerResult<ReadingTime> {
    Ok(i32::try_from(count_words(text))? / AVERAGE_READING_SPEED)
}

fn count_words(text: &str) -> usize {
    let segmenter = WordSegmenter::new_auto(WordBreakInvariantOptions::default());

    segmenter
        .segment_str(text)
        .iter_with_word_type()
        .filter(|(_, word_type)| word_type.is_word_like())
        .count()
}

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

pub enum ArticleMimeType {
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
        };

        if s.starts_with("application/pdf") {
            return Some(ArticleMimeType::Pdf);
        }

        None
    }
}
