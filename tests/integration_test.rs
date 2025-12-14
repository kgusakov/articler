use std::sync::Arc;

use actix_web::{
    App,
    http::{StatusCode, header::ContentType},
    middleware::Logger,
    test, web,
};

use sqlx::SqlitePool;
// TODO is it appropriate way?
use wallabag_rs::{
    api::{app_state_init, entries},
    storage::repository::{EntryRepository, SqliteEntryRepository, SqliteTagRepository},
};

// TODO should be executed once before tests
fn init() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("trace"));
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

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn get_entries_from_db(pool: SqlitePool) -> sqlx::Result<()> {
    let row: (String,) = sqlx::query_as("SELECT * from entries")
        .fetch_one(&pool)
        .await?;

    assert_eq!(row.0, "papa");
    Ok(())
}
