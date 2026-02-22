use actix_session::Session;
use actix_web::{
    HttpResponse, Responder,
    error::ErrorInternalServerError,
    http::header,
    mime,
    web::{self, ServiceConfig, get, post},
};

use crate::{
    app::AppState,
    web::{dto::Client, fake_ui::dto::Clients, ui::do_login},
};
use db::repository::clients::{self};

// The whole file is just a fake pages to support the way of authorization, which Android app and browser extensions are using

pub fn routes(cfg: &mut ServiceConfig) {
    cfg.route("/login_check", post().to(do_login))
        .route("/developer", get().to(developer));
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

mod dto {
    use serde::Serialize;

    use crate::web::dto::Client;

    #[derive(Serialize)]
    pub struct Clients {
        pub clients: Vec<Client>,
    }
}
