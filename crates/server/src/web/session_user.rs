use std::future::{Ready, ready};

use actix_session::SessionExt;
use actix_web::{
    FromRequest, HttpRequest, HttpResponse, ResponseError,
    dev::Payload,
    http::{StatusCode, header},
};
use snafu::Snafu;
use types::Id;

pub struct SessionUser {
    pub user_id: Id,
}

#[derive(Debug, Snafu)]
pub enum Unauthenticated {
    #[snafu(display("Authentication required"))]
    Page,
    #[snafu(display("Authentication required"))]
    Htmx,
}

impl ResponseError for Unauthenticated {
    fn status_code(&self) -> StatusCode {
        match self {
            Unauthenticated::Page => StatusCode::FOUND,
            Unauthenticated::Htmx => StatusCode::FORBIDDEN,
        }
    }

    fn error_response(&self) -> HttpResponse {
        match self {
            Unauthenticated::Page => HttpResponse::Found()
                .insert_header((header::LOCATION, "/login"))
                .finish(),
            Unauthenticated::Htmx => HttpResponse::Forbidden()
                .insert_header(("HX-Redirect", "/login"))
                .finish(),
        }
    }
}

impl FromRequest for SessionUser {
    type Error = Unauthenticated;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _payload: &mut Payload) -> Self::Future {
        // TODO check if user still exists
        let failure = if req.headers().contains_key("HX-Request") {
            Unauthenticated::Htmx
        } else {
            Unauthenticated::Page
        };

        match req.get_session().get::<Id>("user_id") {
            Ok(Some(user_id)) => ready(Ok(SessionUser { user_id })),
            _ => ready(Err(failure)),
        }
    }
}
