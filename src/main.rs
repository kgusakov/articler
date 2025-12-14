use std::{env, sync::Arc};

mod api;
// TODO research why we need it here
mod models;
mod storage;

use crate::api::app_state_init;
use sqlx::sqlite::SqlitePoolOptions;

use crate::api::http_server;

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let db_path = "sqlite://";
    let pool = Arc::new(SqlitePoolOptions::new().connect(db_path).await?);

    let port = env::var("HTTP_PORT")
        .expect("Set HTTP_PORT env variable")
        .parse::<u16>()
        .expect("HTTP_PORT must be valid port number");

    http_server(port, app_state_init(pool.clone()))?.await?;

    pool.close().await;

    Ok(())
}
