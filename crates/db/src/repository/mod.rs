pub mod clients;
pub mod entries;
pub mod tags;
pub mod tokens;
pub mod users;

use sqlx::Sqlite;
use thiserror::Error;

pub type Db = Sqlite;
pub type Id = i64;
type Timestamp = i64;
pub type ReadingTime = i32;

const ENTRIES_TABLE: &str = "entries";
const TAGS_TABLE: &str = "tags";
const ENTRIES_TAG_TABLE: &str = "entry_tags";
const USERS_TABLE: &str = "users";
const CLIENTS_TABLE: &str = "clients";
const TOKENS_TABLE: &str = "tokens";
const SQLITE_LIMIT_VARIABLE_NUMBER: usize = 999;

#[derive(Error, Debug)]
pub enum DbErrorType {
    #[error("Repository error: {0}")]
    RepositoryError(String),
}
