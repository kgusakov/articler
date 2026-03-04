use snafu::{Location, Snafu};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(transparent)]
    Sqlx {
        #[snafu(source)]
        error: sqlx::Error,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(display("{msg}"))]
    NotSupportedYet {
        msg: String,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(display("{msg}"))]
    TooManySqliteHostParameters {
        msg: String,
        #[snafu(implicit)]
        location: Location,
    },
}
