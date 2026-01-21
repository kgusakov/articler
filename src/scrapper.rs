use dom_smoothie::{Article, CandidateSelectMode, Config, Readability};
use reqwest::Client;
use reqwest::Proxy;
use reqwest::header::USER_AGENT;
use std::string::FromUtf8Error;
use thiserror::Error;
use url::Url;

const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Linux; Android 13; Pixel 6) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36";

#[derive(Error, Debug)]
pub enum ScrapperError {
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    #[error(transparent)]
    ReadabilityError(#[from] dom_smoothie::ReadabilityError),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error(transparent)]
    Utf8Error(#[from] FromUtf8Error),
}

pub struct Scrapper {
    client: Client,
}

impl Scrapper {
    pub fn new(proxy_scheme: Option<String>) -> Result<Self, ScrapperError> {
        let mut builder = Client::builder();

        if let Some(p) = proxy_scheme {
            builder = builder.proxy(Proxy::all(p)?);
        }

        Ok(Self {
            client: builder.build()?,
        })
    }

    pub async fn extract(&self, url: &Url) -> Result<(Vec<u8>, Option<String>), ScrapperError> {
        let response = self
            .client
            .get(url.clone())
            .header(USER_AGENT, USER_AGENT_VALUE)
            .send()
            .await?;

        dbg!(&response);

        let buf = response.bytes().await?;

        let cfg = Config {
            candidate_select_mode: CandidateSelectMode::DomSmoothie,
            ..Default::default()
        };

        dbg!(String::from_utf8(buf.to_vec())?);
        let mut readability = Readability::new(String::from_utf8(buf.to_vec())?, None, Some(cfg))?;

        let article: Article = readability.parse()?;
        Ok((article.content.as_bytes().to_vec(), Some(article.title)))
    }
}

#[cfg(test)]
mod tests {
    use url::Url;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use crate::scrapper::Scrapper;

    #[actix_web::test]
    async fn test2() {
        let mock_server = MockServer::start().await;

        let content = r#"
            <!DOCTYPE html><html><title>Test Title</title><body><p>Test Content</p></body></html>
        "#;

        Mock::given(method("GET"))
            .and(path("/test-article"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(content, "text/html"))
            .mount(&mock_server)
            .await;

        let url = Url::parse(format!("{}/test-article", mock_server.uri()).as_str()).unwrap();

        let scrapper = Scrapper::new(None).unwrap();

        let (content, title) = scrapper.extract(&url).await.unwrap();

        let expected =
            "<div id=\"readability-page-1\" class=\"page\"><p>Test Content</p>\n        </div>";

        assert_eq!(expected, String::from_utf8_lossy(&content));
        assert_eq!("Test Title", title.unwrap());
        mock_server.verify().await
    }
}
