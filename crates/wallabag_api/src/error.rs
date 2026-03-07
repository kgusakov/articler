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
    TokenStorage {
        #[snafu(source)]
        error: token_storage::error::Error,
        #[snafu(implicit)]
        location: Location,
    },
    NotImpemented {
        msg: String,
        #[snafu(implicit)]
        location: Location,
    },
    NotFound {
        msg: String,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(display("Unexpected state: {msg}"))]
    UnexpectedState {
        msg: String,
        #[snafu(implicit)]
        location: Location,
    },
    UrlFormat {
        #[snafu(source)]
        error: url::ParseError,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(display("Can't convert timestamp '{timestamp}' to DateTime"))]
    TimestampToDateTime {
        timestamp: i64,
        #[snafu(implicit)]
        location: Location,
    },
    Oauth {
        error: String,
        desription: String,
        status_code: StatusCode,
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
            Self::Oauth {
                error: _,
                desription: _,
                status_code,
                location: _,
            } => *status_code,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
