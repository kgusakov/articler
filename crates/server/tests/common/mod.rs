#![allow(dead_code)]

use std::sync::Once;

use actix_http::{Request, StatusCode};
use actix_service::Service;
use actix_web::{
    Error,
    body::MessageBody,
    cookie::{Cookie, Key},
    dev::ServiceResponse,
    http::header,
    test, web,
};
use app_state::AppState;
use article_scraper::Scraper;
use server::app::{app, init_handlebars};
use sqlx::SqlitePool;
use tokio::sync::OnceCell;

pub const USER_ID: i64 = 1;
pub const CLIENT_ID: i64 = 1;
pub const USERNAME: &str = "wallabag";
pub const PASSWORD: &str = "wallabag";
pub const SESSION_COOKIE_NAME: &str = "id";

static LOGGER: Once = Once::new();

/// The session cookie is minted once per test binary and shared by every test.
///
/// This is sound because the web UI uses `CookieSessionStore` with the constant
/// `[0u8; 64]` key below: the cookie value *is* the session state (`{"user_id":"1"}`)
/// sealed with that key, carrying no TTL, nonce, or app-instance binding. It is
/// therefore valid against any app instance and any per-test pool.
///
/// The invariant this relies on — that the `users` fixture is loaded — is enforced by
/// `assert_users_fixture_loaded` on every `authed_ui_app` call, so it cannot be
/// satisfied by luck of scheduling.
static SESSION_COOKIE_VALUE: OnceCell<String> = OnceCell::const_new();

fn init_logger() {
    LOGGER.call_once(|| {
        env_logger::init_from_env(env_logger::Env::new().default_filter_or("trace"));
    });
}

fn cookie_key() -> Key {
    Key::from(&[0u8; 64])
}

pub async fn init_app(
    pool: SqlitePool,
) -> impl Service<Request, Response = ServiceResponse<impl MessageBody>, Error = Error> {
    init_app_with_state(pool).await.0
}

pub async fn init_app_with_state(
    pool: SqlitePool,
) -> (
    impl Service<Request, Response = ServiceResponse<impl MessageBody>, Error = Error>,
    web::Data<AppState>,
) {
    init_logger();

    let state = web::Data::new(AppState::new(
        pool,
        Scraper::new(None).unwrap(),
        init_handlebars().unwrap(),
    ));

    let service = test::init_service(app(state.clone(), cookie_key())).await;

    (service, state)
}

/// Builds an app plus an `Authorization` header value, minting the access token
/// directly from `TokenStorage` rather than through `/oauth/v2/token`.
///
/// Requires the `users` fixture: `new_token` persists a refresh-token row whose
/// foreign keys point at `users(id)` and `clients(id)`.
pub async fn authed_api_app(
    pool: SqlitePool,
) -> (
    impl Service<Request, Response = ServiceResponse<impl MessageBody>, Error = Error>,
    String,
) {
    let (service, state) = init_app_with_state(pool).await;

    let token = state
        .token_storage
        .new_token(&state.pool, USER_ID, CLIENT_ID)
        .await
        .unwrap();

    (service, format!("Bearer {}", token.access_token))
}

pub async fn authed_ui_app(
    pool: SqlitePool,
) -> (
    impl Service<Request, Response = ServiceResponse<impl MessageBody>, Error = Error>,
    Cookie<'static>,
) {
    let (service, state) = init_app_with_state(pool).await;

    assert_users_fixture_loaded(&state.pool).await;

    let cookie = session_cookie(&service).await;

    (service, cookie)
}

/// Verified on every call, not only on the one that fills [`SESSION_COOKIE_VALUE`], so
/// that a test missing the `users` fixture fails itself instead of whichever test
/// happened to win the race to initialise the cached cookie.
async fn assert_users_fixture_loaded(pool: &SqlitePool) {
    let user = db::repository::users::find_by_username(pool, USERNAME)
        .await
        .expect("querying the users table failed");

    assert!(
        user.is_some(),
        "authed_ui_app requires the `users` fixture: no user named `{USERNAME}` in this test's pool"
    );
}

pub async fn session_cookie<S, B>(service: &S) -> Cookie<'static>
where
    S: Service<Request, Response = ServiceResponse<B>, Error = Error>,
    B: MessageBody,
{
    let value = SESSION_COOKIE_VALUE
        .get_or_init(|| login(service, USERNAME, PASSWORD))
        .await;

    Cookie::new(SESSION_COOKIE_NAME, value.clone())
}

pub async fn login<S, B>(service: &S, username: &str, password: &str) -> String
where
    S: Service<Request, Response = ServiceResponse<B>, Error = Error>,
    B: MessageBody,
{
    let req = test::TestRequest::post()
        .uri("/do_login")
        .set_form([("_username", username), ("_password", password)])
        .to_request();

    let resp = test::call_service(service, req).await;

    assert_eq!(
        resp.status(),
        StatusCode::FOUND,
        "login as {username} failed; does this test load the `users` fixture?"
    );
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/");

    resp.response()
        .cookies()
        .find(|c| c.name() == SESSION_COOKIE_NAME)
        .unwrap()
        .value()
        .to_owned()
}
