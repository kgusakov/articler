use chrono::DateTime;
use chrono::Utc;
use dateparser::parse;
use dom_smoothie::ReadabilityError;
use dom_smoothie::{Article, CandidateSelectMode, Config, Readability};
use icu_segmenter::WordSegmenter;
use icu_segmenter::options::WordBreakInvariantOptions;
use reqwest::Client;
use reqwest::Proxy;
use reqwest::header;
use reqwest::header::USER_AGENT;
use std::ops::Deref;
use std::time::Duration;
use thiserror::Error;
use types::ReadingTime;
use url::Url;

use result::ArticlerResult;

const AVERAGE_READING_SPEED: i32 = 230;
const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Linux; Android 13; Pixel 6) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36";

impl Scraper {
    pub fn new(proxy_scheme: Option<&str>) -> ArticlerResult<Self> {
        let mut builder = Client::builder();

        if let Some(p) = proxy_scheme {
            builder = builder.proxy(Proxy::all(p)?);
        }

        Ok(Self {
            client: builder.build()?,
        })
    }

    pub async fn extract(&self, url: &Url) -> ArticlerResult<Document> {
        let response = self
            .client
            .get(url.as_str())
            .header(USER_AGENT, USER_AGENT_VALUE)
            .timeout(Duration::from_secs(30))
            .send()
            .await?;

        let mime_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|v| String::from_utf8_lossy(v.as_bytes()).to_string());

        if let Some(m) = &mime_type
            && !m.contains("text/html")
        {
            return Ok(Document {
                title: extract_title(url).to_owned(),
                content_html: String::new(),
                content_text: String::new(),
                image_url: None,
                mime_type,
                language: None,
                published_at: None,
                reading_time: 0,
            });
        }

        let buf = response.bytes().await?;

        let cfg = Config {
            candidate_select_mode: CandidateSelectMode::DomSmoothie,
            ..Default::default()
        };

        let mut readability = Readability::new(
            String::from_utf8_lossy(&buf).into_owned(),
            Some(url.as_str()),
            Some(cfg),
        )?;

        let article: Article = readability
            .parse()
            .map_err(|e| ScraperError::ArticleTextParsingError(e, url.clone()))?;

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
            extract_title(url).to_owned().clone_into(&mut title);
        }

        let content_text = article.text_content.deref().to_owned();
        // If i32 overflows - maybe you should read this article LATER
        let reading_time = count_words(&content_text) as i32 / AVERAGE_READING_SPEED;

        Ok(Document {
            title,
            content_html: article.content.deref().to_owned(),
            content_text,
            image_url,
            mime_type,
            language: article.lang,
            published_at,
            reading_time,
        })
    }
}

#[must_use]
pub fn extract_title(url: &Url) -> &str {
    if let Some(mut segments) = url.path_segments()
        && let Some(last) = segments.next_back()
        && !last.is_empty()
    {
        return last;
    }

    if let Some(domain) = url.domain() {
        return domain;
    }

    if let Some(host) = url.host_str() {
        return host;
    }

    url.as_str()
}

fn count_words(text: &str) -> usize {
    let segmenter = WordSegmenter::new_auto(WordBreakInvariantOptions::default());

    segmenter
        .segment_str(text)
        .iter_with_word_type()
        .filter(|(_, word_type)| word_type.is_word_like())
        .count()
}

#[derive(Error, Debug)]
enum ScraperError {
    #[error("Can't receive readable text from url {1}: {0:?}")]
    ArticleTextParsingError(ReadabilityError, Url),
}

pub struct Scraper {
    client: Client,
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

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use url::Url;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use crate::scraper::{Document, Scraper, extract_title};

