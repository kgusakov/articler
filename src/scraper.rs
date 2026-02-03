use chrono::DateTime;
use chrono::Utc;
use dateparser::parse;
use dom_smoothie::ReadabilityError;
use dom_smoothie::{Article, CandidateSelectMode, Config, Readability};
use reqwest::Client;
use reqwest::Proxy;
use reqwest::header;
use reqwest::header::USER_AGENT;
use std::ops::Deref;
use std::time::Duration;
use thiserror::Error;
use url::Url;

use crate::result::ArticlerResult;

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
                title: extract_title(url).to_string(),
                content_html: "".to_string(),
                image_url: None,
                mime_type,
                language: None,
                published_at: None,
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
            title = extract_title(url).to_string();
        }

        Ok(Document {
            title,
            content_html: article.content.deref().to_owned(),
            image_url,
            mime_type,
            language: article.lang,
            published_at,
        })
    }
}

fn extract_title(url: &Url) -> &str {
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
    pub image_url: Option<Url>,
    pub mime_type: Option<String>,
    pub language: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
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

    #[actix_web::test]
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
            title: "Test Title".to_string(),
            content_html:
                "<div id=\"readability-page-1\" class=\"page\"><p>Test Content</p>\n        </div>"
                    .into(),
            image_url: Some(Url::parse("http://example.com/main.jpg").unwrap()),
            mime_type: Some("text/html".to_string()),
            language: Some("en".to_string()),
            published_at: Some(
                NaiveDateTime::parse_from_str("2020-11-24T02:43:22+00:00", "%Y-%m-%dT%H:%M:%S%:z")
                    .unwrap()
                    .and_utc()
            )
        }, document);
        mock_server.verify().await
    }

    #[actix_web::test]
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
        mock_server.verify().await
    }

    #[actix_web::test]
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
        mock_server.verify().await
    }

    #[actix_web::test]
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
        mock_server.verify().await
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
}
