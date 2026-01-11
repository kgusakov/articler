use std::{
    borrow::Cow,
    rc::Rc,
    sync::{Arc, Once},
};

use actix_web::{
    App,
    cookie::time::UtcDateTime,
    middleware::Logger,
    test,
    web::{self},
};

use chrono::{DateTime, Utc};
use proptest::prelude::*;
use serde_json::Value;
use serde_json_assert::{assert_json_eq, assert_json_include};
use sqlx::SqlitePool;
use urlencoding::encode;
// TODO is it appropriate way?
use wallabag_rs::api::{
    app_state_init, delete_entry, delete_tag_by_label, delete_tag_from_entry, entries, get_tags,
    get_tags_by_entry, patch_entry, post_entries,
};

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

#[sqlx::test(migrations = "./migrations")]
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

    let payload = "url=https://example.com/article&archive=1&starred=1&tags=label 1,label 2&title=New title&content=New content&language=ru&published_at=2023-12-01T11:00:00Z&preview_picture=https://example.com/pic.jpg&authors=author1,author2&public=1&origin_url=https://example.com/origin/url";

    let req = test::TestRequest::post()
        .uri("/api/entries.json")
        .set_payload(payload)
        .insert_header(("content-type", "application/x-www-form-urlencoded"))
        .to_request();

    let before_call_time = Utc::now();
    let resp = test::call_and_read_body(&app, req).await;
    let after_call_time = Utc::now();

    let expected = serde_json::from_str::<Value>(include_str!("json/create_entry.json")).unwrap();

    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert!(result.get("id").unwrap().as_i64().unwrap() >= 0);
    assert!(matches!(result.get("uid").unwrap(), Value::String(s) if !s.is_empty()));

    assert_json_date_between(&before_call_time, &after_call_time, "created_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "updated_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "starred_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "archived_at", &result);

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

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn delete_entry_expect_id(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);
    let app_state = app_state_init(a_pool.clone());
    let entry_rep = app_state.entry_repository.clone();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .wrap(Logger::default())
            .service(delete_entry),
    )
    .await;

    let req = test::TestRequest::delete()
        .uri("/api/entries/1.json?expect=id")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;

    let expected: Value = serde_json::from_str(include_str!("json/delete_entry_id.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );

    assert!(
        entry_rep.find_by_id(1).await.unwrap().is_none(),
        "Entry should be deleted from database"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn delete_entry_expect_full(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);
    let app_state = app_state_init(a_pool.clone());
    let entry_rep = app_state.entry_repository.clone();

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state))
            .wrap(Logger::default())
            .service(delete_entry),
    )
    .await;

    let req = test::TestRequest::delete()
        .uri("/api/entries/2.json?expect=full")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;

    let expected: Value =
        serde_json::from_str(include_str!("json/delete_entry_full.json")).unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );

    assert!(
        entry_rep.find_by_id(2).await.unwrap().is_none(),
        "Entry should be deleted from database"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn delete_entry_not_found(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(delete_entry),
    )
    .await;

    let req = test::TestRequest::delete()
        .uri("/api/entries/999.json?expect=id")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        404,
        "Should return 404 for non-existent entry"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn patch_entry_basic_fields(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(patch_entry),
    )
    .await;

    let payload = r#"{"title":"Updated Title","content":"Updated Content","language":"fr"}"#;

    let req = test::TestRequest::patch()
        .uri("/api/entries/1.json")
        .set_payload(payload)
        .insert_header(("content-type", "application/json"))
        .to_request();

    let before_call_time = Utc::now();
    let resp = test::call_and_read_body(&app, req).await;
    let after_call_time = Utc::now();

    let expected =
        serde_json::from_str::<Value>(include_str!("json/patch_entry_basic.json")).unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_json_date_between(&before_call_time, &after_call_time, "updated_at", &result);

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn patch_entry_archive_and_star(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(patch_entry),
    )
    .await;

    // Archive and star entry 1 (which is not archived and not starred)
    let payload = r#"{"archive":1,"starred":1}"#;

    let req = test::TestRequest::patch()
        .uri("/api/entries/1.json")
        .set_payload(payload)
        .insert_header(("content-type", "application/json"))
        .to_request();

    let before_call_time = Utc::now();
    let resp = test::call_and_read_body(&app, req).await;
    let after_call_time = Utc::now();

    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    // TODO if entry already has 1 in these fields - test tests nothing
    // TODO early the main design goal was to test the whole json
    assert_eq!(result.get("is_archived").unwrap().as_i64().unwrap(), 1);
    assert_eq!(result.get("is_starred").unwrap().as_i64().unwrap(), 1);

    assert_json_date_between(&before_call_time, &after_call_time, "updated_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "archived_at", &result);
    assert_json_date_between(&before_call_time, &after_call_time, "starred_at", &result);
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn patch_entry_unarchive_and_unstar(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(patch_entry),
    )
    .await;

    // Unarchive and unstar entry 4 (which is archived and starred)
    let payload = r#"{"archive":0,"starred":0}"#;

    let req = test::TestRequest::patch()
        .uri("/api/entries/4.json")
        .set_payload(payload)
        .insert_header(("content-type", "application/json"))
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    // TODO if entry already has 1 in these fields - test tests nothing
    assert_eq!(result.get("is_archived").unwrap().as_i64().unwrap(), 0);
    assert_eq!(result.get("is_starred").unwrap().as_i64().unwrap(), 0);
    assert!(result.get("archived_at").unwrap().is_null());
    assert!(result.get("starred_at").unwrap().is_null());
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn patch_entry_add_tags(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(patch_entry),
    )
    .await;

    // Add tags to entry 1 (which has no tags)
    let payload = r#"{"tags":"newtag1,newtag2"}"#;

    let req = test::TestRequest::patch()
        .uri("/api/entries/1.json")
        .set_payload(payload)
        .insert_header(("content-type", "application/json"))
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/patch_entry_add_tags.json")).unwrap();

    assert_json_include!(
        actual: serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap(),
        expected: expected,
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn patch_entry_replace_tags(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(patch_entry),
    )
    .await;

    // Replace tags on entry 2 (which has label1 and label2)
    let payload = r#"{"tags":"label3,newtag"}"#;

    let req = test::TestRequest::patch()
        .uri("/api/entries/2.json")
        .set_payload(payload)
        .insert_header(("content-type", "application/json"))
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/patch_entry_replace_tags.json")).unwrap();

    assert_json_include!(
        actual: serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap(),
        expected: expected,
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn patch_entry_remove_all_tags(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(patch_entry),
    )
    .await;

    // Remove all tags from entry 2 (which has label1 and label2)
    let payload = r#"{"tags":""}"#;

    let req = test::TestRequest::patch()
        .uri("/api/entries/2.json")
        .set_payload(payload)
        .insert_header(("content-type", "application/json"))
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert!(result.get("tags").unwrap().as_array().unwrap().is_empty());
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn patch_entry_not_found(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(patch_entry),
    )
    .await;

    let payload = r#"{"title":"Updated"}"#;

    let req = test::TestRequest::patch()
        .uri("/api/entries/999.json")
        .set_payload(payload)
        .insert_header(("content-type", "application/json"))
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        404,
        "Should return 404 for non-existent entry"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn patch_entry_make_public(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(patch_entry),
    )
    .await;

    // Make entry 1 public (which is not public)
    let payload = r#"{"public":1}"#;

    let req = test::TestRequest::patch()
        .uri("/api/entries/1.json")
        .set_payload(payload)
        .insert_header(("content-type", "application/json"))
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_eq!(result.get("is_public").unwrap().as_bool().unwrap(), true);
    assert!(matches!(result.get("uid").unwrap(), Value::String(s) if !s.is_empty()));
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn get_tags_for_entry_with_tags(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(get_tags_by_entry),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/entries/2/tags")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/get_tags_for_entry_with_tags.json"))
            .unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn get_tags_for_entry_without_tags(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(get_tags_by_entry),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/entries/1/tags")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/get_tags_for_entry_without_tags.json"))
            .unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn get_tags_for_nonexistent_entry(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(get_tags_by_entry),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/api/entries/999/tags")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        404,
        "Should return 404 for non-existent entry"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn test_get_all_tags(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(get_tags),
    )
    .await;

    let req = test::TestRequest::get().uri("/api/tags").to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected = serde_json::from_str::<Value>(include_str!("json/get_all_tags.json")).unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_get_all_tags_empty(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(get_tags),
    )
    .await;

    let req = test::TestRequest::get().uri("/api/tags").to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    let tags = result.as_array().unwrap();
    assert_eq!(
        tags.len(),
        0,
        "Should return empty array when no tags exist"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn delete_tag_from_entry_success(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(delete_tag_from_entry),
    )
    .await;

    // Delete tag_id=1 (label1) from entry 2
    let req = test::TestRequest::delete()
        .uri("/api/entries/2/tags/1")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/delete_tag_from_entry_success.json"))
            .unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn delete_nonexistent_tag_from_entry(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(delete_tag_from_entry),
    )
    .await;

    // Try to delete non-existent tag_id=999 from entry 2
    let req = test::TestRequest::delete()
        .uri("/api/entries/2/tags/999")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/delete_tag_from_entry_unchanged.json"))
            .unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    // Entry should be returned unchanged with both original tags
    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn delete_tag_from_nonexistent_entry(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(delete_tag_from_entry),
    )
    .await;

    // Try to delete tag from non-existent entry 999
    let req = test::TestRequest::delete()
        .uri("/api/entries/999/tags/1")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(
        resp.status(),
        404,
        "Should return 404 for non-existent entry"
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn delete_tag_by_label_success(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(delete_tag_by_label),
    )
    .await;

    // Delete tag with label "label1"
    let req = test::TestRequest::delete()
        .uri("/api/tags/label.json?tag=label1")
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    let expected =
        serde_json::from_str::<Value>(include_str!("json/delete_tag_by_label_success.json"))
            .unwrap();
    let result = serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap();

    assert_json_include!(
        actual: result,
        expected: expected
    );
}

#[sqlx::test(migrations = "./migrations", fixtures("entries"))]
async fn delete_nonexistent_tag_by_label(pool: SqlitePool) {
    init();

    let a_pool = Arc::new(pool);

    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(app_state_init(a_pool.clone())))
            .wrap(Logger::default())
            .service(delete_tag_by_label),
    )
    .await;

    // Try to delete non-existent tag
    let req = test::TestRequest::delete()
        .uri("/api/tags/label.json?tag=nonexistent")
        .to_request();

    let resp = test::call_service(&app, req).await;

    assert_eq!(resp.status(), 404, "Should return 404 for non-existent tag");
}
