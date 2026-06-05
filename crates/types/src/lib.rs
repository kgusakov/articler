pub mod error;

use std::ops::Deref;

use snafu::ensure;
use url::Url;

use crate::error::{Validation, ValidationSnafu};

pub type Id = i64;
pub type ReadingTime = i32;

pub struct ClientName<'a>(&'a str);

impl ClientName<'_> {
    const MAX_LENGTH: usize = 1024;
}

impl<'a> TryFrom<&'a str> for ClientName<'a> {
    type Error = Validation;

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

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Title(String);

impl Title {
    const MAX_LENGTH: usize = 1024;
}

impl std::fmt::Display for Title {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for Title {
    type Error = Validation;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let value = value.trim().to_owned();

        ensure!(
            !value.is_empty(),
            ValidationSnafu {
                message: "Title can't be empty",
            }
        );

        let value = if value.chars().count() >= Title::MAX_LENGTH {
            value.chars().take(Title::MAX_LENGTH).collect()
        } else {
            value
        };

        Ok(Self(value))
    }
}

impl Default for Title {
    fn default() -> Self {
        Self("Title N/A".to_owned())
    }
}

impl Deref for Title {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Title> for String {
    fn from(t: Title) -> Self {
        t.0
    }
}

#[derive(Debug)]
pub struct ArticleUrl(Url);

impl std::fmt::Display for ArticleUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<Url> for ArticleUrl {
    type Error = Validation;

    fn try_from(value: Url) -> Result<Self, Self::Error> {
        ensure!(
            value.scheme() == "http" || value.scheme() == "https",
            ValidationSnafu {
                message: "Article url must use http or https url scheme",
            }
        );

        Ok(Self(value))
    }
}

impl Deref for ArticleUrl {
    type Target = Url;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
