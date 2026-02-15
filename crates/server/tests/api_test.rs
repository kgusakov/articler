use std::sync::Once;

use actix_http::{Request, StatusCode, header};
use actix_web::{
    Error,
    body::MessageBody,
    cookie::Key,
    dev::{Service, ServiceResponse},
    test::{self},
    web::{self},
};

use chrono::{DateTime, Utc};
use rstest::rstest;
use rstest_reuse::apply;
use serde_json::{Value, json};
use serde_json_assert::{assert_json_eq, assert_json_include};
use sqlx::SqlitePool;
// TODO is it appropriate way?
use db::repository::entries;
use helpers::hash_str;
use server::{
    app::{AppState, app, init_handlebars},
    scraper::Scraper,
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path},
};

static INIT: Once = Once::new();

fn init() {
    INIT.call_once(|| {
        env_logger::init_from_env(env_logger::Env::new().default_filter_or("trace"));
    });
}

async fn init_app(
    pool: SqlitePool,
) -> impl Service<Request, Response = ServiceResponse<impl MessageBody>, Error = Error> {
    init();

    let cookie_key = Key::from(&[0u8; 64]);

    test::init_service(app(
        web::Data::new(AppState::new(
            pool,
            Scraper::new(None).unwrap(),
            init_handlebars().unwrap(),
        )),
        cookie_key,
    ))
    .await
}

async fn auhorization_header(
    app: &impl Service<Request, Response = ServiceResponse<impl MessageBody>, Error = Error>,
) -> String {
    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_form([
            ("grant_type", "password"),
            ("username", "wallabag"),
            ("password", "wallabag"),
            ("client_id", "client_1"),
            ("client_secret", "secret_1"),
        ])
        .to_request();

    let resp = test::call_service(app, req).await;

    assert_eq!(resp.status(), 200);

    let body: Value = test::read_body_json(resp).await;

    let access_token = body
        .get("access_token")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();

    format!("Bearer {access_token}")
}

fn assert_json_date_between(
    before: &DateTime<Utc>,
    after: &DateTime<Utc>,
    date_json_field: &str,
    json: &Value,
) {
    if let Value::Object(m) = json {
        let date_str = m.get(date_json_field).unwrap().as_str().unwrap();
        let date = DateTime::parse_from_rfc3339(date_str)
            .unwrap()
            .with_timezone(&Utc);
        assert!(date.timestamp() >= before.timestamp() && date.timestamp() <= after.timestamp());
    } else {
        panic!("{} is expected, but not found", date_json_field);
    }
}

