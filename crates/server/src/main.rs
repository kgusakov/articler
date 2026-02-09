use std::env;

use actix_web::cookie::Key;
use result::ArticlerResult;
use server::{
    app::{app_state_init, http_server, init_handlebars},
    scraper::Scraper,
};
use sqlx::sqlite::SqlitePoolOptions;

#[actix_web::main]
async fn main() -> ArticlerResult<()> {
    env_logger::init();

    let db_path = env::var("DATABASE_URL").expect("Environment variable DATABASE_URL is not set");
    let cookie_key = env::var("COOKIE_KEY").expect("Environment variable COOKIE_KEY is not set");

    let proxy_scheme = match env::var("ALL_PROXY") {
        Ok(p) if !p.is_empty() => Some(p),
        _ => None,
    };

    let pool = SqlitePoolOptions::new().connect(&db_path).await?;

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("Failed to run database migrations");

    let scraper = Scraper::new(proxy_scheme.as_deref()).expect("Scraper can't be initialized");

    let port = env::var("HTTP_PORT")
        .expect("Set HTTP_PORT env variable")
        .parse::<u16>()
        .expect("HTTP_PORT must be valid port number");

    http_server(
        port,
        app_state_init(pool.clone(), scraper, init_handlebars()),
        Key::from(cookie_key.as_bytes()),
    )?
    .await?;

    pool.close().await;

    Ok(())
}
