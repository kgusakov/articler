use std::sync::Once;

use actix_http::Request;
use actix_web::{
    Error,
    body::MessageBody,
    cookie::Key,
    dev::{Service, ServiceResponse},
    test,
    web::{self},
};
use serde_json::Value;
use sqlx::SqlitePool;
use wallabag_rs::{
    app::{app, app_state_init},
    scraper::Scraper,
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
        web::Data::new(app_state_init(pool.into(), Scraper::new(None).unwrap())),
        cookie_key,
    ))
    .await
}

#[sqlx::test(migrations = "./migrations", fixtures("oauth"))]
async fn test_oauth_post_token_password_grant_success(pool: SqlitePool) {
    let app = init_app(pool).await;

    let password = "test_password_123";
    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_form(&[
            ("grant_type", "password"),
            ("username", "oauth_test_user"),
            ("password", password),
            ("client_id", "test_client_id"),
            ("client_secret", "test_client_secret"),
        ])
        .to_request();

    let resp: Value = test::call_and_read_body_json(&app, req).await;

    assert!(
        resp.get("access_token").is_some(),
        "Response should contain access_token"
    );
    assert!(
        resp.get("refresh_token").is_some(),
        "Response should contain refresh_token"
    );
    assert_eq!(
        resp.get("token_type").unwrap().as_str(),
        Some("bearer"),
        "Token type should be bearer"
    );
    assert!(
        resp.get("expires_in").unwrap().as_i64().unwrap() > 0,
        "Expiry should be positive"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("oauth"))]
async fn test_oauth_post_token_json_password_grant_success(pool: SqlitePool) {
    let app = init_app(pool).await;

    let password = "test_password_123";

    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_json(serde_json::json!({
            "grant_type": "password",
            "username": "oauth_test_user",
            "password": password,
            "client_id": "test_client_id",
            "client_secret": "test_client_secret"
        }))
        .to_request();

    let resp: Value = test::call_and_read_body_json(&app, req).await;

    assert!(
        resp.get("access_token").is_some(),
        "Response should contain access_token"
    );
    assert!(
        resp.get("refresh_token").is_some(),
        "Response should contain refresh_token"
    );
    assert_eq!(
        resp.get("token_type").unwrap().as_str(),
        Some("bearer"),
        "Token type should be bearer"
    );
    assert!(
        resp.get("expires_in").unwrap().as_i64().unwrap() > 0,
        "Expiry should be positive"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("oauth"))]
async fn test_oauth_missing_grant_type(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_form(&[("username", "oauth_test_user"), ("password", "password123")])
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        400,
        "Should return 400 for missing grant_type"
    );

    let body: Value = test::read_body_json(resp).await;

    assert_eq!(
        body.get("error").unwrap().as_str().unwrap(),
        "invalid_request",
        "Error should be invalid_request"
    );
    assert_eq!(
        body.get("error_description").unwrap().as_str().unwrap(),
        "Invalid grant_type parameter or parameter missing",
        "Should have correct error_description"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("oauth"))]
async fn test_oauth_invalid_grant_type(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_form(&[
            ("grant_type", "invalid_type"),
            ("username", "testuser"),
            ("password", "password123"),
        ])
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        400,
        "Should return 400 for invalid grant_type"
    );

    let body: Value = test::read_body_json(resp).await;

    assert_eq!(
        body.get("error").and_then(|v| v.as_str()),
        Some("invalid_request"),
        "Error should be invalid_request"
    );

    assert_eq!(
        body.get("error_description").unwrap().as_str().unwrap(),
        "Invalid grant_type parameter or parameter missing",
        "Should have correct error_description"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("oauth"))]
async fn test_oauth_invalid_credentials(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_form(&[
            ("grant_type", "password"),
            ("username", "test_user_invalid"),
            ("password", "wrong_password"),
            ("client_id", "test_client"),
            ("client_secret", "test_secret"),
        ])
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        400,
        "Should return 400 for invalid credentials"
    );

    let body: Value = test::read_body_json(resp).await;

    assert_eq!(
        body.get("error").unwrap().as_str().unwrap(),
        "invalid_grant",
        "Error should be invalid_grant"
    );

    assert_eq!(
        body.get("error_description").unwrap().as_str().unwrap(),
        "Invalid username and password combination",
        "Should have correct error_description"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("oauth"))]
