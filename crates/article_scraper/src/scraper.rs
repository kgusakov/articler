use log::error;
use reqwest::Client;
use reqwest::Proxy;
use reqwest::header;
use reqwest::header::USER_AGENT;
use std::time::Duration;
use thiserror::Error;
use url::Url;

use result::ArticlerResult;

use crate::ArticleMimeType;
use crate::Document;
use crate::html::HtmlExtractor;
use crate::pdf::PdfExtractor;

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
            .map_or(ArticleMimeType::Html.to_string(), |v| {
                String::from_utf8_lossy(v.as_bytes()).to_string()
            });

        let Some(mime_type) = ArticleMimeType::from(&mime_type) else {
            return Err(
                ScraperError::MimeTypeNotSupported(mime_type.clone(), url.clone()).into(),
            );
        };

        // TODO need to rethink this code pattern
        match mime_type {
            ArticleMimeType::Html => HtmlExtractor::extract(url, &response.text().await?),

            ArticleMimeType::Pdf => Ok(PdfExtractor::extract(url, &response.bytes().await?)),
        }
    }

    pub async fn extract_or_fallback(&self, url: &Url) -> Document {
        match self.extract(url).await {
            Ok(document) => document,
            Err(err) => {
                error!("Error while parsing url {url}: {err:?}");

                Document {
                    title: extract_title(url).to_owned(),
                    content_html: String::new(),
                    content_text: String::new(),
                    image_url: None,
                    mime_type: None,
                    language: None,
                    published_at: None,
                    reading_time: 0,
                }
            }
        }
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

#[derive(Error, Debug)]
enum ScraperError {
    #[error("Mime type {1} is not supported {0:?}")]
    MimeTypeNotSupported(String, Url),
}

pub struct Scraper {
    client: Client,
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
    async fn test_unsupported_mime_type() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/test-article/file.zip"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw("data", "application/octet-stream"),
            )
            .mount(&mock_server)
            .await;

        let url =
            Url::parse(format!("{}/test-article/file.zip", mock_server.uri()).as_str()).unwrap();

        let scraper = Scraper::new(None).unwrap();

        let document = scraper.extract(&url).await;

        assert!(document.is_err());
        assert_eq!(
            format!("{}", document.err().unwrap().source),
            format!(r#"Mime type {url} is not supported "application/octet-stream""#)
        );

        mock_server.verify().await;
    }

    #[tokio::test]
    async fn test_pdf_extraction() {
        let mock_server = MockServer::start().await;

        let pdf_bytes = include_bytes!("../test_articles/2310.11703v2.pdf");

        Mock::given(method("GET"))
            .and(path("/papers/2310.11703v2.pdf"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw(pdf_bytes.as_slice(), "application/pdf"),
            )
            .mount(&mock_server)
            .await;

        let url =
            Url::parse(format!("{}/papers/2310.11703v2.pdf", mock_server.uri()).as_str()).unwrap();

        let scraper = Scraper::new(None).unwrap();

        let document = scraper.extract(&url).await.unwrap();

        insta::assert_snapshot!(document.title, @"A Comprehensive Survey on Vector Database: Storage and Retrieval Technique, Challenge");
        assert_eq!("", document.content_html);
        insta::assert_snapshot!(document.content_text);
        assert_eq!(Some("application/pdf".to_owned()), document.mime_type);
        insta::assert_snapshot!(document.reading_time, @"6");

        mock_server.verify().await;
    }

    #[tokio::test]
    async fn test_pdf_fallback_on_invalid_data() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/test-article/new.pdf"))
            .respond_with(
                ResponseTemplate::new(200).set_body_raw("not a valid pdf", "application/pdf"),
            )
            .mount(&mock_server)
            .await;

        let url =
            Url::parse(format!("{}/test-article/new.pdf", mock_server.uri()).as_str()).unwrap();

        let scraper = Scraper::new(None).unwrap();

        let document = scraper.extract_or_fallback(&url).await;

        assert_eq!("new", document.title);
        assert_eq!(Some("application/pdf".to_owned()), document.mime_type);
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
}
