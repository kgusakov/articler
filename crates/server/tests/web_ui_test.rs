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
    let titles_set: HashSet<&str> = titles.iter().map(|s| s.as_str()).collect();

    // Exactly 3 unread (not archived) articles from fixtures: entries 1, 3, and 5
    assert_eq!(titles_set, HashSet::from(["title1", "title3", "title5"]));

    // Each article must have an archive form with the correct article_id
    let forms = helpers::find_archive_forms(content);
    let forms_set: HashSet<&str> = forms.iter().map(|s| s.as_str()).collect();
    assert_eq!(forms_set, HashSet::from(["1", "3", "5"]));

    // Unarchived articles must show MarkUnRead icon
    let archive_icons = helpers::find_archive_icons(content);
    assert_eq!(archive_icons.len(), 3);
    assert!(
        archive_icons
            .iter()
            .all(|src| src == "/static/images/MarkUnRead.svg")
    );

    // Each article must have a delete form with the correct article_id
    let del_forms = helpers::find_delete_forms(content);
    let delete_forms: HashSet<&str> = del_forms.iter().map(|s| s.as_str()).collect();
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
    let links_set: HashSet<&str> = links.iter().map(|s| s.as_str()).collect();
    // Index shows unread entries 1, 3, 5
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

    // Archive entry 1 (currently unarchived)
    let req = test::TestRequest::post()
        .uri("/do_archive")
        .cookie(cookie.clone())
        .set_form([("article_id", "1"), ("archived", "true")])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/");

    // Verify main page no longer shows the archived article
    let req = test::TestRequest::get()
        .uri("/")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let titles = helpers::find_article_titles(content);
    let titles_set: HashSet<&str> = titles.iter().map(|s| s.as_str()).collect();

    // Entry 1 should no longer appear, only entries 3 and 5 remain
    assert_eq!(titles_set, HashSet::from(["title3", "title5"]));
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn do_unarchive(pool: SqlitePool) {
    let app = init_ui_app(pool).await;

    let cookie = login("wallabag", "wallabag", &app).await;

    // Unarchive entry 2 (currently archived)
    let req = test::TestRequest::post()
        .uri("/do_archive")
        .cookie(cookie.clone())
        .set_form([("article_id", "2"), ("archived", "false")])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/");

    // Verify main page now shows the unarchived article
    let req = test::TestRequest::get()
        .uri("/")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let titles = helpers::find_article_titles(content);
    let titles_set: HashSet<&str> = titles.iter().map(|s| s.as_str()).collect();

    // Entry 2 should now appear alongside entries 1, 3, and 5
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
    let titles_set: HashSet<&str> = titles.iter().map(|s| s.as_str()).collect();

    // All 6 entries should appear (no archive filter)
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
    let titles_set: HashSet<&str> = titles.iter().map(|s| s.as_str()).collect();

    // Entries 3, 4, and 6 are starred
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
    let titles_set: HashSet<&str> = titles.iter().map(|s| s.as_str()).collect();

    // Entries 2, 4, and 6 are archived
    assert_eq!(titles_set, HashSet::from(["title2", "title4", "title6"]));

    // Archived articles must show MarkRead icon
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

    // Entry 1: not starred → FavoriteOff
    // Entry 3: starred → FavoriteOn
    // Entry 5: not starred → FavoriteOff
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

    // All starred articles must show FavoriteOn icon
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

    // Star entry 1 (currently unstarred)
    let req = test::TestRequest::post()
        .uri("/do_favourite")
        .cookie(cookie.clone())
        .set_form([("article_id", "1"), ("starred", "true")])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/");

    // Verify entry 1 now appears on the favourite page
    let req = test::TestRequest::get()
        .uri("/favourite")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let titles = helpers::find_article_titles(content);
    let titles_set: HashSet<&str> = titles.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        titles_set,
        HashSet::from(["title1", "title3", "title4", "title6"])
    );

    // Verify entry 1 now shows FavoriteOn icon on the index page
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
async fn do_unfavourite(pool: SqlitePool) {
    let app = init_ui_app(pool).await;
    let cookie = login("wallabag", "wallabag", &app).await;

    // Unstar entry 3 (currently starred)
    let req = test::TestRequest::post()
        .uri("/do_favourite")
        .cookie(cookie.clone())
        .set_form([("article_id", "3"), ("starred", "false")])
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    assert_eq!(resp.headers().get(header::LOCATION).unwrap(), "/");

    // Verify entry 3 no longer appears on the favourite page
    let req = test::TestRequest::get()
        .uri("/favourite")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    let titles = helpers::find_article_titles(content);
    let article_titles: HashSet<&str> = titles.iter().map(|s| s.as_str()).collect();
    assert_eq!(article_titles, HashSet::from(["title4", "title6"]));

    // Verify entry 3 now shows FavoriteOff icon on the index page
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

    // Test unread page (/)
    let req = test::TestRequest::get()
        .uri("/")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();
    let active_category = helpers::find_active_category(content);
    assert_eq!(active_category, Some("unread".to_string()));

    // Test all page (/all)
    let req = test::TestRequest::get()
        .uri("/all")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();
    let active_category = helpers::find_active_category(content);
    assert_eq!(active_category, Some("all".to_string()));

    // Test favourite page (/favourite)
    let req = test::TestRequest::get()
        .uri("/favourite")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();
    let active_category = helpers::find_active_category(content);
    assert_eq!(active_category, Some("favourite".to_string()));

    // Test archive page (/archive)
    let req = test::TestRequest::get()
        .uri("/archive")
        .cookie(cookie.clone())
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();
    let active_category = helpers::find_active_category(content);
    assert_eq!(active_category, Some("archived".to_string()));
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

    // Entry 1: not archived, not starred
    let req = test::TestRequest::get()
        .uri("/article/1")
        .cookie(cookie)
        .to_request();
    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let content = str::from_utf8(&body).unwrap();

    // Title
    assert_eq!(
        helpers::find_article_title(content),
        Some("title1".to_string())
    );

    // Content
    // TODO replace by actual html path searching
    assert!(content.contains("content1"));

    // Domain link to original URL
    let (domain_text, domain_href) = helpers::find_article_domain_link(content).unwrap();
    assert_eq!(domain_text, "a.com");
    assert_eq!(domain_href, "https://a.com/1");

    // Reading time
    // TODO replace by actual html path searching
    assert!(content.contains("8 min read"));

    // Unstarred → FavoriteOff icon
    let (_, fav_icon) = helpers::find_favourite_icons_by_article(content)
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(fav_icon, "/static/images/FavoriteOff.svg");

    // Unarchived → MarkUnRead icon
    let archive_icon = helpers::find_archive_icons(content)
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(archive_icon, "/static/images/MarkUnRead.svg");

    // Delete form present with correct article_id and back_location
    let delete_forms = helpers::find_delete_forms(content);
    assert_eq!(delete_forms.len(), 1);
    assert_eq!(delete_forms[0], "1");
    assert_eq!(
        helpers::find_delete_back_location(content),
        Some("/".to_string())
    );
}

