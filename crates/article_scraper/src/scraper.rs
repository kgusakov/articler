use log::error;
use reqwest::Client;
use reqwest::Proxy;
use reqwest::header;
use reqwest::header::USER_AGENT;
use snafu::ResultExt;
use std::time::Duration;
use types::{ArticleUrl, Title};
use url::Url;

use crate::ArticleMimeType;
use crate::Document;
use crate::error::HttpClientInitSnafu;
use crate::error::HttpRequestSnafu;
use crate::error::HttpResponseParsingSnafu;
use crate::error::MimeTypeNotSupportedSnafu;
use crate::error::Result;
use crate::html::HtmlExtractor;
use crate::pdf::PdfExtractor;

const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Linux; Android 13; Pixel 6) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36";

impl Scraper {
    pub fn new(proxy_scheme: Option<&str>) -> Result<Self> {
        let mut builder = Client::builder();

        if let Some(p) = proxy_scheme {
            builder = builder.proxy(Proxy::all(p).context(HttpClientInitSnafu)?);
        }

        Ok(Self {
            client: builder.build().context(HttpClientInitSnafu)?,
        })
    }

    pub async fn extract(&self, url: &ArticleUrl) -> Result<Document> {
        let response = self
            .client
            .get(url.as_str())
            .header(USER_AGENT, USER_AGENT_VALUE)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .context(HttpRequestSnafu)?;

        let mime_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .map_or(ArticleMimeType::Html.to_string(), |v| {
                String::from_utf8_lossy(v.as_bytes()).to_string()
            });

        let Some(mime_type) = ArticleMimeType::from(&mime_type) else {
            return MimeTypeNotSupportedSnafu { mime_type }.fail();
        };

        // TODO need to rethink this code pattern
        match mime_type {
            ArticleMimeType::Html => HtmlExtractor::extract(
                url,
                &response.text().await.context(HttpResponseParsingSnafu)?,
            ),

            ArticleMimeType::Pdf => Ok(PdfExtractor::extract(
                url,
                &response.bytes().await.context(HttpResponseParsingSnafu)?,
            )),
        }
    }

    pub async fn extract_or_fallback(&self, url: &ArticleUrl) -> Document {
        match self.extract(url).await {
            Ok(document) => document,
            Err(err) => {
                error!("Error while parsing url {url}: {err:?}");

                Document {
                    title: extract_title(url),
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
pub fn extract_title(url: &Url) -> Title {
    let s = url
        .path_segments()
        .and_then(|mut seg| seg.next_back().filter(|s| !s.is_empty()))
        .or_else(|| url.domain())
        .or_else(|| url.host_str())
        .unwrap_or_else(|| url.as_str());
    Title::try_from(s.to_owned()).unwrap_or_default()
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
            &*extract_title(&Url::parse("http://site.com/some-text").unwrap())
        );

        assert_eq!(
            "site.com",
            &*extract_title(&Url::parse("http://site.com").unwrap())
        );

        assert_eq!(
            "127.0.0.1",
            &*extract_title(&Url::parse("http://127.0.0.1").unwrap())
        );
    }
}
