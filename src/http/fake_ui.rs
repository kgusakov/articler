use actix_session::{Session, SessionMiddleware, storage::CookieSessionStore};
use actix_web::{
    HttpResponse, Responder,
    cookie::Key,
    error::ErrorInternalServerError,
    web::{self, ServiceConfig, get, post},
};
use serde::Deserialize;

use crate::{app::AppState, helpers::find_user};

const ANDROID_APP_NAME: &str = "Android app";

// The whole file is just a fake pages to support the way of authorization, which Android app is using

pub fn routes(cfg: &mut ServiceConfig, cookie_key: Key) {
    cfg.service(
        web::scope("")
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), cookie_key.clone())
                    .cookie_secure(false) // Set to true in production with HTTPS
                    .build(),
            )
            .route("/login", get().to(login))
            .route("/", get().to(index))
            .route("/developer", get().to(developer))
            .route("/login_check", post().to(login_check)),
    );
}

async fn login(_session: Session) -> impl Responder {
    HttpResponse::Ok().append_header(("Content-type", "text/html")).body(
        r#"
        <html>
            <body>
            <div class="card sw">
                <div class="center"><img src="" class="typo-logo" alt="wallabag logo"></div>
                <form action="/login_check" method="post" name="loginform">
                    <div class="card-content">
                        <div class="row">

                            <div class="input-field col s12">
                                <label for="username" class="">Username</label>
                                <input type="text" id="username" name="_username" value="" autofocus="">
                            </div>

                            <div class="input-field col s12">
                                <label for="password">Password</label>
                                <input type="password" id="password" name="_password">
                            </div>

                            <div class="input-field col s12">
                                <input type="checkbox" id="remember_me" name="_remember_me" checked="">
                                <label for="remember_me">Keep me logged in</label>
                            </div>

                        </div>
                    </div>
                    <div class="card-action center">
                        <input type="hidden" name="_csrf_token" value="fUYkqRJIgF0uxA6GUEk9ZrwnUEnuvQEUxiPPls1CDOc">
                                <button class="btn waves-effect waves-light" type="submit" name="send">
                            Log in
                            <i class="material-icons right">send</i>
                        </button>
                    </div>
                    <div class="card-action center">
                        <a href="/resetting/request">Forgot your password?</a>
                    </div>
                </form>
            </div>
            </body>
        </html>
    "#)
}

async fn index(session: Session) -> impl Responder {
    if session.contains_key("user_id") {
        HttpResponse::Ok()
            .append_header(("Content-type", "text/html"))
            .body(
                r#"
                <html>
                    <body>
                        <div class="center"><img src="" class="typo-logo" alt="wallabag logo"></div>
                        <a href="/logout"><i class="material-icons">input</i> Logout</a>
                    </body>
                </html>
        "#,
            )
    } else {
        HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish()
    }
}

async fn developer(session: Session, data: web::Data<AppState>) -> impl Responder {
    if let Ok(Some(user_id)) = session.get("user_id") {
        if let Ok(Some(client_row)) = data
            .client_repository
            .find_by_client_name_and_user_id(user_id, ANDROID_APP_NAME)
            .await
        {
            let (client_id, client_secret) = (client_row.client_id, client_row.client_secret);
            HttpResponse::Ok().append_header(("Content-type", "text/html")).body(format!(
            r#"
            <html>
                <body>
                    <ul class="collapsible" data-collapsible="expandable">
                        <li>
                            <div class="collapsible-header">{ANDROID_APP_NAME} - #1</div>
                            <div class="collapsible-body">
                                <table class="striped">
                                    <tbody><tr>
                                        <td>Client ID</td>
                                        <td>
                                            <strong><code>{client_id}</code></strong>
                                            <button class="btn" data-clipboard-text="6_3hday2utyqww40cwosw8c0w88wk00os0koowo8ksg0ccgccksc">Copy</button>
                                        </td>
                                    </tr>
                                    <tr>
                                        <td>Client secret</td>
                                        <td>
                                            <strong><code>{client_secret}</code></strong>
                                            <button class="btn" data-clipboard-text="1j9zp8t547r4woc0oo4og0s4oo0csc0ws840ksokscwskg80os">Copy</button>
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

                                <p>You have the ability to remove the client httpie. This action is IRREVERSIBLE !</p>
                                <p>If you remove it, every app configured with that client won't be able to auth on your wallabag.</p>
                                <form action="/developer/client/delete/6" method="post" name="delete-client">
                                    <input type="hidden" name="token" value="CkHfckZmWbqhAg5rpx031STjJjIQ8XGRmUstX3v_yOA">

                                    <button class="waves-effect waves-light btn red" type="submit">Remove the client httpie</button>
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
    data: web::Data<AppState>,
    form: web::Form<LoginForm>,
    session: Session,
) -> impl Responder {
    if let Ok(Some(user)) = find_user(&data.user_repository, &form.username, &form.password).await {
        if let Err(err) = session.insert("user_id", user.id) {
            return HttpResponse::from_error(ErrorInternalServerError(err));
        }
    }

    HttpResponse::Found()
        .append_header(("Location", "/"))
        .finish()
}
