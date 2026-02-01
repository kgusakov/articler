use actix_web::ResponseError;

use crate::result::ArticlerError;

pub mod fake_ui;
pub mod oauth;
pub mod wallabag_api;

impl ResponseError for ArticlerError {}
