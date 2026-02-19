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
use handlebars::{Handlebars, TemplateError};
use sqlx::{Pool, Sqlite};

use crate::{middleware::wrap_with_tx, scraper::Scraper, token_storage::TokenStorage};
use db::repository;

#[expect(clippy::module_name_repetitions)]
pub struct AppState {
    pub pool: Pool<repository::Db>,
    pub token_storage: TokenStorage,
    pub scraper: Scraper,
    pub handlebars: Handlebars<'static>,
}

impl AppState {
    #[must_use]
    pub fn new(pool: Pool<Sqlite>, scraper: Scraper, handlebars: Handlebars<'static>) -> Self {
        Self {
            pool,
            token_storage: TokenStorage::default(),
            scraper,
            handlebars,
        }
    }
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
        .app_data(app_data)
        .wrap(Logger::default())
        .wrap(from_fn(wrap_with_tx))
        .configure(crate::rest::oauth::routes)
        .configure(crate::rest::wallabag::routes)
        .service(ResourceFiles::new("/static", generated))
        .configure(|cfg| crate::web::routes(cfg, cookie_key))
}

// TODO rethink 'static hardcode
// TODO manual file registration is a bad way
pub fn init_handlebars() -> Result<Handlebars<'static>, TemplateError> {
    let mut handlebars = Handlebars::new();
    handlebars.register_template_string("index", include_str!("../templates/index.hbs"))?;
    handlebars.register_template_string(
        "fake_development",
        include_str!("../templates/fake_development.hbs"),
    )?;
    handlebars.register_partial("login", include_str!("../templates/login.hbs"))?;
    handlebars.register_partial("navigation", include_str!("../templates/navigation.hbs"))?;
    handlebars.register_partial("main", include_str!("../templates/main.hbs"))?;
    handlebars.register_partial("article", include_str!("../templates/article.hbs"))?;
    handlebars.register_partial("article_cards", include_str!("../templates/article_cards.hbs"))?;
    handlebars.register_partial("clients", include_str!("../templates/clients.hbs"))?;
    Ok(handlebars)
}

pub fn http_server(port: u16, app_state: AppState, cookie_key: Key) -> std::io::Result<Server> {
    let app_data = web::Data::new(app_state);

    Ok(
        HttpServer::new(move || app(app_data.clone(), cookie_key.clone()))
            .bind(format!("0.0.0.0:{port}"))?
            .run(),
    )
}
