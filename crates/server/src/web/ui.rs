use actix_session::Session;
use actix_web::{
    HttpResponse, Responder,
    error::ErrorInternalServerError,
    web::{self, ServiceConfig, get, post},
};
use serde::Deserialize;

use crate::{app::static_resources, auth::find_user, middleware::TransactionContext};

pub fn routes(cfg: &mut ServiceConfig) {
    cfg.route("/login", get().to(login))
        .route("/", get().to(index))
        .route("/do_login", post().to(do_login));
}

async fn login(_session: Session) -> impl Responder {
    let st = static_resources();
    let login_html = st.get("login.html").unwrap();
    let body = std::str::from_utf8(login_html.data).unwrap();

    HttpResponse::Ok()
        .append_header(("Content-type", "text/html"))
        .body(body)
}

async fn index(session: Session) -> impl Responder {
    if session.contains_key("user_id") {
        let st = static_resources();
        let index_html = st.get("index.html").unwrap();
        let body = std::str::from_utf8(index_html.data).unwrap();
        HttpResponse::Ok()
            .append_header(("Content-type", "text/html"))
            .body(body)
    } else {
        HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish()
    }
}

#[derive(Deserialize)]
pub(in crate::web) struct LoginForm {
    #[serde(rename(deserialize = "_username"))]
    username: String,
    #[serde(rename(deserialize = "_password"))]
    password: String,
}

pub(in crate::web) async fn do_login(
    tctx: web::ReqData<TransactionContext<'_>>,
    form: web::Form<LoginForm>,
    session: Session,
) -> impl Responder {
    let mut tx = match tctx.tx() {
        Ok(tx) => tx,
        Err(e) => return HttpResponse::from_error(ErrorInternalServerError(e)),
    };

    if let Ok(Some(user)) = find_user(&mut tx, &form.username, &form.password).await
        && let Err(err) = session.insert("user_id", user.id)
    {
        return HttpResponse::from_error(ErrorInternalServerError(err));
    }

    HttpResponse::Found()
        .append_header(("Location", "/"))
        .finish()
}
