use std::sync::{Arc, Once};

use actix_web::{
    App,
    cookie::time::UtcDateTime,
    middleware::Logger,
    test,
    web::{self},
};

use chrono::{DateTime, Utc};
use serde_json::Value;
use serde_json_assert::{assert_json_eq, assert_json_include};
use sqlx::SqlitePool;
// TODO is it appropriate way?
use wallabag_rs::api::{app_state_init, entries, post_entries};

static INIT: Once = Once::new();

fn init() {
    INIT.call_once(|| {
        env_logger::init_from_env(env_logger::Env::new().default_filter_or("trace"));
    });
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn get_entries_json(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(entries),
    )
    .await;

    let req = test::TestRequest::default()
        .uri("/api/entries.json")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;

    let expected: Value = serde_json::from_str(include_str!("json/entries.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn get_entries(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(entries),
    )
    .await;

    let req = test::TestRequest::default()
        .uri("/api/entries")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;

    let expected: Value = serde_json::from_str(include_str!("json/entries.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn get_entries_ordered_by_updated_at(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(entries),
    )
    .await;

    let req = test::TestRequest::default()
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

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn get_entries_with_pages(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(entries),
    )
    .await;

    let req = test::TestRequest::default()
        .uri("/api/entries?page=2&perPage=1")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    dbg!(serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap());

    let expected: Value = serde_json::from_str(include_str!("json/entries_paging.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn get_entries_archived(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(entries),
    )
    .await;

    let req = test::TestRequest::default()
        .uri("/api/entries?archive=1")
        .to_request();

    dbg!(&req.uri());

    let resp = test::call_and_read_body(&app, req).await;

    let expected: Value = serde_json::from_str(include_str!("json/archived_entries.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn get_entries_starred(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(entries),
    )
    .await;

    let req = test::TestRequest::default()
        .uri("/api/entries?starred=1")
        .to_request();

    dbg!(&req.uri());

    let resp = test::call_and_read_body(&app, req).await;

    let expected: Value = serde_json::from_str(include_str!("json/starred_entries.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn get_entries_public(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(entries),
    )
    .await;

    let req = test::TestRequest::default()
        .uri("/api/entries?public=1")
        .to_request();

    dbg!(&req.uri());

    let resp = test::call_and_read_body(&app, req).await;

    let expected: Value = serde_json::from_str(include_str!("json/public_entries.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn test_post_entries(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(post_entries),
    )
    .await;

    let req = test::TestRequest::post()
        .uri("/api/entries.json")
        .set_payload("url=https://example.com/article&archive=0&starred=0")
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .to_request();

    let before_call_time = Utc::now();
    let resp = test::call_and_read_body(&app, req).await;
    let after_call_time = Utc::now();

    let expected = serde_json::from_str::<Value>(include_str!("json/create_entry.json")).unwrap();

    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert!(result.get("id").unwrap().as_i64().unwrap() >= 0);

    assert_json_date_between(&before_call_time, &after_call_time, "created_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "updated_at", &result);

    assert_json_include!(
        actual: result,
        expected: expected
    );
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
