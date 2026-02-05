use std::env;

use clap::{Parser, Subcommand};
use cli::create_user;
use sqlx::sqlite::SqlitePoolOptions;
use tokio::runtime::Runtime;

#[derive(Parser)]
#[command(about = "Command-line interface for articler", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    CreateUser {
        username: String,
        password: String,
        name: String,
        email: String,
    },
}

fn main() -> result::ArticlerResult<()> {
    let rt = Runtime::new()?;
    let db_path = env::var("DATABASE_URL").expect("Environment variable DATABASE_URL is not set");
    let pool = rt.block_on(async { SqlitePoolOptions::new().connect(&db_path).await })?;

    let cli = Cli::parse();

    match cli.command {
        Commands::CreateUser {
            username,
            password,
            name,
            email,
        } => {
            rt.block_on(async { create_user(&pool, &username, &password, &name, &email).await })?
        }
    }

    Ok(())
}
