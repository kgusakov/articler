use std::{fmt::Display, panic::Location};

use actix_web::ResponseError;

pub type ArticlerResult<R> = std::result::Result<R, ArticlerError>;

type BoxDynError = Box<dyn std::error::Error>;

#[derive(Debug)]
pub struct ArticlerError {
    source: BoxDynError,
    #[allow(dead_code)]
    location: &'static Location<'static>,
}

impl<T: std::error::Error + 'static> From<T> for ArticlerError {
    #[track_caller]
    fn from(value: T) -> Self {
        ArticlerError {
            source: Box::new(value),
            location: Location::caller(),
        }
    }
}

impl Display for ArticlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.source)
    }
}

// TODO wrong place for it, here is due to orphan rule
impl ResponseError for ArticlerError {}

#[cfg(test)]
mod tests {
    // Simple tests for compiling of expected cases

    use std::{io, panic::Location};

    use crate::ArticlerResult;

    fn io_err() -> std::io::Result<()> {
        Err(io::Error::from(io::ErrorKind::UnexpectedEof))
    }

    #[expect(dead_code)]
    fn compile_test_question_mark() -> ArticlerResult<()> {
        io_err()?;

        Ok(())
    }

    fn articler_res_err() -> ArticlerResult<()> {
        Err(io::Error::from(io::ErrorKind::UnexpectedEof).into())
    }

    #[test]
    fn test_res() {
        let l = Location::caller();
        assert_eq!(l.file(), articler_res_err().unwrap_err().location.file());
    }
}
