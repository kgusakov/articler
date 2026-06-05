use snafu::{Location, Snafu};

#[derive(Debug, Snafu)]
#[snafu(display("{message}"))]
#[snafu(visibility(pub(crate)))]
pub struct Validation {
    message: String,
    #[snafu(implicit)]
    location: Location,
}
