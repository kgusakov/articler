use actix_http::StatusCode;
use actix_web::ResponseError;
use snafu::{Location, Snafu};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    Sqlx {
        #[snafu(source)]
        error: sqlx::error::Error,
        #[snafu(implicit)]
        location: Location,
    },
    Db {
        #[snafu(source)]
        error: db::error::Error,
        #[snafu(implicit)]
        location: Location,
    },
}

impl ResponseError for Error {
    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}
