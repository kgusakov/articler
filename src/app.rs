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
    scrapper::Scrapper,
    storage::{
        repository::{
            ClientRepository, EntryRepository, SqliteClientRepository, SqliteEntryRepository,
            SqliteTagRepository, SqliteUserRepository, TagRepository, UserRepository,
        },
        token_storage::TokenStorage,
    },
};

pub struct AppState {
    pub tag_repository: Arc<dyn TagRepository>,
    pub entry_repository: Arc<dyn EntryRepository>,
    pub user_repository: Arc<dyn UserRepository>,
    pub client_repository: Arc<dyn ClientRepository>,
    pub token_storage: TokenStorage,
    pub scrapper: Scrapper,
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
        .configure(|cfg| crate::http::oauth::routes(cfg))
        .configure(|cfg| crate::http::api::routes(cfg))
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

pub fn app_state_init(pool: Pool<Sqlite>, scrapper: Scrapper) -> AppState {
    let tag_repo = Arc::new(SqliteTagRepository::new(pool.clone()));

    AppState {
        tag_repository: tag_repo.clone(),
        entry_repository: Arc::new(SqliteEntryRepository::new(pool.clone(), tag_repo.clone())),
        user_repository: Arc::new(SqliteUserRepository::new(pool.clone())),
        client_repository: Arc::new(SqliteClientRepository::new(pool)),
        token_storage: TokenStorage::default(),
        scrapper: scrapper,
    }
}
