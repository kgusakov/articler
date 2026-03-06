use std::num::TryFromIntError;

use dom_smoothie::ReadabilityError;
use snafu::{Location, Snafu};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("Text wordcount is bigger than {}", i32::MAX))]
    WordCountOverflow {
        #[snafu(source)]
        error: TryFromIntError,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(display("Can't initialize Readability parser"))]
    ReadabilityInit {
        #[snafu(source)]
        error: ReadabilityError,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(display("Can't parse article"))]
    ReadabilityParse {
        #[snafu(source)]
        error: ReadabilityError,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(display("No title attribute in PDF metadata"))]
    PdfTitleFromMetadata {
        #[snafu(source)]
        error: mupdf::error::Error,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(display("Can't receive first page of PDF"))]
    PdfContentParsing {
        #[snafu(source)]
        error: mupdf::error::Error,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(display("Can't build http client"))]
    HttpClientInit {
        #[snafu(source)]
        error: reqwest::Error,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(display("Can't fetch url content by http"))]
    HttpRequest {
        #[snafu(source)]
        error: reqwest::Error,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(display("Can't parse http response"))]
    HttpResponseParsing {
        #[snafu(source)]
        error: reqwest::Error,
        #[snafu(implicit)]
        location: Location,
    },
    #[snafu(display("Mime type is not supported: {}", mime_type))]
    MimeTypeNotSupported {
        mime_type: String,
        #[snafu(implicit)]
        location: Location,
    },
}
