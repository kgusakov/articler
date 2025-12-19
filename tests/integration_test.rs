use std::sync::Arc;

use actix_web::{
    App,
    body::MessageBody,
    http::{StatusCode, header::ContentType},
    middleware::Logger,
    test,
    web::{self, Bytes},
};

use serde_json::Value;
use sqlx::SqlitePool;
// TODO is it appropriate way?
use wallabag_rs::{
    api::{Entries, EntriesRequest, app_state_init, entries},
    storage::repository::{
        AllEntriesParams, EntryRepository, SqliteEntryRepository, SqliteTagRepository,
    },
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
        .set_json(
            r#"{
                    "archive":1,
                    "starred":0,
                    "sort":"created",
                    "order":"asc",
                    "page":0,
                    "perPage":10,
                    "tags":"api,rest",
                    "since":0,
                    "public":1,
                    "detail":"full",
                    "domain_name":"example.com"
                    }"#,
        )
        .to_request();

    let resp = test::call_and_read_body(&app, req).await;
    // assert_eq!(resp.status(), StatusCode::OK);

    println!("{}", str::from_utf8(&resp).unwrap());

    // let json_body = resp.response().body().try_into_bytes().unwrap();
    let expected: Value = serde_json::from_str(r#"
    {
        "page":1,
        "limit":30,
        "pages":1,
        "total":1,
        "embedded":{
            "items":[
                {
                    "id":1,
                    "url":"https://example.com/article/rust-web-backend/url",
                    "hashed_url":"a3f5e8d9c2b1a0f4e7d6c5b4a3f2e1d0",
                    "given_url":"https://example.com/article/rust-web-backend/given_url",
                    "hashed_given_url":"a3f5e8d9c2b1a0f4e7d6c5b4a3f2e1d0",
                    "title":"Building Web Backends with Rust",
                    "content":"This comprehensive guide covers building modern web backends using Rust, actix-web, and sqlx. Learn about async programming, database integration, and best practices for production-ready applications.",
                    "is_archived":0,
                    "archived_at":null,
                    "is_starred":1,
                    "starred_at":"2023-12-10T15:00:00Z",
                    "tags":[
                    {
                        "id":1,
                        "label":"Rust",
                        "slug":"rust"
                    },
                    {
                        "id":2,
                        "label":"Web Development",
                        "slug":"web-development"
                    },
                    {
                        "id":3,
                        "label":"Backend",
                        "slug":"backend"
                    },
                    {
                        "id":4,
                        "label":"Tutorial",
                        "slug":"tutorial"
                    }
                    ],
                    "created_at":"2023-12-01T11:00:00Z",
                    "updated_at":"2023-12-10T15:00:00Z",
                    "annotations":null,
                    "mimetype":"text/html",
                    "language":"en",
                    "reading_time":8,
                    "domain_name":"example.com",
                    "preview_picture":"https://example.com/images/rust-backend-preview.jpg",
                    "origin_url":"https://example.com/article/rust-web-backend/origin",
                    "published_at":"2023-12-01T10:00:00Z",
                    "published_by":"John Doe",
                    "is_public":false,
                    "uid":null
                }
            ]
        },
        "_links":{
            "self": {
                "href":"http://example.com/"
            },
            "first": {
                "href":"http://example.com/"
            },
            "last": {
                "href":"http://example.com/"
            },
            "next": {
                "href":"http://example.com/"
            }
        }
    }"#).unwrap();
    assert_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}
