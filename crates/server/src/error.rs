use actix_http::StatusCode;
use actix_web::ResponseError;
use snafu::{Location, Snafu};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(transparent)]
    Sqlx {
        #[snafu(source)]
        error: sqlx::error::Error,
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
    Io {
        #[snafu(source)]
        error: std::io::Error,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(transparent)]
    SessionGet {
        #[snafu(source)]
        error: actix_session::SessionGetError,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(transparent)]
    SessionInsert {
        #[snafu(source)]
        error: actix_session::SessionInsertError,
        #[snafu(implicit)]
        location: Location,
    },
    Forbidden {
        #[snafu(implicit)]
        location: Location,
    },
    NotFound {
        msg: String,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(transparent)]
    Render {
        #[snafu(source)]
        error: handlebars::RenderError,
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
    Auth {
        #[snafu(source)]
        error: auth::error::Error,
        #[snafu(implicit)]
        location: Location,
    },
}

impl ResponseError for Error {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound { .. } => StatusCode::NOT_FOUND,
            Self::Forbidden { .. } => StatusCode::FORBIDDEN,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
