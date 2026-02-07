use actix_session::{Session, SessionMiddleware, storage::CookieSessionStore};
use actix_web::{
    HttpResponse, Responder,
    cookie::Key,
    error::ErrorInternalServerError,
    web::{self, ServiceConfig, get, post},
};
use serde::Deserialize;

use crate::{app::static_resources, auth::find_user, middleware::TransactionContext};
use db::repository::clients;

const ANDROID_APP_NAME: &str = "Android app";

// The whole file is just a fake pages to support the way of authorization, which Android app is using

pub fn routes(cfg: &mut ServiceConfig, cookie_key: Key) {
    cfg.service(
        web::scope("")
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), cookie_key.clone())
                    .cookie_secure(false) // TODO Set to true in production with HTTPS
                    .build(),
            )
            .route("/login", get().to(login))
            .route("/", get().to(index))
            .route("/developer", get().to(developer))
            .route("/login_check", post().to(login_check))
            .route("/login_check_normal", post().to(login_check)),
    );
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

async fn developer(session: Session, tctx: web::ReqData<TransactionContext<'_>>) -> impl Responder {
    if let Ok(Some(user_id)) = session.get("user_id") {
        let mut tx = match tctx.tx() {
            Ok(tx) => tx,
            Err(err) => {
                return HttpResponse::InternalServerError().body(err.to_string());
            }
        };

        if let Ok(Some(client_row)) =
            clients::find_by_client_name_and_user_id(&mut tx, user_id, ANDROID_APP_NAME).await
        {
            let (id, client_id, client_secret) = (
                client_row.id,
                client_row.client_id,
                client_row.client_secret,
            );
            HttpResponse::Ok().append_header(("Content-type", "text/html")).body(format!(
            r#"
            <html>
                <body>
                    <ul class="collapsible" data-collapsible="expandable" display:none>
                        <li>
                            <div class="collapsible-header">{ANDROID_APP_NAME} - #1</div>
                            <div class="collapsible-body">
                                <table class="striped">
                                    <tbody><tr>
                                        <td>Client ID</td>
                                        <td>
                                            <strong><code>{client_id}</code></strong>
                                            <button class="btn">Copy</button>
                                        </td>
                                    </tr>
                                    <tr>
                                        <td>Client secret</td>
                                        <td>
                                            <strong><code>{client_secret}</code></strong>
                                            <button class="btn">Copy</button>
                                        </td>
                                    </tr>
                                    <tr>
                                        <td>Redirect URIs</td>
                                        <td><strong><code>[null]</code></strong></td>
                                    </tr>
                                    <tr>
                                        <td>Grant type allowed</td>
                                        <td><strong><code>["token","authorization_code","password","refresh_token"]</code></strong></td>
                                    </tr>
                                </tbody></table>
                                
                                <form action="/developer/client/delete/{id}" method="post" name="delete-client">
                                    <input type="hidden" name="token" value="">

                                    <button class="waves-effect waves-light btn red" type="submit">Remove the client {ANDROID_APP_NAME}</button>
                                </form>
                            </div>
                        </li>
                    </ul>
                </body>
            </html>
        "#),
        )
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

#[derive(Deserialize)]
struct LoginForm {
    #[serde(rename(deserialize = "_username"))]
    username: String,
    #[serde(rename(deserialize = "_password"))]
    password: String,
}

async fn login_check(
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
