use chrono::NaiveDateTime;
use url::Url;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

use article_scraper::{Document, Scraper, error::Error};

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

    assert_eq!(
        Document {
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
        },
        document
    );
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

    let url = Url::parse(format!("{}/test-article/slug-like-url-path", mock_server.uri()).as_str())
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
        .respond_with(ResponseTemplate::new(200).set_body_raw("data", "application/octet-stream"))
        .mount(&mock_server)
        .await;

    let url = Url::parse(format!("{}/test-article/file.zip", mock_server.uri()).as_str()).unwrap();

    let scraper = Scraper::new(None).unwrap();

    let document = scraper.extract(&url).await;

    let err = document.unwrap_err();
    assert!(matches!(
        &err,
        Error::MimeTypeNotSupported { mime_type, .. } if mime_type == "application/octet-stream"
    ));
    assert_eq!(
        err.to_string(),
        "Mime type is not supported: application/octet-stream"
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
        .respond_with(ResponseTemplate::new(200).set_body_raw("not a valid pdf", "application/pdf"))
        .mount(&mock_server)
        .await;

    let url = Url::parse(format!("{}/test-article/new.pdf", mock_server.uri()).as_str()).unwrap();

    let scraper = Scraper::new(None).unwrap();

    let document = scraper.extract_or_fallback(&url).await;

    assert_eq!("new", document.title);
    assert_eq!(Some("application/pdf".to_owned()), document.mime_type);
    assert_eq!("", document.content_html);
    mock_server.verify().await;
}
