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
