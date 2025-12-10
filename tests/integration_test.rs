use actix_web::{
    App,
    http::{StatusCode, header::ContentType},
    middleware::Logger,
    test,
};

// TODO is it appropriate way?
use wallabag_rs::api::entries;

#[actix_web::test]
async fn get_entries() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("trace"));
    let app = test::init_service(App::new().wrap(Logger::default()).service(entries)).await;

    let req = test::TestRequest::default()
        .uri("/api/entries")
        .to_request();

    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}
