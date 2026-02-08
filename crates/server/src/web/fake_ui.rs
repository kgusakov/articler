use actix_session::Session;
use actix_web::{
    HttpResponse, Responder,
    web::{self, ServiceConfig, get, post},
};

use crate::{middleware::TransactionContext, web::ui::do_login};
use db::repository::clients;

const ANDROID_APP_NAME: &str = "Android app";

// The whole file is just a fake pages to support the way of authorization, which Android app is using

pub fn routes(cfg: &mut ServiceConfig) {
    cfg.route("/login_check", post().to(do_login))
        .route("/developer", get().to(developer));
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