    #[tokio::test]
    async fn test_success() {
        let mock_server = MockServer::start().await;

        let content = r#"
            <!DOCTYPE html><html lang="en"><head><title>Test Title</title><meta property="article:published_time" content="2020-11-24T02:43:22+00:00"><meta property="og:image" content="http://example.com/main.jpg"></head><body><p>Test Content</p></body></html>
        "#;

        Mock::given(method("GET"))
            .and(path("/test-article"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(content, "text/html"))
            .mount(&mock_server)
            .await;

        let url = Url::parse(format!("{}/test-article", mock_server.uri()).as_str()).unwrap();

        let scraper = Scraper::new(None).unwrap();

        let document = scraper.extract(&url).await.unwrap();

        assert_eq!(Document {
            title: "Test Title".to_owned(),
            content_html:
                "<div id=\"readability-page-1\" class=\"page\"><p>Test Content</p>\n        </div>"
                    .into(),
            content_text: "Test Content\n        ".into(),
            image_url: Some(Url::parse("http://example.com/main.jpg").unwrap()),
            mime_type: Some("text/html".to_owned()),
            language: Some("en".to_owned()),
            published_at: Some(
                NaiveDateTime::parse_from_str("2020-11-24T02:43:22+00:00", "%Y-%m-%dT%H:%M:%S%:z")
                    .unwrap()
                    .and_utc()
            ),
            reading_time: 0
        }, document);
        mock_server.verify().await;
    }

    #[tokio::test]
    async fn test_reading_time() {
        let mock_server = MockServer::start().await;

        let content = include_str!("../test_articles/joe_pass.html");

        Mock::given(method("GET"))
            .and(path("/test-article"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(content, "text/html"))
            .mount(&mock_server)
            .await;

        let url = Url::parse(format!("{}/test-article", mock_server.uri()).as_str()).unwrap();

        let scraper = Scraper::new(None).unwrap();

        let document = scraper.extract(&url).await.unwrap();

        insta::assert_snapshot!(document.title, @"Was Joe Pass a “Genius” of Jazz Guitar?");

        insta::assert_snapshot!(document.content_html);

        insta::assert_snapshot!(document.content_text);

        insta::assert_snapshot!(document.reading_time, @"5");

        mock_server.verify().await;
    }

    #[tokio::test]
    async fn test_relative_image() {
        let mock_server = MockServer::start().await;

        let content = r#"
            <!DOCTYPE html><html lang="en"><head><title>Test Title</title><meta property="article:published_time" content="2020-11-24T02:43:22+00:00"><meta property="og:image" content="/upload/main.jpg"></head><body><p>Test Content</p></body></html>
        "#;

        Mock::given(method("GET"))
            .and(path("/test-article"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(content, "text/html"))
            .mount(&mock_server)
            .await;

        let url = Url::parse(format!("{}/test-article", mock_server.uri()).as_str()).unwrap();

        let scraper = Scraper::new(None).unwrap();

        let document = scraper.extract(&url).await.unwrap();

        assert!(document.image_url.is_none());
        mock_server.verify().await;
    }

    #[tokio::test]
    async fn test_empty_title() {
        let mock_server = MockServer::start().await;

        let content = r#"
            <!DOCTYPE html><html lang="en"><head><meta property="article:published_time" content="2020-11-24T02:43:22+00:00"><meta property="og:image" content="/upload/main.jpg"></head><body><p>Test Content</p></body></html>
        "#;

        Mock::given(method("GET"))
            .and(path("/test-article/slug-like-url-path"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(content, "text/html"))
            .mount(&mock_server)
            .await;

        let url =
            Url::parse(format!("{}/test-article/slug-like-url-path", mock_server.uri()).as_str())
                .unwrap();

        let scraper = Scraper::new(None).unwrap();

        let document = scraper.extract(&url).await.unwrap();

        assert_eq!("slug-like-url-path", document.title);
        mock_server.verify().await;
    }

    #[tokio::test]
    async fn test_non_html_mime_type() {
        let mock_server = MockServer::start().await;

        let content = "no valid content";

        Mock::given(method("GET"))
            .and(path("/test-article/new.pdf"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(content, "application/pdf"))
            .mount(&mock_server)
            .await;

        let url =
            Url::parse(format!("{}/test-article/new.pdf", mock_server.uri()).as_str()).unwrap();

        let scraper = Scraper::new(None).unwrap();

        let document = scraper.extract(&url).await.unwrap();

        assert_eq!("new.pdf", document.title);
        assert_eq!("", document.content_html);
        mock_server.verify().await;
    }

    #[test]
    fn test_extract_title() {
        assert_eq!(
            "some-text",
            extract_title(&Url::parse("http://site.com/some-text").unwrap())
        );

        assert_eq!(
            "site.com",
            extract_title(&Url::parse("http://site.com").unwrap())
        );

        assert_eq!(
            "127.0.0.1",
            extract_title(&Url::parse("http://127.0.0.1").unwrap())
        );
    }

    #[test]
    fn test_count_words_english() {
        let text = "Hello world. This is a test sentence.";
        assert_eq!(7, super::count_words(text));
    }

    #[test]
    fn test_count_words_german() {
        let text = "Das ist ein Testartikel. Er enthält mehrere Sätze.";
        assert_eq!(8, super::count_words(text));
    }

    #[test]
    fn test_count_words_russian() {
        let text = "Это тестовая статья. Она содержит несколько предложений.";
        assert_eq!(7, super::count_words(text));
    }

    #[test]
    fn test_count_words_chinese() {
        let text = "这是一篇测试文章。它包含多个句子。";
        assert_eq!(7, super::count_words(text));
    }

    #[test]
    fn test_count_words_korean() {
        let text = "이것은 테스트 기사입니다. 여러 문장이 포함되어 있습니다.";
        assert_eq!(7, super::count_words(text));
    }
}
