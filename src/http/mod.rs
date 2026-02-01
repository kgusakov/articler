use actix_web::ResponseError;

use crate::result::ArticlerError;

pub mod api;
pub mod fake_ui;
pub mod oauth;

impl ResponseError for ArticlerError {}
