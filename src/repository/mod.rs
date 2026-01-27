pub mod users;
pub mod clients;
pub mod entries;
pub mod tags;

use sqlx::Error as SqlxError;
use sqlx::Sqlite;
use thiserror::Error;

pub type Db = Sqlite;
type Result<T> = std::result::Result<T, DbError>;
type Id = i64;
type Timestamp = i64;
type ReadingTime = i32;

const ENTRIES_TABLE: &str = "entries";
const TAGS_TABLE: &str = "tags";
const ENTRIES_TAG_TABLE: &str = "entry_tags";
const USERS_TABLE: &str = "users";
const CLIENTS_TABLE: &str = "clients";
const SQLITE_LIMIT_VARIABLE_NUMBER: usize = 999;

#[derive(Error, Debug)]
pub enum DbError {
    // TODO produce ugly wrapped SqliteError(Database(SqliteError { code: 1, message: "no such column: et.tag_id" }))
    #[error(transparent)]
    SqliteRepositoryError(#[from] SqlxError),
    #[error("Repository error: {0}")]
    RepositoryError(String),
}
