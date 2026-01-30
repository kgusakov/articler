use actix_service::ServiceFactory;
use actix_web::{
    App, HttpServer,
    body::MessageBody,
    cookie::Key,
    dev::{Server, ServiceRequest, ServiceResponse},
    middleware::{Logger, from_fn},
    web,
};
use sqlx::{Pool, Sqlite};

use crate::{middleware::wrap_with_tx, repository, scraper::Scraper, token_storage::TokenStorage};

pub struct AppState {
    // TODO web::Data is an Arc itself. Looks like these arcs must be deleted
    // TODO use more generic database type here
    pub pool: Pool<repository::Db>,
    pub token_storage: TokenStorage,
    pub scraper: Scraper,
}

pub fn app(
    app_data: web::Data<AppState>,
    cookie_key: Key,
) -> App<
    impl ServiceFactory<
        ServiceRequest,
        Config = (),
        Response = ServiceResponse<impl MessageBody>,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    App::new()
        .app_data(app_data.clone())
        .wrap(Logger::default())
        .wrap(from_fn(wrap_with_tx))
        .configure(crate::http::oauth::routes)
        .configure(crate::http::api::routes)
        .configure(|cfg| crate::http::fake_ui::routes(cfg, cookie_key))
}

pub fn http_server(port: u16, app_state: AppState, cookie_key: Key) -> std::io::Result<Server> {
    let app_data = web::Data::new(app_state);

    // TODO looks like it is created multiple times as a result - need to check
    Ok(
        HttpServer::new(move || app(app_data.clone(), cookie_key.clone()))
            .bind(format!("0.0.0.0:{}", port))?
            .run(),
    )
}

pub fn app_state_init(pool: Pool<Sqlite>, scraper: Scraper) -> AppState {
    AppState {
        pool,
        token_storage: TokenStorage::default(),
        scraper,
    }
}
