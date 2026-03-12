use actix_service::ServiceFactory;
use actix_web::{
    App, HttpServer,
    body::MessageBody,
    cookie::Key,
    dev::{Server, ServiceRequest, ServiceResponse},
    middleware::Logger,
    web,
};
use actix_web_static_files::ResourceFiles;
use handlebars::{Handlebars, TemplateError};

use app_state::AppState;

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
        .configure(wallabag_api::oauth::routes)
        .configure(wallabag_api::wallabag::routes)
        .service(ResourceFiles::new("/static", generated))
        .configure(|cfg| crate::web::routes(cfg, cookie_key))
}

// TODO rethink 'static hardcode
// TODO manual file registration is a bad way
pub fn init_handlebars() -> Result<Handlebars<'static>, TemplateError> {
    let mut handlebars = Handlebars::new();

    handlebars.register_partial("base", include_str!("../templates/base.hbs"))?;
    handlebars.register_partial("page", include_str!("../templates/page.hbs"))?;
    handlebars.register_partial("navigation", include_str!("../templates/navigation.hbs"))?;
    handlebars.register_partial("categories", include_str!("../templates/categories.hbs"))?;
    handlebars.register_partial("article", include_str!("../templates/article.hbs"))?;
    handlebars.register_partial(
        "article_cards",
        include_str!("../templates/article_cards.hbs"),
    )?;
    handlebars.register_partial(
        "articles_and_categories",
        include_str!("../templates/articles_and_categories.hbs"),
    )?;
    handlebars.register_partial("clients", include_str!("../templates/clients.hbs"))?;

    handlebars.register_template_string("login", include_str!("../templates/login.hbs"))?;
    handlebars
        .register_template_string("page_articles", include_str!("../templates/page_articles.hbs"))?;
    handlebars
        .register_template_string("page_article", include_str!("../templates/page_article.hbs"))?;
    handlebars
        .register_template_string("page_clients", include_str!("../templates/page_clients.hbs"))?;

    handlebars.register_template_string("article", include_str!("../templates/article.hbs"))?;
    handlebars.register_template_string(
        "article_cards",
        include_str!("../templates/article_cards.hbs"),
    )?;
    handlebars.register_template_string(
        "articles_and_categories",
        include_str!("../templates/articles_and_categories.hbs"),
    )?;
    handlebars.register_template_string(
        "categories",
        include_str!("../templates/categories.hbs"),
    )?;

    handlebars.register_template_string(
        "fake_development",
        include_str!("../templates/fake_development.hbs"),
    )?;
    handlebars.register_template_string(
        "fake_client_create_result",
        include_str!("../templates/fake_client_create_result.hbs"),
    )?;
    handlebars.register_template_string(
        "fake_client_create",
        include_str!("../templates/fake_client_create.hbs"),
    )?;

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
