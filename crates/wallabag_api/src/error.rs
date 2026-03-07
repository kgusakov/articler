use actix_http::{StatusCode, header::TryIntoHeaderValue};
use actix_web::{ResponseError, web::BufMut};
use serde_json::json;
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
        description: String,
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
            Self::Oauth { status_code, .. } => *status_code,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> actix_web::HttpResponse<actix_web::body::BoxBody> {
        let mut res = actix_web::HttpResponse::new(self.status_code());

        let json_error = match self {
            Self::Oauth {
                error, description, ..
            } => json!({
                "error": error,
                "error_description": description
            }),
            _ => {
                json!({
                    "error": "internal error",
                    "error_description": self.to_string()
                })
            }
        };

        let mut buf = actix_web::web::BytesMut::new();

        buf.put_slice(json_error.to_string().as_bytes());

        let mime_type = actix_web::mime::APPLICATION_JSON
            .try_into_value()
            .expect("Into value for constant mime type can never be invalid");

        res.headers_mut()
            .insert(actix_web::http::header::CONTENT_TYPE, mime_type);

        res.set_body(actix_web::body::BoxBody::new(buf))
    }
}
