use actix_session::Session;
use actix_web::{
    HttpResponse, Responder,
    error::ErrorInternalServerError,
    http::header,
    mime,
    web::{self, ServiceConfig, get, post},
};
use serde::Serialize;

use crate::{
    app::AppState,
    middleware::TransactionContext,
    web::{template_data::Client, ui::do_login},
};
use db::repository::clients::{self};

// The whole file is just a fake pages to support the way of authorization, which Android app and browser extensions are using

pub fn routes(cfg: &mut ServiceConfig) {
    cfg.route("/login_check", post().to(do_login))
        .route("/developer", get().to(developer));
}

async fn developer(
    app: web::Data<AppState>,
    session: Session,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> impl Responder {
    if let Ok(Some(user_id)) = session.get("user_id") {
        let mut tx = match tctx.tx() {
            Ok(tx) => tx,
            Err(err) => {
                return HttpResponse::InternalServerError().body(err.to_string());
            }
        };

        if let Ok(client_rows) = clients::find_by_user_id(&mut tx, user_id).await {
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

#[derive(Serialize)]
struct Clients {
    clients: Vec<Client>,
}
