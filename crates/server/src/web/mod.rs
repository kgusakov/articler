use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web::{cookie::Key, web};

pub mod fake_ui;
pub mod ui;

pub fn routes(cfg: &mut web::ServiceConfig, cookie_key: Key) {
    cfg.service(
        web::scope("")
            .wrap(
                SessionMiddleware::builder(CookieSessionStore::default(), cookie_key)
                    .cookie_secure(false) // TODO Set to true in production with HTTPS
                    .build(),
            )
            .configure(fake_ui::routes)
            .configure(ui::routes),
    );
}

mod dto {
    use db::repository::clients::ClientRow;
    use serde::{Deserialize, Serialize};
    use types::Id;

    #[derive(Serialize)]
    pub struct Client {
        id: Id,
        client_id: String,
        client_name: String,
        client_secret: String,
    }

    impl From<ClientRow> for Client {
        fn from(value: ClientRow) -> Self {
            Client {
                id: value.id,
                client_id: value.client_id,
                client_name: value.name,
                client_secret: value.client_secret,
            }
        }
    }

    #[derive(Serialize, Deserialize)]
    pub(in crate::web) struct LoginForm {
        #[serde(rename(deserialize = "_username"))]
        pub username: String,
        #[serde(rename(deserialize = "_password"))]
        pub password: String,
    }
}
