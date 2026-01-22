use std::{env, sync::Arc};

mod api;
// TODO research why we need it here
mod fake_ui;
mod helpers;
mod models;
mod oauth;
mod scrapper;
mod storage;

use actix_web::cookie::Key;
use sqlx::sqlite::SqlitePoolOptions;
use wallabag_rs::scrapper::Scrapper;
use wallabag_rs::{app_state_init, http_server};

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let db_path = env::var("DATABASE_URL").expect("Environment variable DATABASE_URL is not set");
    let cookie_key = env::var("COOKIE_KEY").expect("Environment variable COOKIE_KEY is not set");
    let proxy_scheme = env::var("ALL_PROXY").ok();

    let pool = SqlitePoolOptions::new().connect(&db_path).await?;
    let scrapper = Scrapper::new(proxy_scheme).expect("Scrapper can't be initialized");

    let port = env::var("HTTP_PORT")
        .expect("Set HTTP_PORT env variable")
        .parse::<u16>()
        .expect("HTTP_PORT must be valid port number");

    http_server(
        port,
        app_state_init(pool.clone(), scrapper),
        Key::from(cookie_key.as_bytes()),
    )?
    .await?;

    pool.close().await;

    Ok(())
}
