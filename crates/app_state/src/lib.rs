pub use token_storage::{Claim, NewToken, TokenStorage};

use article_scraper::Scraper;
use db::repository;
use handlebars::Handlebars;
use sqlx::{Pool, Sqlite};

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