#[sqlx::test(
    migrations = "../../migrations",
    fixtures("../tests/fixtures/users.sql", "../tests/fixtures/entries.sql")
)]
async fn article_page_archived_starred(pool: SqlitePool) {
    let app = init_ui_app(pool).await;
    let cookie = login("wallabag", "wallabag", &app).await;

    // Entry 4: archived, starred
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
        Some("title4".to_string())
    );
    assert!(content.contains("content4"));

    let (domain_text, domain_href) = helpers::find_article_domain_link(content).unwrap();
    assert_eq!(domain_text, "d.com");
    assert_eq!(domain_href, "https://d.com/4");

    assert!(content.contains("15 min read"));

    // Starred → FavoriteOn icon
    let (_, fav_icon) = helpers::find_favourite_icons_by_article(content)
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(fav_icon, "/static/images/FavoriteOn.svg");

    // Archived → MarkRead icon
    let archive_icon = helpers::find_archive_icons(content)
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(archive_icon, "/static/images/MarkRead.svg");

    // Delete form present with correct article_id and back_location
    let delete_forms = helpers::find_delete_forms(content);
    assert_eq!(delete_forms.len(), 1);
    assert_eq!(delete_forms[0], "4");
    assert_eq!(
        helpers::find_delete_back_location(content),
        Some("/".to_string())
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
                    .map(|v| v.to_string())
            })
            .collect()
    }

    /// Returns the article_id values from all archive forms in the page.
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
                    .map(|v| v.to_string())
            })
            .collect()
    }

    /// Returns the article_id values from all delete forms in the page.
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
                    .map(|v| v.to_string())
            })
            .collect()
    }

    /// Returns the back_location value from the delete form, if present.
    pub fn find_delete_back_location(content: &str) -> Option<String> {
        let document = Html::parse_document(content);
        let form_sel = Selector::parse(r#"form[action="/do_delete"]"#).unwrap();
        let input_sel = Selector::parse(r#"input[name="back_location"]"#).unwrap();

        document.select(&form_sel).next().and_then(|form| {
            form.select(&input_sel)
                .next()
                .and_then(|input| input.value().attr("value"))
                .map(|v| v.to_string())
        })
    }

    /// Returns (article_id, icon_src) pairs from all favourite forms in the page.
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
                    .map(|v| v.to_string())?;
                let icon_src = form
                    .select(&img_sel)
                    .next()
                    .and_then(|img| img.value().attr("src"))
                    .map(|v| v.to_string())?;
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
            .filter_map(|el| el.value().attr("href").map(|v| v.to_string()))
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

    /// Returns (domain_text, href) from the domain link on the article detail page.
    pub fn find_article_domain_link(content: &str) -> Option<(String, String)> {
        let document = Html::parse_document(content);
        let link = document
            .select(&Selector::parse(r#"a[target="_blank"]"#).unwrap())
            .next()?;
        let href = link.value().attr("href")?.to_string();
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
                return Some(category.to_string());
            }
        }
        None
    }
}
