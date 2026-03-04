use dateparser::parse;
use dom_smoothie::{CandidateSelectMode, Config, Readability};
use snafu::ResultExt;
use std::ops::Deref;
use url::Url;

use crate::{
    ArticleMimeType, Document,
    error::{ReadabilityInitSnafu, ReadabilityParseSnafu, Result},
    extract_title,
    helpers::reading_time,
};

pub struct HtmlExtractor {}

impl HtmlExtractor {
    pub fn extract(url: &Url, data: &str) -> Result<Document> {
        let cfg = Config {
            candidate_select_mode: CandidateSelectMode::DomSmoothie,
            ..Default::default()
        };

        let mut readability =
            Readability::new(data, Some(url.as_str()), Some(cfg)).context(ReadabilityInitSnafu)?;

        let article = readability.parse().context(ReadabilityParseSnafu)?;

        let image_url = match article.image {
            Some(u) => Url::parse(&u).ok(),
            _ => None,
        };

        let published_at = match article.published_time {
            Some(t) => parse(&t).ok(),
            None => None,
        };

        let mut title = article.title;

        if title.is_empty() {
            extract_title(url).clone_into(&mut title);
        }

        let content_text = article.text_content.deref().to_owned();
        let reading_time = reading_time(&content_text)?;

        Ok(Document {
            title,
            content_html: article.content.deref().to_owned(),
            content_text,
            image_url,
            mime_type: Some(ArticleMimeType::Html.to_string()),
            language: article.lang,
            published_at,
            reading_time,
        })
    }
}