async fn test_oauth_invalid_client(pool: SqlitePool) {
    let password = "test_password";

    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_form(&[
            ("grant_type", "password"),
            ("username", "test_user_client"),
            ("password", password),
            ("client_id", "invalid_client"),
            ("client_secret", "valid_secret"),
        ])
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 400, "Should return 400 for invalid client");

    let body: Value = test::read_body_json(resp).await;

    assert_eq!(
        body.get("error").unwrap().as_str().unwrap(),
        "invalid_client",
        "Error should be invalid_client"
    );

    assert_eq!(
        body.get("error_description").unwrap().as_str().unwrap(),
        "The client credentials are invalid",
        "Should have correct error_description"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("oauth"))]
async fn test_oauth_missing_username(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_form(&[
            ("grant_type", "password"),
            ("password", "test"),
            ("client_id", "client"),
            ("client_secret", "secret"),
        ])
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 400, "Should return 400 for missing username");

    let body: Value = test::read_body_json(resp).await;

    assert_eq!(
        body.get("error").unwrap().as_str().unwrap(),
        "invalid_request",
        "Error should be invalid_request"
    );

    assert_eq!(
        body.get("error_description").unwrap().as_str().unwrap(),
        "Missing parameters. \"username\" and \"password\" required",
        "Should have correct error_description"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("oauth"))]
async fn test_oauth_missing_password(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_form(&[
            ("grant_type", "password"),
            ("username", "user"),
            ("client_id", "client"),
            ("client_secret", "secret"),
        ])
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 400, "Should return 400 for missing password");

    let body: Value = test::read_body_json(resp).await;

    assert_eq!(
        body.get("error").unwrap().as_str().unwrap(),
        "invalid_request",
        "Error should be invalid_request"
    );

    assert_eq!(
        body.get("error_description").unwrap().as_str().unwrap(),
        "Missing parameters. \"username\" and \"password\" required",
        "Should have correct error_description"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("oauth"))]
async fn test_oauth_missing_client_id(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_form(&[
            ("grant_type", "password"),
            ("username", "user"),
            ("password", "test"),
            ("client_secret", "secret"),
        ])
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        400,
        "Should return 400 for missing client_id"
    );

    let body: Value = test::read_body_json(resp).await;

    assert_eq!(
        body.get("error").unwrap().as_str().unwrap(),
        "invalid_client",
        "Error should be invalid_client"
    );

    assert_eq!(
        body.get("error_description").unwrap().as_str().unwrap(),
        "Client id was not found in the headers or body",
        "Should have correct error_description"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("oauth"))]
async fn test_oauth_missing_client_secret(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_form(&[
            ("grant_type", "password"),
            ("username", "user"),
            ("password", "test"),
            ("client_id", "client"),
        ])
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        400,
        "Should return 400 for missing client_secret"
    );

    let body: Value = test::read_body_json(resp).await;

    assert_eq!(
        body.get("error").unwrap().as_str().unwrap(),
        "invalid_client",
        "Error should be invalid_client"
    );

    assert_eq!(
        body.get("error_description").unwrap().as_str().unwrap(),
        "The client credentials are invalid",
        "Should have correct error_description"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("oauth"))]
