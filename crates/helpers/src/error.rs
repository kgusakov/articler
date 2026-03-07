use snafu::{Location, Snafu};

pub(crate) type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(transparent)]
    PassowrdHasher {
        #[snafu(source)]
        error: argon2::password_hash::errors::Error,
        #[snafu(implicit)]
        location: Location,
    },
}
