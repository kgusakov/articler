use std::sync::Arc;

use actix_service::ServiceFactory;
use actix_web::{
    App, HttpServer,
    body::MessageBody,
    cookie::Key,
    dev::{Server, ServiceRequest, ServiceResponse},
    middleware::Logger,
    web,
};
use sqlx::{Pool, Sqlite};

use crate::{
    repository,
    scraper::Scraper,
    storage::{
        repository::{
            ClientRepository, EntryRepository, SqliteClientRepository, SqliteEntryRepository,
            SqliteTagRepository, TagRepository,
        },
        token_storage::TokenStorage,
    },
};

pub struct AppState {
    // TODO web::Data is an Arc itself. Looks like these arcs must be deleted
    // TODO use more generic database type here
    pub pool: Pool<repository::Db>,
    pub tag_repository: Arc<dyn TagRepository>,
    pub entry_repository: Arc<dyn EntryRepository>,
    pub client_repository: Arc<dyn ClientRepository>,
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
    let tag_repo = Arc::new(SqliteTagRepository::new(pool.clone()));

    AppState {
        pool: pool.clone(),
        tag_repository: tag_repo.clone(),
        entry_repository: Arc::new(SqliteEntryRepository::new(pool.clone(), tag_repo.clone())),
        client_repository: Arc::new(SqliteClientRepository::new(pool)),
        token_storage: TokenStorage::default(),
        scraper,
    }
}
