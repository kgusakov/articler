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
use handlebars::Handlebars;
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
        .service(ResourceFiles::new("/static", generated))
        .configure(|cfg| crate::web::routes(cfg, cookie_key.clone()))
}

// TODO rethink 'static hardcode
// TODO manual file registration is a bad way
pub fn init_handlebars() -> Handlebars<'static> {
    let mut handlebars = Handlebars::new();
    handlebars
        .register_template_string("index", include_str!("../templates/index.hbs"))
        .unwrap();
    handlebars
        .register_partial("login", include_str!("../templates/login.hbs"))
        .unwrap();
    handlebars
        .register_partial("navigation", include_str!("../templates/navigation.hbs"))
        .unwrap();
    handlebars
        .register_partial("main", include_str!("../templates/main.hbs"))
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