#[rstest_reuse::template]
#[rstest]
#[case::no_format("")]
#[case::json_format(".json")]
fn formats(#[case] f: &str) {}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn get_entries(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::default()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/entries{f}"))
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;

    let expected: Value = serde_json::from_str(include_str!("json/entries.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn entries_exists(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::default()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/entries/exists{f}"))
        .to_request();

    let resp: Value = test::call_and_read_body_json(&app, req).await;

    let expected: Value = serde_json::from_str(r#"{ "exists": false }"#).unwrap();

    assert_json_eq!(expected, resp);
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations")]
async fn get_version(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::default()
        .uri(&format!("/api/version{f}"))
        .to_request();

    let resp: Value = test::call_and_read_body_json(&app, req).await;

    assert_eq!("2.6.12", resp.as_str().unwrap());
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations")]
async fn options_version(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::default()
        .method(actix_http::Method::OPTIONS)
        .append_header((header::ACCESS_CONTROL_REQUEST_METHOD, "GET"))
        .uri(&format!("/api/version{f}"))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(StatusCode::OK, resp.status());

    let mut sorted_result: Vec<&str> = resp
        .headers()
        .get(header::ACCESS_CONTROL_ALLOW_METHODS)
        .unwrap()
        .to_str()
        .unwrap()
        .split(",")
        .map(|s| s.trim())
        .collect();

    sorted_result.sort();

    assert!(
        ["GET", "POST", "PATCH"]
            .iter()
            .all(|m| sorted_result.contains(m))
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn get_entries_ordered_by_updated_at(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::default()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries?sort=updated")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;

    let expected: Value =
        serde_json::from_str(include_str!("json/entries_ordered_updated_at.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn get_entries_with_pages(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::default()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries?page=2&perPage=1")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;

    let expected: Value = serde_json::from_str(include_str!("json/entries_paging.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn get_entries_page_out_of_range(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::default()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries?page=10&perPage=2")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        404,
        "Should return 404 when requested page exceeds total pages"
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn get_entries_archived(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::default()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries?archive=1")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;

    let expected: Value = serde_json::from_str(include_str!("json/archived_entries.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn get_entries_starred(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::default()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries?starred=1")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;

    let expected: Value = serde_json::from_str(include_str!("json/starred_entries.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn get_entries_public(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::default()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries?public=1")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;

    let expected: Value = serde_json::from_str(include_str!("json/public_entries.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn post_entries_form_data(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let payload = "url=https://example.com/article&archive=1&starred=1&tags=label 1,label 2&title=New title&content=New content&language=ru&published_at=2023-12-01T11:00:00Z&preview_picture=https://example.com/pic.jpg&authors=author1,author2&public=1&origin_url=https://example.com/origin/url";

    let req = test::TestRequest::post()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/entries{f}"))
        .set_payload(payload)
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .to_request();

    let before_call_time = Utc::now();
    let resp = test::call_and_read_body(&app, req).await;
    let after_call_time = Utc::now();

    let expected = serde_json::from_str::<Value>(include_str!("json/create_entry.json")).unwrap();

    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert!(result.get("id").unwrap().as_i64().unwrap() >= 0);
    assert!(matches!(result.get("uid").unwrap(), Value::String(s) if !s.is_empty()));

    assert_json_date_between(&before_call_time, &after_call_time, "created_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "updated_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "starred_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "archived_at", &result);

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn post_entries_json_data(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/entries{f}"))
        .set_json(json!({
            "url": "https://example.com/article",
            "archive": 1,
            "starred": 1,
            "tags": "label 1,label 2",
            "title": "New title",
            "content": "New content",
            "language": "ru",
            "published_at": "2023-12-01T11:00:00Z",
            "preview_picture": "https://example.com/pic.jpg",
            "authors": "author1,author2",
            "public": 1,
            "origin_url": "https://example.com/origin/url"
        }))
        .insert_header(("content-type", "application/json"))
        .to_request();

    let before_call_time = Utc::now();
    let resp = test::call_and_read_body(&app, req).await;
    let after_call_time = Utc::now();

    let expected = serde_json::from_str::<Value>(include_str!("json/create_entry.json")).unwrap();

    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert!(result.get("id").unwrap().as_i64().unwrap() >= 0);
    assert!(matches!(result.get("uid").unwrap(), Value::String(s) if !s.is_empty()));

    assert_json_date_between(&before_call_time, &after_call_time, "created_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "updated_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "starred_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "archived_at", &result);

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn post_entries_with_scraping_needed(pool: SqlitePool) {
    let app = init_app(pool).await;

    let mock_server = MockServer::start().await;
    let base_server_uri = mock_server.uri();

    let content = r#"
            <!DOCTYPE html><html lang="en"><head><title>Test Title</title><meta property="article:published_time" content="2020-11-24T02:43:22+00:00"><meta property="og:image" content="http://example.com/main.jpg"></head><body><p>Test Content</p></body></html>
        "#;

    Mock::given(method("GET"))
        .and(path("/test-article"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(content, "text/html"))
        .mount(&mock_server)
        .await;

    let url = format!("{base_server_uri}/test-article");

    let payload = format!(
        "url={url}&archive=1&starred=1&tags=label 1,label 2&public=1&authors=author1,author2&origin_url=https://example.com/origin/url"
    );

    let req = test::TestRequest::post()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries")
        .set_payload(payload)
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .to_request();

    let before_call_time = Utc::now();
    let resp = test::call_and_read_body(&app, req).await;
    let after_call_time = Utc::now();

    let expected =
        serde_json::from_str::<Value>(include_str!("json/create_entry_with_scraping.json"))
            .unwrap();

    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert!(result.get("id").unwrap().as_i64().unwrap() >= 0);

    assert_eq!(url, result.get("url").unwrap().as_str().unwrap());
    assert_eq!(
        hash_str(&url),
        result.get("hashed_url").unwrap().as_str().unwrap()
    );

    // TODO implement integration test with redirects
    assert_eq!(url, result.get("given_url").unwrap().as_str().unwrap());
    assert_eq!(
        hash_str(&url),
        result.get("hashed_given_url").unwrap().as_str().unwrap()
    );

    assert!(matches!(result.get("uid").unwrap(), Value::String(s) if !s.is_empty()));

    assert_json_date_between(&before_call_time, &after_call_time, "created_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "updated_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "starred_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "archived_at", &result);

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn post_entries_with_scraping_error(pool: SqlitePool) {
    let app = init_app(pool).await;

    let mock_server = MockServer::start().await;
    let base_server_uri = mock_server.uri();

    let content = r#"
            <!DOCTYPE html><html><body><!-- This HTML is intentionally broken and incomplete to trigger parsing error
        "#;

    Mock::given(method("GET"))
        .and(path("/test-article/parsing-error"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(content, "text/html"))
        .mount(&mock_server)
        .await;

    let url = format!("{base_server_uri}/test-article/parsing-error");

    let payload = format!("url={url}");

    let req = test::TestRequest::post()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries")
        .set_payload(payload)
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .to_request();

    let before_call_time = Utc::now();
    let resp = test::call_and_read_body(&app, req).await;
    let after_call_time = Utc::now();

    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert!(result.get("id").unwrap().as_i64().unwrap() >= 0);

    assert_eq!(url, result.get("url").unwrap().as_str().unwrap());
    assert_eq!(
        hash_str(&url),
        result.get("hashed_url").unwrap().as_str().unwrap()
    );

    assert_eq!(
        "parsing-error",
        result.get("title").unwrap().as_str().unwrap()
    );

    assert_eq!("", result.get("content").unwrap().as_str().unwrap());

    assert_eq!("", result.get("mimetype").unwrap().as_str().unwrap());

    assert!(result.get("published_at").unwrap().is_null());
    assert!(result.get("language").unwrap().is_null());
    assert!(result.get("preview_picture").unwrap().is_null());

    assert_json_date_between(&before_call_time, &after_call_time, "created_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "updated_at", &result);
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn delete_entry_expect_id(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool.clone()).await;

    let req = test::TestRequest::delete()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/entries/1{f}?expect=id"))
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;

    let expected: Value = serde_json::from_str(include_str!("json/delete_entry_id.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );

    let mut tx = pool.begin().await.unwrap();
    assert!(
        entries::find_by_id(&mut tx, 1, 1).await.unwrap().is_none(),
        "Entry should be deleted from database"
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn delete_entry_expect_full(pool: SqlitePool) {
    let app = init_app(pool.clone()).await;

    let req = test::TestRequest::delete()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries/2.json?expect=full")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;

    let expected: Value =
        serde_json::from_str(include_str!("json/delete_entry_full.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );

    let mut tx = pool.begin().await.unwrap();
    assert!(
        entries::find_by_id(&mut tx, 1, 2).await.unwrap().is_none(),
        "Entry should be deleted from database"
    );
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn delete_entry_not_found(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::delete()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/entries/999{f}?expect=id"))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        404,
        "Should return 404 for non-existent entry"
    );
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn patch_entry_with_some_fields(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::patch()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/entries/1{f}"))
        .set_form([
            ("title", "Updated Title"),
            ("content", "Updated Content"),
            ("language", "fr"),
        ])
        .to_request();

    let before_call_time = Utc::now();
    let resp = test::call_and_read_body(&app, req).await;
    let after_call_time = Utc::now();

    let expected =
        serde_json::from_str::<Value>(include_str!("json/patch_entry_basic.json")).unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_json_date_between(&before_call_time, &after_call_time, "updated_at", &result);

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn patch_entry_archive_and_star(pool: SqlitePool) {
    let app = init_app(pool).await;

    // Archive and star entry 1 (which is not archived and not starred)
    let req = test::TestRequest::patch()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries/1")
        .set_form([("archive", "1"), ("starred", "1")])
        .to_request();

    let before_call_time = Utc::now();
    let resp = test::call_and_read_body(&app, req).await;
    let after_call_time = Utc::now();

    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    // TODO if entry already has 1 in these fields - test tests nothing
    // TODO early the main design goal was to test the whole json
    assert_eq!(result.get("is_archived").unwrap().as_i64().unwrap(), 1);
    assert_eq!(result.get("is_starred").unwrap().as_i64().unwrap(), 1);

    assert_json_date_between(&before_call_time, &after_call_time, "updated_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "archived_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "starred_at", &result);
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn patch_entry_unarchive_and_unstar(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::patch()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries/4")
        .set_form([("archive", "0"), ("starred", "0")])
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    // TODO if entry already has 1 in these fields - test tests nothing
    assert_eq!(result.get("is_archived").unwrap().as_i64().unwrap(), 0);
    assert_eq!(result.get("is_starred").unwrap().as_i64().unwrap(), 0);
    assert!(result.get("archived_at").unwrap().is_null());
    assert!(result.get("starred_at").unwrap().is_null());
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn patch_entry_add_tags(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::patch()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries/1")
        .set_form([("tags", "newtag1,newtag2")])
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/patch_entry_add_tags.json")).unwrap();

    assert_json_include!(
        actual: serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap(),
        expected: expected,
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn patch_entry_replace_tags(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::patch()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries/2")
        .set_form([("tags", "label3,newtag")])
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/patch_entry_replace_tags.json")).unwrap();

    assert_json_include!(
        actual: serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap(),
        expected: expected,
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn patch_entry_remove_all_tags(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::patch()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries/2")
        .set_form([("tags", "")])
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert!(result.get("tags").unwrap().as_array().unwrap().is_empty());
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn patch_entry_not_found(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::patch()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/entries/999{f}"))
        .set_form([("title", "Updated")])
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        404,
        "Should return 404 for non-existent entry"
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn patch_entry_make_public(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::patch()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries/1")
        .set_form([("public", "1")])
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert!(result.get("is_public").unwrap().as_bool().unwrap());
    assert!(matches!(result.get("uid").unwrap(), Value::String(s) if !s.is_empty()));
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn get_tags_for_entry_with_tags(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::get()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/entries/2/tags{f}"))
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/get_tags_for_entry_with_tags.json"))
            .unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn get_tags_for_entry_without_tags(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::get()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries/1/tags")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected = serde_json::from_str::<Value>("[]").unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn get_tags_for_nonexistent_entry(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::get()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/entries/999/tags{f}"))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        404,
        "Should return 404 for non-existent entry"
    );
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn get_all_tags(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::get()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/tags{f}"))
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected = serde_json::from_str::<Value>(include_str!("json/get_all_tags.json")).unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users"))]
async fn get_all_tags_empty(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::get()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/tags")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    let tags = result.as_array().unwrap();
    assert_eq!(
        tags.len(),
        0,
        "Should return empty array when no tags exist"
    );
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn delete_tag_from_entry_success(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::delete()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/entries/2/tags/1{f}"))
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/delete_tag_from_entry_success.json"))
            .unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn delete_nonexistent_tag_from_entry(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::delete()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries/2/tags/999")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/delete_tag_from_entry_unchanged.json"))
            .unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn delete_tag_from_nonexistent_entry(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::delete()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/entries/999/tags/1{f}"))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        404,
        "Should return 404 for non-existent entry"
    );
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn delete_tag_by_label_success(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::delete()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/tag/label{f}?tag=label1"))
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/delete_tag_by_label_success.json"))
            .unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn delete_nonexistent_tag_by_label(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::delete()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/tag/label{f}?tag=nonexistent"))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 404, "Should return 404 for non-existent tag");
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn delete_tags_by_label_success(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::delete()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/tags/label.json?tags=label1,label2,label3")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/delete_tags_by_label_success.json"))
            .unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn delete_tags_by_label_partial(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::delete()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/tags/label.json?tags=label1,nonexistent,label2")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    let tags = result.as_array().unwrap();
    assert_eq!(tags.len(), 2, "Should return 2 deleted tags");

    let labels: Vec<String> = tags
        .iter()
        .map(|t| t["label"].as_str().unwrap().to_string())
        .collect();
    assert!(labels.contains(&"label1".to_string()));
    assert!(labels.contains(&"label2".to_string()));
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn delete_tags_by_label_nonexistent(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::delete()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/tags/label.json?tags=fake1,fake2,fake3")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    let tags = result.as_array().unwrap();
    assert_eq!(
        tags.len(),
        0,
        "Should return empty array when no tags deleted"
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn delete_tags_by_label_empty(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::delete()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/tags/label.json?tags=")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    let tags = result.as_array().unwrap();
    assert_eq!(
        tags.len(),
        0,
        "Should return empty array for empty tags parameter"
    );
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn delete_tag_by_id_success(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::delete()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/tags/1{f}"))
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/delete_tag_by_id_success.json")).unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn delete_tag_by_id_not_found(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::delete()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/tag/999{f}"))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 404, "Should return 404 for non-existent tag");
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn post_entry_tags_add(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/entries/1/tags{f}"))
        .set_form([("tags", "label3,label4")])
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/post_entry_tags_add.json")).unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn post_entry_tags_replace(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/entries/2/tags{f}"))
        .set_form([("tags", "label5,label6")])
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/post_entry_tags_replace.json")).unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn post_entry_tags_remove_all(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri("/api/entries/2/tags")
        .set_form([("tags", "")])
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    let tags = result["tags"].as_array().unwrap();
    assert_eq!(
        tags.len(),
        0,
        "Entry should have no tags after posting empty"
    );
}

#[apply(formats)]
#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn post_entry_tags_not_found(f: &str, #[ignore] pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .append_header((header::AUTHORIZATION, auhorization_header(&app).await))
        .uri(&format!("/api/entries/999/tags{f}"))
        .set_form([("tags", "label1,label2")])
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        404,
        "Should return 404 for non-existent entry"
    );
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn auth_wrong_bearer(pool: SqlitePool) {
    let app = init_app(pool).await;

    let access_token = "wrong_token";

    let req = test::TestRequest::default()
        .append_header((header::AUTHORIZATION, format!("Bearer {access_token}")))
        .uri("/api/entries")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 401);

    let expected: Value = serde_json::from_str(
        r#"{
            "error": "invalid_grant",
            "error_description": "The access token provided is invalid."
            }"#,
    )
    .unwrap();

    let body: Value = test::read_body_json(resp).await;

    assert_json_eq!(expected, body);
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn auth_no_bearer(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::default()
        .uri("/api/entries")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 401);

    let expected: Value = serde_json::from_str(
        r#"{
            "error": "access_denied",
            "error_description": "OAuth2 authentication required"
            }"#,
    )
    .unwrap();

    let body: Value = test::read_body_json(resp).await;

    assert_json_eq!(expected, body);
}

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries", "oauth"))]
async fn auth_success(pool: SqlitePool) {
    let app = init_app(pool).await;

    let access_token = {
        let password = "wallabag";
        let req = test::TestRequest::post()
            .uri("/oauth/v2/token")
            .set_form([
                ("grant_type", "password"),
                ("username", "wallabag"),
                ("password", password),
                ("client_id", "client_1"),
                ("client_secret", "secret_1"),
            ])
            .to_request();

        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);

        let body: Value = test::read_body_json(resp).await;

        body.get("access_token")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    };

    let req = test::TestRequest::default()
        .append_header((header::AUTHORIZATION, format!("Bearer {access_token}")))
        .uri("/api/entries")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 200);

    let expected: Value = serde_json::from_str(include_str!("json/entries.json")).unwrap();

    let body: Value = test::read_body_json(resp).await;

    assert_json_eq!(expected, body);
}
