use actix_session::Session;
use actix_web::{
    HttpResponse, Responder,
    error::ErrorInternalServerError,
    http::header,
    mime,
    web::{self, ServiceConfig, get, post},
};
use app_state::AppState;
use helpers::{generate_client_id, generate_client_secret};

use crate::web::{
    dto::Client,
    fake_ui::dto::{Clients, CreatedClient},
    ui::do_login,
};
use crate::{error::Result, web::fake_ui::dto::CreateClientForm};
use db::repository::clients::{self};
use types::ClientName;

// The whole file is just a fake pages to support the way of authorization, which Android app and browser extensions are using

pub fn routes(cfg: &mut ServiceConfig) {
    cfg.route("/login_check", post().to(do_login))
        .route("/developer", get().to(developer))
        .route("/developer/client/create", get().to(get_client_create))
        .route("/developer/client/create", post().to(post_client_create));
}

async fn developer(app: web::Data<AppState>, session: Session) -> impl Responder {
    if let Ok(Some(user_id)) = session.get("user_id") {
        if let Ok(client_rows) = clients::find_by_user_id(&app.pool, user_id).await {
            match app.handlebars.render(
                "fake_development",
                &Clients {
                    clients: client_rows.into_iter().map(Client::from).collect(),
                },
            ) {
                Ok(rendered) => HttpResponse::Ok()
                    .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
                    .body(rendered),
                Err(e) => HttpResponse::from_error(ErrorInternalServerError(e)),
            }
        } else {
            HttpResponse::Ok()
                .append_header(("Content-type", "text/html"))
                .body("")
        }
    } else {
        HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish()
    }
}

async fn get_client_create(app: web::Data<AppState>, session: Session) -> Result<HttpResponse> {
    if session.get::<i64>("user_id")?.is_some() {
        let rendered = app.handlebars.render("fake_client_create", &[0; 0])?;

        Ok(HttpResponse::Ok()
            .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
            .body(rendered))
    } else {
        Ok(HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish())
    }
}

async fn post_client_create(
    app: web::Data<AppState>,
    form: web::Form<CreateClientForm>,
    session: Session,
) -> Result<HttpResponse> {
    if let Some(user_id) = session.get::<i64>("user_id")? {
        let form = form.into_inner();

        let now = chrono::Utc::now().timestamp();

        let client = clients::create(
            &app.pool,
            user_id,
            &ClientName::try_from(form.name.as_str())?,
            &generate_client_id(),
            &generate_client_secret(),
            now,
        )
        .await?;

        let rendered = app.handlebars.render(
            "fake_client_create_result",
            &CreatedClient {
                client_name: client.name,
                client_id: client.client_id,
                client_secret: client.client_secret,
            },
        )?;

        Ok(HttpResponse::Ok()
            .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
            .body(rendered))
    } else {
        Ok(HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish())
    }
}

mod dto {
    use serde::{Deserialize, Serialize};

    use crate::web::dto::Client;

    #[derive(Serialize)]
    pub struct Clients {
        pub clients: Vec<Client>,
    }

    #[derive(Deserialize)]
    pub struct CreateClientForm {
        #[serde(rename(deserialize = "client[name]"))]
        pub name: String,
        #[serde(rename(deserialize = "client[redirect_uris]"))]
        _redirect_uris: String,
        #[serde(rename(deserialize = "client[save]"))]
        _save: String,
        // TODO implement token processing
        #[serde(rename(deserialize = "client[_token]"))]
        _token: String,
    }

    #[derive(Serialize)]
    pub struct CreatedClient {
        pub client_name: String,
        pub client_id: String,
        pub client_secret: String,
    }
}
