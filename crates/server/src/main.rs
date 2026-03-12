use std::env;
use std::str::FromStr;

use actix_web::cookie::Key;
use app_state::AppState;
use article_scraper::Scraper;
use server::app::{http_server, init_handlebars};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

use server::error::Result;

#[actix_web::main]
async fn main() -> Result<()> {
    env_logger::init();

    let db_path = env::var("DATABASE_URL").expect("Environment variable DATABASE_URL is not set");
    let cookie_key = env::var("COOKIE_KEY").expect("Environment variable COOKIE_KEY is not set");

    let proxy_scheme = match env::var("ALL_PROXY") {
        Ok(p) if !p.is_empty() => Some(p),
        _ => None,
    };

    let connect_options = SqliteConnectOptions::from_str(&db_path)?
        .journal_mode(SqliteJournalMode::Wal)
        .pragma("trusted_schema", "OFF");

    let pool = SqlitePoolOptions::new()
        .connect_with(connect_options)
        .await?;

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
        AppState::new(
            pool.clone(),
            scraper,
            init_handlebars().expect("Handlebars init was failed"),
        ),
        Key::from(cookie_key.as_bytes()),
    )?
    .await?;

    pool.close().await;

    Ok(())
}
