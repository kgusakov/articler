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
use serde_json_assert::assert_json_eq;
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

    let expected: Value = serde_json::from_str(
        r#"
{
   "page":1,
   "limit":30,
   "pages":1,
   "total":1,
   "embedded":{
      "items":[
         {
            "id":1,
            "url":"https://a.com/1",
            "hashed_url":"hash1",
            "given_url":"https://a.com/g1",
            "hashed_given_url":"ghash1",
            "title":"title1",
            "content":"content1",
            "is_archived":0,
            "archived_at":null,
            "is_starred":1,
            "starred_at":"2023-12-10T15:00:00Z",
            "tags":[
               {
                  "id":1,
                  "label":"label1",
                  "slug":"slug1"
               },
               {
                  "id":2,
                  "label":"label2",
                  "slug":"slug2"
               },
               {
                  "id":3,
                  "label":"label3",
                  "slug":"slug3"
               },
               {
                  "id":4,
                  "label":"label4",
                  "slug":"slug4"
               }
            ],
            "created_at":"2023-12-01T11:00:00Z",
            "updated_at":"2023-12-10T15:00:00Z",
            "annotations":null,
            "mimetype":"text/html",
            "language":"en",
            "reading_time":8,
            "domain_name":"a.com",
            "preview_picture":"https://a.com/pic1.jpg",
            "origin_url":"https://a.com/o1",
            "published_at":"2023-12-01T10:00:00Z",
            "published_by":"author1",
            "is_public":false,
            "uid":null
         }
      ]
   },
   "_links":{
      "self":{
         "href":"http://example.com/"
      },
      "first":{
         "href":"http://example.com/"
      },
      "last":{
         "href":"http://example.com/"
      },
      "next":{
         "href":"http://example.com/"
      }
   }
}
    "#,
    )
    .unwrap();

    assert_json_eq!(
        expected,
        serde_json::from_str::<Value>(str::from_utf8(&resp).unwrap()).unwrap()
    );
}
