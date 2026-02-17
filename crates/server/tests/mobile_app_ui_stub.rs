// This tests checks if Android wallabag app can login correctly
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
use regex::Regex;
use server::{
    app::{AppState, app, init_handlebars},
    scraper::Scraper,
};
use sqlx::SqlitePool;

static INIT: Once = Once::new();

fn init() {
    INIT.call_once(|| {
        env_logger::init_from_env(env_logger::Env::new().default_filter_or("trace"));
    });
}

async fn init_ui_app(
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

#[sqlx::test(migrations = "../../migrations", fixtures("users", "entries"))]
async fn android_app_login_flow(pool: SqlitePool) {
    let app = init_ui_app(pool).await;

    // Step 1: Check that "/" redirects to "/login"
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status().as_u16(), 302);
    let location = resp.headers().get("location").unwrap().to_str().unwrap();
    assert_eq!(location, "/login");

    // Step 2: Get login page and check patterns
    let req = test::TestRequest::get().uri("/login").to_request();
    let resp_body = test::call_and_read_body(&app, req).await;
    let login_html = std::str::from_utf8(&resp_body).unwrap();

    // Check WALLABAG_LOGIN_FORM_V2 pattern: /login_check" method="post" name="loginform">
    let login_form_pattern =
        Regex::new(r#"/login_check"? method="?post"? name="?loginform"?>"#).unwrap();
    assert!(
        login_form_pattern.is_match(login_html),
        "Login form pattern not found in: {login_html}"
    );

    // Check WALLABAG_LOGO_V2 pattern: alt="wallabag logo" />
    let logo_pattern = Regex::new(r#"alt="wallabag logo" ?/?>"#).unwrap();
    assert!(
        logo_pattern.is_match(login_html),
        "Wallabag logo pattern not found in: {login_html}"
    );

    // Step 3: Login with credentials from fixtures
    let req = test::TestRequest::post()
        .uri("/login_check")
        .set_form([("_username", "wallabag"), ("_password", "wallabag")])
        .to_request();
    let resp = test::call_service(&app, req).await;

    // Should redirect to "/"
    assert_eq!(resp.status().as_u16(), 302);
    let location = resp.headers().get("location").unwrap().to_str().unwrap();
    assert_eq!(location, "/");

    // Extract session cookie
    let cookie = resp.response().cookies().next().unwrap();

    // Step 4: Check logged-in home page has logout link
    let req = test::TestRequest::get()
        .uri("/")
        .cookie(cookie.clone())
        .to_request();
    let resp_body = test::call_and_read_body(&app, req).await;
    let home_html = std::str::from_utf8(&resp_body).unwrap();

    // Check WALLABAG_LOGOUT_LINK_V2 pattern: /logout">
    let logout_pattern = Regex::new(r#"/logout"?>"#).unwrap();
    assert!(
        logout_pattern.is_match(home_html),
        "Logout link pattern not found in: {home_html}"
    );

    // Step 5: Go to /developer and check client pattern
    let req = test::TestRequest::get()
        .uri("/developer")
        .cookie(cookie.clone())
        .to_request();
    let resp_body = test::call_and_read_body(&app, req).await;
    let developer_html = std::str::from_utf8(&resp_body).unwrap();

    let client_pattern = Regex::new(
        r#"(?s)<div class="collapsible-header">([^<]+?)</div>.*?<strong><code>([^<]+?)</code></strong>.*?<strong><code>([^<]+?)</code></strong>.*?<strong><code>([^<]+?)</code></strong>.*?<strong><code>([^<]+?)</code></strong>.*?/developer/client/delete/"#,
    )
    .unwrap();

    let mut clients = vec![];
    for (_, [name, id, secret, _, grant_permissions]) in client_pattern
        .captures_iter(developer_html)
        .map(|c| c.extract())
    {
        clients.push((name, id, secret, grant_permissions));
    }

    assert_eq!(
        clients,
        vec![
            (
                "Client 1 - #0",
                "client_1",
                "secret_1",
                "[\"token\",\"authorization_code\",\"password\",\"refresh_token\"]"
            ),
            (
                "Client 2 - #1",
                "client_2",
                "secret_2",
                "[\"token\",\"authorization_code\",\"password\",\"refresh_token\"]"
            ),
            (
                "Android app - #2",
                "android_client_id",
                "android_client_secret",
                "[\"token\",\"authorization_code\",\"password\",\"refresh_token\"]"
            )
        ]
    );
}
