use std::sync::{Arc, Once};

use actix_web::{
    App,
    middleware::Logger,
    test,
    web::{self},
};

use serde_json::Value;
use serde_json_assert::assert_json_eq;
use sqlx::SqlitePool;
// TODO is it appropriate way?
use wallabag_rs::api::{app_state_init, entries};

static INIT: Once = Once::new();

fn init() {
    INIT.call_once(|| {
        env_logger::init_from_env(env_logger::Env::new().default_filter_or("trace"));
    });
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
