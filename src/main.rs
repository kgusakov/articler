use std::{env, sync::LazyLock};

mod api;
// TODO research why we need it here
mod models;

use actix_web::{
    App, HttpServer,
    web::{self},
};

use crate::api::entries;

// TODO LazyLock looks like not so elegant solution
static HTTP_PORT: LazyLock<u16> = LazyLock::new(|| {
    env::var("HTTP_PORT")
        .expect("Set HTTP_PORT env variable")
        .parse::<u16>()
        .expect("HTTP_PORT must be valid port number")
});

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    HttpServer::new(move || App::new().service(web::scope("/").service(entries)))
        .bind(format!("0.0.0.0:{}", *HTTP_PORT))?
        .run()
        .await
}
