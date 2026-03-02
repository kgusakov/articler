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
            return Err(ScraperError::MimeTypeNotSupported(mime_type.clone(), url.clone()).into());
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
    use url::Url;

    use crate::extract_title;

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
