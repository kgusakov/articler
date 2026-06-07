use actix_web::{
    HttpResponse,
    http::header,
    mime,
    web::{self, Form, ServiceConfig, get, post},
};
use app_state::AppState;
use chrono::Utc;
use db::repository::users;
use handlebars::Context;
use helpers::hash_password;
use types::{Password, Username};

use crate::error::Result;
use dto::SetupForm;

pub fn routes(cfg: &mut ServiceConfig) {
    cfg.route("/setup", get().to(setup))
        .route("/do_setup", post().to(do_setup));
}

async fn setup(app: web::Data<AppState>) -> Result<HttpResponse> {
    let count = users::count(&app.pool).await?;
    if count > 0 {
        return Ok(HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish());
    }
    let rendered = app
        .handlebars
        .render_with_context("setup", &Context::null())?;
    Ok(HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
        .body(rendered))
}

async fn do_setup(app: web::Data<AppState>, form: Form<SetupForm>) -> Result<HttpResponse> {
    let count = users::count(&app.pool).await?;
    if count > 0 {
        return Ok(HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish());
    }

    let form = form.into_inner();

    let username = match Username::try_from(form.username.as_str()) {
        Ok(u) => u,
        Err(err) => return Ok(error_response(&err.to_string())),
    };

    let password = match Password::try_from(form.password.as_str()) {
        Ok(p) => p,
        Err(err) => return Ok(error_response(&err.to_string())),
    };

    let confirm_password = match Password::try_from(form.confirm_password.as_str()) {
        Ok(p) => p,
        Err(err) => return Ok(error_response(&err.to_string())),
    };

    if password != confirm_password {
        return Ok(error_response("Passwords do not match"));
    }

    let password_hash = hash_password(&password)?;
    let now = Utc::now().timestamp();

    let mut tx = app.pool.begin().await?;
    let count = users::count(&mut *tx).await?;
    if count > 0 {
        // TODO it can produce confuse behavior, like the target user was created - think about better UX
        return Ok(HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish());
    }
    users::create_user(&mut *tx, &username, &password_hash, "", "", now, now).await?;
    tx.commit().await?;

    Ok(HttpResponse::Ok()
        .append_header(("HX-Redirect", "/login"))
        .finish())
}

fn error_response(error: &str) -> HttpResponse {
    HttpResponse::UnprocessableEntity()
        .content_type(mime::TEXT_HTML)
        .body(error.to_owned())
}

mod dto {
    use serde::Deserialize;

    #[derive(Deserialize)]
    pub struct SetupForm {
        pub username: String,
        pub password: String,
        pub confirm_password: String,
    }
}
