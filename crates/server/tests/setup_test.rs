use actix_http::{Request, StatusCode};
use actix_service::Service;
use actix_web::{
    Error, body::MessageBody, cookie::Key, dev::ServiceResponse, http::header, test, web,
};
use app_state::AppState;
use article_scraper::Scraper;
use server::app::{app, init_handlebars};
use sqlx::SqlitePool;
use std::sync::Once;

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

#[sqlx::test(migrations = "../../migrations")]
async fn login_redirects_to_setup_when_no_users(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::get().uri("/login").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::FOUND);
    assert_eq!(
        resp.headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap(),
        "/setup"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn setup_page_renders_when_no_users(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::get().uri("/setup").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    let body = test::read_body(resp).await;
    let content = std::str::from_utf8(&body).unwrap();
    assert_eq!(
        helpers::find_submit_button_text(content),
        Some("Create account".to_owned())
    );
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql")
)]
async fn setup_page_redirects_to_login_when_users_exist(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::get().uri("/setup").to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::FOUND);
    assert_eq!(
        resp.headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap(),
        "/login"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn do_setup_with_mismatched_passwords_returns_error(pool: SqlitePool) {
    let app = init_app(pool).await;

    let req = test::TestRequest::post()
        .uri("/do_setup")
        .set_form([
            ("username", "admin"),
            ("password", "password123"),
            ("confirm_password", "different123"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = test::read_body(resp).await;
    assert_eq!(
        std::str::from_utf8(&body).unwrap(),
        "Passwords do not match"
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn do_setup_creates_user_and_redirects_to_login(pool: SqlitePool) {
    let app = init_app(pool.clone()).await;

    let req = test::TestRequest::post()
        .uri("/do_setup")
        .set_form([
            ("username", "admin"),
            ("password", "password123"),
            ("confirm_password", "password123"),
        ])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("HX-Redirect").unwrap().to_str().unwrap(),
        "/login"
    );

    let count = db::repository::users::count(&pool).await.unwrap();
    assert_eq!(count, 1);
}

mod helpers {
    use scraper::{Html, Selector};

    pub fn find_submit_button_text(content: &str) -> Option<String> {
        let document = Html::parse_document(content);
        document
            .select(&Selector::parse(r#"form#setup-form button[type="submit"]"#).unwrap())
            .next()
            .map(|el| el.text().collect::<String>().trim().to_owned())
    }
}
