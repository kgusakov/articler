use snafu::{Location, Snafu};
use types::ValidationError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(transparent)]
    Io {
        #[snafu(source)]
        error: std::io::Error,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(transparent)]
    Sqlx {
        #[snafu(source)]
        error: sqlx::Error,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(transparent)]
    UrlParse {
        #[snafu(source)]
        error: url::ParseError,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(transparent)]
    Db {
        #[snafu(source)]
        error: db::error::Error,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(transparent)]
    Helper {
        #[snafu(source)]
        error: helpers::error::Error,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(transparent)]
    Scraper {
        #[snafu(source)]
        error: article_scraper::error::Error,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(transparent)]
    Validation {
        #[snafu(source)]
        error: ValidationError,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(display("This username is already busy"))]
    UsernameBusy,
    #[snafu(display("Email address is invalid"))]
    EmailInvalid,
    #[snafu(display("User with this username was not found"))]
    UserNotFound,
}