async fn test_oauth_refresh_token_grant_success(pool: SqlitePool) {
    let app = init_app(pool).await;

    let password = "test_password";

    // First, get an initial token using password grant
    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_form(&[
            ("grant_type", "password"),
            ("username", "refresh_test_user"),
            ("password", password),
            ("client_id", "refresh_client"),
            ("client_secret", "refresh_secret"),
        ])
        .to_request();

    let initial_resp: Value = test::call_and_read_body_json(&app, req).await;
    let refresh_token = initial_resp.get("refresh_token").unwrap().as_str().unwrap();
    let initial_access_token = initial_resp.get("access_token").unwrap().as_str().unwrap();

    // Now use the refresh token to get a new token
    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_form(&[
            ("grant_type", "refresh_token"),
            ("client_id", "refresh_client"),
            ("client_secret", "refresh_secret"),
            ("refresh_token", refresh_token),
        ])
        .to_request();

    let refresh_resp: Value = test::call_and_read_body_json(&app, req).await;

    assert!(
        refresh_resp.get("access_token").is_some(),
        "Refreshed response should contain access_token"
    );
    assert!(
        refresh_resp.get("refresh_token").is_some(),
        "Refreshed response should contain new refresh_token"
    );

    let new_access_token = refresh_resp.get("access_token").unwrap().as_str().unwrap();
    let new_refresh_token = refresh_resp.get("refresh_token").unwrap().as_str().unwrap();

    assert_ne!(
        initial_access_token, new_access_token,
        "New access token should be different from initial"
    );
    assert_ne!(
        refresh_token, new_refresh_token,
        "New refresh token should be different from initial"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("oauth"))]
async fn test_oauth_refresh_json_token_grant_success(pool: SqlitePool) {
    let app = init_app(pool).await;

    let password = "test_password";

    // First, get an initial token using password grant
    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_json(serde_json::json!({
            "grant_type": "password",
            "username": "refresh_test_user",
            "password": password,
            "client_id": "refresh_client",
            "client_secret": "refresh_secret"
        }))
        .to_request();

    let initial_resp: Value = test::call_and_read_body_json(&app, req).await;
    let refresh_token = initial_resp.get("refresh_token").unwrap().as_str().unwrap();
    let initial_access_token = initial_resp.get("access_token").unwrap().as_str().unwrap();

    // Now use the refresh token to get a new token
    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_json(serde_json::json!({
            "grant_type": "refresh_token",
            "client_id": "refresh_client",
            "client_secret": "refresh_secret",
            "refresh_token": refresh_token
        }))
        .to_request();

    let refresh_resp: Value = test::call_and_read_body_json(&app, req).await;

    assert!(
        refresh_resp.get("access_token").is_some(),
        "Refreshed response should contain access_token"
    );
    assert!(
        refresh_resp.get("refresh_token").is_some(),
        "Refreshed response should contain new refresh_token"
    );

    let new_access_token = refresh_resp.get("access_token").unwrap().as_str().unwrap();
    let new_refresh_token = refresh_resp.get("refresh_token").unwrap().as_str().unwrap();

    assert_ne!(
        initial_access_token, new_access_token,
        "New access token should be different from initial"
    );
    assert_ne!(
        refresh_token, new_refresh_token,
        "New refresh token should be different from initial"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("oauth"))]
async fn test_oauth_refresh_with_invalid_token(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_form(&[
            ("grant_type", "refresh_token"),
            ("client_id", "invalid_refresh_client"),
            ("client_secret", "invalid_refresh_secret"),
            ("refresh_token", "totally_invalid_token"),
        ])
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        400,
        "Should return 400 for invalid refresh token"
    );

    let body: Value = test::read_body_json(resp).await;

    assert_eq!(
        body.get("error").unwrap().as_str().unwrap(),
        "invalid_grant",
        "Error should be invalid_grant"
    );

    assert_eq!(
        body.get("error_description").unwrap().as_str().unwrap(),
        "Invalid refresh token",
        "Should have correct error_description"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("oauth"))]
async fn test_oauth_refresh_missing_refresh_token(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .uri("/oauth/v2/token")
        .set_form(&[
            ("grant_type", "refresh_token"),
            ("client_id", "invalid_refresh_client"),
            ("client_secret", "invalid_refresh_secret"),
        ])
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        400,
        "Should return 400 for missing refresh_token parameter"
    );

    let body: Value = test::read_body_json(resp).await;

    assert_eq!(
        body.get("error").and_then(|v| v.as_str()),
        Some("invalid_request"),
        "Error should be invalid_request"
    );

    assert_eq!(
        body.get("error_description").unwrap().as_str().unwrap(),
        "No \"refresh_token\" parameter found",
        "Should have correct error_description"
    );
}
