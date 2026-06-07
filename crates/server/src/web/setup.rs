use actix_web::{
    HttpResponse,
    http::header,
    mime,
    web::{self, Form, ServiceConfig, get, post},
};
use app_state::AppState;
use chrono::Utc;
use db::repository::users;
use helpers::hash_password;
use types::{Password, Username};

use crate::error::Result;
use dto::{SetupForm, SetupFormError};

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
    let rendered = app.handlebars.render(
        "setup",
        &SetupFormError {
            error: None,
            username: None,
        },
    )?;
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
        Err(err) => return render_error(&app, &err.to_string(), &form.username),
    };

    let password = match Password::try_from(form.password.as_str()) {
        Ok(p) => p,
        Err(err) => return render_error(&app, &err.to_string(), &form.username),
    };

    let confirm_password = match Password::try_from(form.confirm_password.as_str()) {
        Ok(p) => p,
        Err(err) => return render_error(&app, &err.to_string(), &form.username),
    };

    if password != confirm_password {
        return render_error(&app, "Passwords do not match", &form.username);
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

    Ok(HttpResponse::SeeOther()
        .append_header(("Location", "/login"))
        .finish())
}

fn render_error(app: &web::Data<AppState>, error: &str, username: &str) -> Result<HttpResponse> {
    let rendered = app.handlebars.render(
        "setup",
        &SetupFormError {
            error: Some(error.to_owned()),
            username: Some(username.to_owned()),
        },
    )?;
    Ok(HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
        .body(rendered))
}

mod dto {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize)]
    pub struct SetupForm {
        pub username: String,
        pub password: String,
        pub confirm_password: String,
    }

    #[derive(Serialize)]
    pub struct SetupFormError {
        pub error: Option<String>,
        pub username: Option<String>,
    }
}
