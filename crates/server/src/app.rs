use actix_service::ServiceFactory;
use actix_web::{
    App, HttpServer,
    body::MessageBody,
    cookie::Key,
    dev::{Server, ServiceRequest, ServiceResponse},
    middleware::{Logger, from_fn},
    web,
};
use actix_web_static_files::ResourceFiles;
use handlebars::{DirectorySourceOptionsBuilder, Handlebars};
use sqlx::{Pool, Sqlite};

use crate::{middleware::wrap_with_tx, scraper::Scraper, token_storage::TokenStorage};
use db::repository;

pub struct AppState {
    pub pool: Pool<repository::Db>,
    pub token_storage: TokenStorage,
    pub scraper: Scraper,
    pub handlebars: Handlebars<'static>,
}

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

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
    let generated = static_resources();
    App::new()
        .app_data(app_data.clone())
        .wrap(Logger::default())
        .wrap(from_fn(wrap_with_tx))
        .configure(crate::rest::oauth::routes)
        .configure(crate::rest::wallabag::routes)
        .configure(|cfg| crate::web::routes(cfg, cookie_key.clone()))
        .service(ResourceFiles::new("/static", generated))
}

// TODO rethink 'static hardcode
pub fn init_handlebars() -> Handlebars<'static> {
    let mut handlebars = Handlebars::new();
    handlebars
        .register_templates_directory(
            "./templates",
            DirectorySourceOptionsBuilder::default()
                .tpl_extension(".html")
                .hidden(false)
                .temporary(false)
                .build()
                .unwrap(),
        )
        .unwrap();

    handlebars
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

pub fn app_state_init(
    pool: Pool<Sqlite>,
    scraper: Scraper,
    handlebars: Handlebars<'static>,
) -> AppState {
    AppState {
        pool,
        token_storage: TokenStorage::default(),
        scraper,
        handlebars,
    }
}
