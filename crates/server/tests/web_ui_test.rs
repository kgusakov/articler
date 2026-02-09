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
use scraper::{Html, Selector};
use server::{
    app::{app, app_state_init, init_handlebars},
    scraper::Scraper,
};
use sqlx::SqlitePool;
use std::sync::Once;

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
        web::Data::new(app_state_init(
            pool,
            Scraper::new(None).unwrap(),
            init_handlebars(),
        )),
        cookie_key,
    ))
    .await
}

#[sqlx::test(migrations = "../../migrations")]
async fn index_without_auth_must_redirect_to_login(pool: SqlitePool) {
    let app = init_ui_app(pool).await;

    // Step 1: Check that "/" redirects to "/login"
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::FOUND);
    let location = resp
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(location, "/login");
}

#[sqlx::test(migrations = "../../migrations")]
async fn login_page(pool: SqlitePool) {
    let app = init_ui_app(pool).await;

    let req = test::TestRequest::get().uri("/login").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let document = Html::parse_document(content);
    let form = document
        .select(&Selector::parse("form#main-form").unwrap())
        .next()
        .unwrap()
        .value();
    assert_eq!(form.attr("action").unwrap(), "/do_login");

    assert_eq!(
        document
            .select(&Selector::parse("form input#main-login").unwrap())
            .next()
            .unwrap()
            .attr("name")
            .unwrap(),
        "_username"
    );

    assert_eq!(
        document
            .select(&Selector::parse("form input#main-password").unwrap())
            .next()
            .unwrap()
            .attr("name")
            .unwrap(),
        "_password"
    );
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql",)
)]
async fn do_login_with_correct_credentials(pool: SqlitePool) {
    let app = init_ui_app(pool).await;

    let req = test::TestRequest::post()
        .uri("/do_login")
        .set_form([("_username", "wallabag"), ("_password", "wallabag")])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::FOUND);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/");

    let session_cookie = resp
        .response()
        .cookies()
        .find(|c| c.name() == "id")
        .unwrap()
        .into_owned();

    let req = test::TestRequest::get()
        .uri("/")
        .cookie(session_cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql",)
)]
async fn do_login_with_incorrect_credentials(pool: SqlitePool) {
    let app = init_ui_app(pool).await;

    let req = test::TestRequest::post()
        .uri("/do_login")
        .set_form([("_username", "wallabag"), ("_password", "wrong_password")])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::FOUND);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/");

    assert!(resp.response().cookies().peekable().peek().is_none());
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn main_page(pool: SqlitePool) {
    let app = init_ui_app(pool).await;

    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::get()
        .uri("/")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let document = Html::parse_document(content);
    let article_titles: Vec<String> = document
        .select(&Selector::parse("article h3").unwrap())
        .map(|el| el.text().collect::<String>())
        .collect();

    // Exactly 3 unread (not archived) articles from fixtures: entries 1, 3, and 5
    assert_eq!(article_titles.len(), 3);
    assert!(article_titles.iter().any(|t| t == "title1"));
    assert!(article_titles.iter().any(|t| t == "title3"));
    assert!(article_titles.iter().any(|t| t == "title5"));
}

async fn login(
    username: &str,
    password: &str,
    app: impl Service<Request, Response = ServiceResponse<impl MessageBody>, Error = Error>,
) -> Cookie<'static> {
    let req = test::TestRequest::post()
        .uri("/do_login")
        .set_form([("_username", username), ("_password", password)])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::FOUND);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/");

    resp.response()
        .cookies()
        .find(|c| c.name() == "id")
        .unwrap()
        .into_owned()
}
