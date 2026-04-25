use std::ops::Deref;

use snafu::{Location, prelude::*};

pub type Id = i64;
pub type ReadingTime = i32;

pub struct ClientName<'a>(&'a str);

impl ClientName<'_> {
    const MAX_LENGTH: usize = 1024;
}

impl<'a> TryFrom<&'a str> for ClientName<'a> {
    type Error = ValidationError;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        let value = value.trim();

        ensure!(
            !value.is_empty(),
            ValidationSnafu {
                message: "Client name can't be empty",
            }
        );

        ensure!(
            value.len() < ClientName::MAX_LENGTH,
            ValidationSnafu {
                message: format!(
                    "Client name must be shorter than {}",
                    ClientName::MAX_LENGTH
                )
            }
        );

        Ok(Self(value))
    }
}

impl<'a> Deref for ClientName<'a> {
    type Target = &'a str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("{message}"))]
pub struct ValidationError {
    message: String,
    #[snafu(implicit)]
    location: Location,
}
