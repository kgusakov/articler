use chrono::DateTime;
use chrono::Utc;
use dateparser::parse;
use dom_smoothie::{Article, CandidateSelectMode, Config, Readability};
use reqwest::Client;
use reqwest::Proxy;
use reqwest::header;
use reqwest::header::USER_AGENT;
use std::ops::Deref;
use std::string::FromUtf8Error;
use thiserror::Error;
use url::Url;

const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Linux; Android 13; Pixel 6) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36";

#[derive(Error, Debug)]
pub enum ScraperError {
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    #[error(transparent)]
    ReadabilityError(#[from] dom_smoothie::ReadabilityError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    Utf8Error(#[from] FromUtf8Error),
    #[error(transparent)]
    UrlParseError(#[from] url::ParseError),
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

impl Scraper {
    pub fn new(proxy_scheme: Option<&str>) -> Result<Self, ScraperError> {
        let mut builder = Client::builder();

        if let Some(p) = proxy_scheme {
            builder = builder.proxy(Proxy::all(p)?);
        }

        Ok(Self {
            client: builder.build()?,
        })
    }

    pub async fn extract(&self, url: &Url) -> Result<Document, ScraperError> {
        let response = self
            .client
            .get(url.as_str())
            .header(USER_AGENT, USER_AGENT_VALUE)
            .send()
            .await?;

        let mime_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .map(|v| String::from_utf8_lossy(v.as_bytes()).to_string());

        let buf = response.bytes().await?;

        let cfg = Config {
            candidate_select_mode: CandidateSelectMode::DomSmoothie,
            ..Default::default()
        };

        let mut readability =
            Readability::new(String::from_utf8_lossy(&buf).into_owned(), None, Some(cfg))?;

        let article: Article = readability.parse()?;

        let image_url = match article.image {
            Some(u) => Some(Url::parse(&u)?),
            _ => None,
        };

        let published_at = match article.published_time {
            Some(t) => parse(&t).ok(),
            None => None,
        };

        Ok(Document {
            title: article.title,
            content_html: article.content.deref().to_owned(),
            image_url,
            mime_type,
            language: article.lang,
            published_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use url::Url;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use crate::scraper::{Document, Scraper};

    #[actix_web::test]
    async fn test() {
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
}
