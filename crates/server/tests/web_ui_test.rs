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
    app::{AppState, app, init_handlebars},
    scraper::Scraper,
};
use sqlx::SqlitePool;
use std::{collections::HashSet, sync::Once};

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
async fn index_page(pool: SqlitePool) {
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

    let titles = helpers::find_article_titles(content);
    let titles_set: HashSet<&str> = titles.iter().map(std::string::String::as_str).collect();

    assert_eq!(titles_set, HashSet::from(["title1", "title3", "title5"]));

    let forms = helpers::find_archive_forms(content);
    let forms_set: HashSet<&str> = forms.iter().map(std::string::String::as_str).collect();
    assert_eq!(forms_set, HashSet::from(["1", "3", "5"]));

    let archive_icons = helpers::find_archive_icons(content);
    assert_eq!(archive_icons.len(), 3);
    assert!(
        archive_icons
            .iter()
            .all(|src| src == "/static/images/MarkUnRead.svg")
    );

    let del_forms = helpers::find_delete_forms(content);
    let delete_forms: HashSet<&str> = del_forms.iter().map(std::string::String::as_str).collect();
    assert_eq!(delete_forms, HashSet::from(["1", "3", "5"]));
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn article_links_on_index_page(pool: SqlitePool) {
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

    let links = helpers::find_article_links(content);
    let links_set: HashSet<&str> = links.iter().map(std::string::String::as_str).collect();

    assert_eq!(
        links_set,
        HashSet::from(["/article/1", "/article/3", "/article/5"])
    );
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn do_archive(pool: SqlitePool) {
    let app = init_ui_app(pool).await;

    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::post()
        .uri("/do_archive")
        .cookie(cookie.clone())
        .set_form([("article_id", "1"), ("archived", "true")])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/");

    let req = test::TestRequest::get()
        .uri("/")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let titles = helpers::find_article_titles(content);
    let titles_set: HashSet<&str> = titles.iter().map(std::string::String::as_str).collect();

    assert_eq!(titles_set, HashSet::from(["title3", "title5"]));
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn do_unarchive(pool: SqlitePool) {
    let app = init_ui_app(pool).await;

    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::post()
        .uri("/do_archive")
        .cookie(cookie.clone())
        .set_form([("article_id", "2"), ("archived", "false")])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/");

    let req = test::TestRequest::get()
        .uri("/")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let titles = helpers::find_article_titles(content);
    let titles_set: HashSet<&str> = titles.iter().map(std::string::String::as_str).collect();

    assert_eq!(
        titles_set,
        HashSet::from(["title1", "title2", "title3", "title5"])
    );
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn all_page(pool: SqlitePool) {
    let app = init_ui_app(pool).await;

    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::get()
        .uri("/all")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let titles = helpers::find_article_titles(content);
    let titles_set: HashSet<&str> = titles.iter().map(std::string::String::as_str).collect();

    assert_eq!(
        titles_set,
        HashSet::from(["title1", "title2", "title3", "title4", "title5", "title6"])
    );
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn favourite_page(pool: SqlitePool) {
    let app = init_ui_app(pool).await;

    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::get()
        .uri("/favourite")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let titles = helpers::find_article_titles(content);
    let titles_set: HashSet<&str> = titles.iter().map(std::string::String::as_str).collect();

    assert_eq!(titles_set, HashSet::from(["title3", "title4", "title6"]));
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn archive_page(pool: SqlitePool) {
    let app = init_ui_app(pool).await;

    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::get()
        .uri("/archive")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let titles = helpers::find_article_titles(content);
    let titles_set: HashSet<&str> = titles.iter().map(std::string::String::as_str).collect();

    assert_eq!(titles_set, HashSet::from(["title2", "title4", "title6"]));

    let archive_icons = helpers::find_archive_icons(content);
    assert_eq!(archive_icons.len(), 3);
    assert!(
        archive_icons
            .iter()
            .all(|src| src == "/static/images/MarkRead.svg")
    );
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn index_page_favourite_icons(pool: SqlitePool) {
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

    let fav_icons = helpers::find_favourite_icons_by_article(content);
    let icons: HashSet<(&str, &str)> = fav_icons
        .iter()
        .map(|(id, src)| (id.as_str(), src.as_str()))
        .collect();

    assert_eq!(
        icons,
        HashSet::from([
            ("1", "/static/images/FavoriteOff.svg"),
            ("3", "/static/images/FavoriteOn.svg"),
            ("5", "/static/images/FavoriteOff.svg")
        ])
    );
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn favourite_page_favourite_icons(pool: SqlitePool) {
    let app = init_ui_app(pool).await;
    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::get()
        .uri("/favourite")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let icons = helpers::find_favourite_icons_by_article(content);
    assert_eq!(icons.len(), 3);

    assert!(
        icons
            .iter()
            .all(|(_, src)| src == "/static/images/FavoriteOn.svg")
    );
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn do_favourite(pool: SqlitePool) {
    let app = init_ui_app(pool).await;
    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::post()
        .uri("/do_favourite")
        .cookie(cookie.clone())
        .set_form([("article_id", "1"), ("starred", "true")])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/");

    let req = test::TestRequest::get()
        .uri("/favourite")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let titles = helpers::find_article_titles(content);
    let titles_set: HashSet<&str> = titles.iter().map(std::string::String::as_str).collect();
    assert_eq!(
        titles_set,
        HashSet::from(["title1", "title3", "title4", "title6"])
    );

    let req = test::TestRequest::get()
        .uri("/")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let fav_icons = helpers::find_favourite_icons_by_article(content);
    let icons: HashSet<(&str, &str)> = fav_icons
        .iter()
        .map(|(id, src)| (id.as_str(), src.as_str()))
        .collect();
    assert!(icons.contains(&("1", "/static/images/FavoriteOn.svg")));
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn do_favourite_htmx(pool: SqlitePool) {
    let app = init_ui_app(pool).await;
    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::get()
        .uri("/")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().get(header::LOCATION).is_none());

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let fav_icons = helpers::find_favourite_icons_by_article(content);
    let icons: HashSet<(&str, &str)> = fav_icons
        .iter()
        .map(|(id, src)| (id.as_str(), src.as_str()))
        .collect();
    assert!(!icons.contains(&("1", "/static/images/FavoriteOn.svg")));

    let req = test::TestRequest::post()
        .uri("/do_favourite")
        .cookie(cookie.clone())
        .insert_header(("HX-Request", "true"))
        .insert_header((header::REFERER, "/"))
        .set_form([("article_id", "1"), ("starred", "true")])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp.headers().get(header::LOCATION).is_none());

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let fav_icons = helpers::find_favourite_icons_by_article(content);
    let icons: HashSet<(&str, &str)> = fav_icons
        .iter()
        .map(|(id, src)| (id.as_str(), src.as_str()))
        .collect();
    assert!(icons.contains(&("1", "/static/images/FavoriteOn.svg")));
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn do_unfavourite(pool: SqlitePool) {
    let app = init_ui_app(pool).await;
    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::post()
        .uri("/do_favourite")
        .cookie(cookie.clone())
        .set_form([("article_id", "3"), ("starred", "false")])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/");

    let req = test::TestRequest::get()
        .uri("/favourite")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let titles = helpers::find_article_titles(content);
    let article_titles: HashSet<&str> = titles.iter().map(std::string::String::as_str).collect();
    assert_eq!(article_titles, HashSet::from(["title4", "title6"]));

    let req = test::TestRequest::get()
        .uri("/")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let icons = helpers::find_favourite_icons_by_article(content);
    let icons_set: HashSet<(&str, &str)> = icons
        .iter()
        .map(|(id, src)| (id.as_str(), src.as_str()))
        .collect();
    assert!(icons_set.contains(&("3", "/static/images/FavoriteOff.svg")));
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn active_category_highlighting(pool: SqlitePool) {
    let app = init_ui_app(pool).await;
    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::get()
        .uri("/")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();
    let active_category = helpers::find_active_category(content);
    assert_eq!(active_category, Some("unread".to_owned()));

    let req = test::TestRequest::get()
        .uri("/all")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();
    let active_category = helpers::find_active_category(content);
    assert_eq!(active_category, Some("all".to_owned()));

    let req = test::TestRequest::get()
        .uri("/favourite")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();
    let active_category = helpers::find_active_category(content);
    assert_eq!(active_category, Some("favourite".to_owned()));

    let req = test::TestRequest::get()
        .uri("/archive")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();
    let active_category = helpers::find_active_category(content);
    assert_eq!(active_category, Some("archived".to_owned()));
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn article_page_not_found(pool: SqlitePool) {
    let app = init_ui_app(pool).await;
    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::get()
        .uri("/article/999")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn article_page_unarchived_unstarred(pool: SqlitePool) {
    let app = init_ui_app(pool).await;
    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::get()
        .uri("/article/1")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    assert_eq!(
        helpers::find_article_title(content),
        Some("title1".to_owned())
    );

    // TODO replace by actual html path searching
    assert!(content.contains("content1"));

    let (domain_text, domain_href) = helpers::find_article_domain_link(content).unwrap();
    assert_eq!(domain_text, "a.com");
    assert_eq!(domain_href, "https://a.com/1");

    // TODO replace by actual html path searching
    assert!(content.contains("8 min read"));

    let (_, fav_icon) = helpers::find_favourite_icons_by_article(content)
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(fav_icon, "/static/images/FavoriteOff.svg");

    let archive_icon = helpers::find_archive_icons(content)
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(archive_icon, "/static/images/MarkUnRead.svg");

    let delete_forms = helpers::find_delete_forms(content);
    assert_eq!(delete_forms.len(), 1);
    assert_eq!(delete_forms[0], "1");
    assert_eq!(
        helpers::find_delete_back_location(content),
        Some("/".to_owned())
    );
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn article_page_archived_starred(pool: SqlitePool) {
    let app = init_ui_app(pool).await;
    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::get()
        .uri("/article/4")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    assert_eq!(
        helpers::find_article_title(content),
        Some("title4".to_owned())
    );
    assert!(content.contains("content4"));

    let (domain_text, domain_href) = helpers::find_article_domain_link(content).unwrap();
    assert_eq!(domain_text, "d.com");
    assert_eq!(domain_href, "https://d.com/4");

    assert!(content.contains("15 min read"));

    let (_, fav_icon) = helpers::find_favourite_icons_by_article(content)
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(fav_icon, "/static/images/FavoriteOn.svg");

    let archive_icon = helpers::find_archive_icons(content)
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(archive_icon, "/static/images/MarkRead.svg");

    let delete_forms = helpers::find_delete_forms(content);
    assert_eq!(delete_forms.len(), 1);
    assert_eq!(delete_forms[0], "4");
    assert_eq!(
        helpers::find_delete_back_location(content),
        Some("/".to_owned())
    );
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn do_delete(pool: SqlitePool) {
    let app = init_ui_app(pool).await;
    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::get()
        .uri("/article/1")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let req = test::TestRequest::post()
        .uri("/do_delete")
        .cookie(cookie.clone())
        .set_form([("article_id", "1")])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/");

    let req = test::TestRequest::get()
        .uri("/article/1")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = "../../migrations")]
async fn clients_without_auth_must_redirect_to_login(pool: SqlitePool) {
    let app = init_ui_app(pool).await;

    let req = test::TestRequest::get().uri("/clients").to_request();
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

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql")
)]
async fn clients_page(pool: SqlitePool) {
    let app = init_ui_app(pool).await;
    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::get()
        .uri("/clients")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let clients = helpers::find_clients(content);

    let clients_set: HashSet<(&str, &str, &str)> = clients
        .iter()
        .map(|(name, id, secret)| (name.as_str(), id.as_str(), secret.as_str()))
        .collect();

    assert_eq!(
        clients_set,
        HashSet::from([
            ("Client 1", "client_1", "secret_1"),
            ("Client 2", "client_2", "secret_2"),
            ("Android app", "android_client_id", "android_client_secret")
        ])
    );
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql")
)]
async fn logout_clears_session(pool: SqlitePool) {
    let app = init_ui_app(pool).await;
    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::get()
        .uri("/")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let req = test::TestRequest::get()
        .uri("/logout")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/login");

    let deletion_cookie = resp
        .response()
        .cookies()
        .find(|c| c.name() == "id")
        .expect("Logout should return a deletion cookie");

    let req = test::TestRequest::get()
        .uri("/")
        .cookie(deletion_cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::FOUND);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/login");
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql")
)]
async fn do_create_client(pool: SqlitePool) {
    let app = init_ui_app(pool).await;
    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::get()
        .uri("/clients")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let clients_before = helpers::find_clients(content);
    assert_eq!(clients_before.len(), 3);

    let req = test::TestRequest::post()
        .uri("/do_create_client")
        .insert_header((header::REFERER, "/clients"))
        .cookie(cookie.clone())
        .set_form([("client_name", "New Test Client")])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/clients");

    let req = test::TestRequest::get()
        .uri("/clients")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let clients_after = helpers::find_clients(content);
    assert_eq!(clients_after.len(), 4);

    let new_client = clients_after
        .iter()
        .find(|(name, _, _)| name == "New Test Client")
        .expect("New client should be present");

    assert_eq!(new_client.0, "New Test Client");
    assert!(!new_client.1.is_empty(), "Client ID should not be empty");
    assert!(
        !new_client.2.is_empty(),
        "Client secret should not be empty"
    );
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql")
)]
async fn do_client_delete(pool: SqlitePool) {
    let app = init_ui_app(pool).await;
    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::get()
        .uri("/clients")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let clients_before = helpers::find_clients(content);
    assert_eq!(clients_before.len(), 3);

    let client_to_delete = clients_before
        .iter()
        .find(|(name, _, _)| name == "Client 1")
        .expect("Client 1 should exist");
    assert_eq!(client_to_delete.0, "Client 1");
    assert_eq!(client_to_delete.1, "client_1");
    assert_eq!(client_to_delete.2, "secret_1");

    let delete_forms = helpers::find_client_delete_forms(content);
    assert_eq!(delete_forms.len(), 3);
    assert!(delete_forms.contains(&"1".to_owned()));

    let req = test::TestRequest::post()
        .uri("/do_client_delete")
        .insert_header((header::REFERER, "/clients"))
        .cookie(cookie.clone())
        .set_form([("id", "1")])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/clients");

    let req = test::TestRequest::get()
        .uri("/clients")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let clients_after = helpers::find_clients(content);
    assert_eq!(clients_after.len(), 2);

    let deleted_client = clients_after.iter().find(|(name, _, _)| name == "Client 1");
    assert!(
        deleted_client.is_none(),
        "Client 1 should not exist after deletion"
    );

    let remaining_clients: HashSet<&str> = clients_after
        .iter()
        .map(|(name, _, _)| name.as_str())
        .collect();
    assert_eq!(
        remaining_clients,
        HashSet::from(["Client 2", "Android app"])
    );
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn do_add(pool: SqlitePool) {
    let app = init_ui_app(pool).await;

    let mock_server = wiremock::MockServer::start().await;

    let content = r#"<!DOCTYPE html><html lang="en"><head><title>Scraped Article Title</title></head><body><p>Article body content</p></body></html>"#;

    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/test-article"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_raw(content, "text/html"))
        .mount(&mock_server)
        .await;

    let url = format!("{}/test-article", mock_server.uri());

    let cookie = login("wallabag", "wallabag", &app).await;

    let req = test::TestRequest::post()
        .uri("/add")
        .insert_header((header::REFERER, "/all"))
        .cookie(cookie.clone())
        .set_form([("url", url.as_str())])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/all");

    let req = test::TestRequest::get()
        .uri("/")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let page_content = str::from_utf8(&body).unwrap();

    let titles = helpers::find_article_titles(page_content);

    assert!(
        titles.contains(&"Scraped Article Title".to_owned()),
        "Expected 'Scraped Article Title' in {titles:?}"
    );
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

mod helpers {
    use scraper::{Html, Selector};

    pub fn find_article_titles(content: &str) -> Vec<String> {
        let document = Html::parse_document(content);
        document
            .select(&Selector::parse("article h3").unwrap())
            .map(|el| el.inner_html())
            .collect()
    }

    /// Returns the icon src from the archive button in each archive form.
    pub fn find_archive_icons(content: &str) -> Vec<String> {
        let document = Html::parse_document(content);
        let form_sel = Selector::parse(r#"form[action="/do_archive"]"#).unwrap();
        let img_sel = Selector::parse("button img").unwrap();

        document
            .select(&form_sel)
            .filter_map(|form| {
                form.select(&img_sel)
                    .next()
                    .and_then(|img| img.value().attr("src"))
                    .map(std::borrow::ToOwned::to_owned)
            })
            .collect()
    }

    /// Returns the `article_id` values from all archive forms in the page.
    pub fn find_archive_forms(content: &str) -> Vec<String> {
        let document = Html::parse_document(content);
        let form_sel = Selector::parse(r#"form[action="/do_archive"]"#).unwrap();
        let input_sel = Selector::parse(r#"input[name="article_id"]"#).unwrap();

        document
            .select(&form_sel)
            .filter_map(|form| {
                form.select(&input_sel)
                    .next()
                    .and_then(|input| input.value().attr("value"))
                    .map(std::borrow::ToOwned::to_owned)
            })
            .collect()
    }

    /// Returns the `article_id` values from all delete forms in the page.
    pub fn find_delete_forms(content: &str) -> Vec<String> {
        let document = Html::parse_document(content);
        let form_sel = Selector::parse(r#"form[action="/do_delete"]"#).unwrap();
        let input_sel = Selector::parse(r#"input[name="article_id"]"#).unwrap();

        document
            .select(&form_sel)
            .filter_map(|form| {
                form.select(&input_sel)
                    .next()
                    .and_then(|input| input.value().attr("value"))
                    .map(std::borrow::ToOwned::to_owned)
            })
            .collect()
    }

    /// Returns the `back_location` value from the delete form, if present.
    pub fn find_delete_back_location(content: &str) -> Option<String> {
        let document = Html::parse_document(content);
        let form_sel = Selector::parse(r#"form[action="/do_delete"]"#).unwrap();
        let input_sel = Selector::parse(r#"input[name="back_location"]"#).unwrap();

        document.select(&form_sel).next().and_then(|form| {
            form.select(&input_sel)
                .next()
                .and_then(|input| input.value().attr("value"))
                .map(std::borrow::ToOwned::to_owned)
        })
    }

    /// Returns (`article_id`, `icon_src`) pairs from all favourite forms in the page.
    pub fn find_favourite_icons_by_article(content: &str) -> Vec<(String, String)> {
        let document = Html::parse_document(content);
        let form_sel = Selector::parse(r#"form[action="/do_favourite"]"#).unwrap();
        let input_sel = Selector::parse(r#"input[name="article_id"]"#).unwrap();
        let img_sel = Selector::parse("button img").unwrap();

        document
            .select(&form_sel)
            .filter_map(|form| {
                let article_id = form
                    .select(&input_sel)
                    .next()
                    .and_then(|input| input.value().attr("value"))
                    .map(std::borrow::ToOwned::to_owned)?;
                let icon_src = form
                    .select(&img_sel)
                    .next()
                    .and_then(|img| img.value().attr("src"))
                    .map(std::borrow::ToOwned::to_owned)?;
                Some((article_id, icon_src))
            })
            .collect()
    }

    /// Returns deduplicated article page hrefs (e.g. "/article/1") from the listing page.
    pub fn find_article_links(content: &str) -> Vec<String> {
        let document = Html::parse_document(content);
        let sel = Selector::parse(r#"a[href^="/article/"]"#).unwrap();
        let mut seen = std::collections::HashSet::new();
        document
            .select(&sel)
            .filter_map(|el| el.value().attr("href").map(std::borrow::ToOwned::to_owned))
            .filter(|href| seen.insert(href.clone()))
            .collect()
    }

    /// Returns the article title from the h1 element on the article detail page.
    pub fn find_article_title(content: &str) -> Option<String> {
        let document = Html::parse_document(content);
        document
            .select(&Selector::parse("h1").unwrap())
            .next()
            .map(|el| el.text().collect::<String>())
    }

    /// Returns (`domain_text`, href) from the domain link on the article detail page.
    pub fn find_article_domain_link(content: &str) -> Option<(String, String)> {
        let document = Html::parse_document(content);
        let link = document
            .select(&Selector::parse(r#"a[target="_blank"]"#).unwrap())
            .next()?;
        let href = link.value().attr("href")?.to_owned();
        let text = link.text().collect::<String>();
        Some((text, href))
    }

    pub fn find_active_category(content: &str) -> Option<String> {
        let document = Html::parse_document(content);
        let sidebar_sel = Selector::parse("aside a").unwrap();

        for link in document.select(&sidebar_sel) {
            let class_attr = link.value().attr("class")?;
            if class_attr.contains("bg-surface") && !class_attr.contains("hover:bg-surface") {
                let href = link.value().attr("href")?;
                let category = match href {
                    "/" => "unread",
                    "/all" => "all",
                    "/favourite" => "favourite",
                    "/archive" => "archived",
                    _ => continue,
                };
                return Some(category.to_owned());
            }
        }
        None
    }

    /// Returns clients from the clients page as (name, `client_id`, `client_secret`) tuples.
    pub fn find_clients(content: &str) -> Vec<(String, String, String)> {
        let document = Html::parse_document(content);
        let details_sel = Selector::parse("details").unwrap();
        let name_sel = Selector::parse("summary span.font-medium").unwrap();
        let code_sel = Selector::parse("div.bg-gray-50 code").unwrap();

        document
            .select(&details_sel)
            .filter_map(|details| {
                let name = details.select(&name_sel).next()?.text().collect::<String>();

                let mut codes = details.select(&code_sel);
                let client_id = codes.next()?.text().collect::<String>();
                let client_secret = codes.next()?.text().collect::<String>();

                Some((name, client_id, client_secret))
            })
            .collect()
    }

    /// Returns the client id values from all client delete forms in the page.
    pub fn find_client_delete_forms(content: &str) -> Vec<String> {
        let document = Html::parse_document(content);
        let form_sel = Selector::parse(r#"form[action="/do_client_delete"]"#).unwrap();
        let input_sel = Selector::parse(r#"input[name="id"]"#).unwrap();

        document
            .select(&form_sel)
            .filter_map(|form| {
                form.select(&input_sel)
                    .next()
                    .and_then(|input| input.value().attr("value"))
                    .map(std::borrow::ToOwned::to_owned)
            })
            .collect()
    }
}
