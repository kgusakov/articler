use actix_web::{
    Error, HttpMessage,
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    error::ErrorInternalServerError,
    middleware::Next,
};
use sqlx::Transaction;
use std::cell::{RefCell, RefMut};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use crate::app::AppState;
use db::repository::Db;

#[derive(Clone)]
pub struct TransactionContext<'c> {
    tx: Rc<RefCell<Option<Transaction<'c, Db>>>>,
}

// Automatically derefs to `&mut Transaction`.
pub struct TransactionHolder<'a, 'c> {
    tx: RefMut<'a, Option<Transaction<'c, Db>>>,
}

impl<'a, 'c> Deref for TransactionHolder<'a, 'c> {
    type Target = Transaction<'c, Db>;

    fn deref(&self) -> &Self::Target {
        self.tx.as_ref().expect("Transaction already consumed")
    }
}

impl<'a, 'c> DerefMut for TransactionHolder<'a, 'c> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.tx.as_mut().expect("Transaction already consumed")
    }
}

impl<'c> TransactionContext<'c> {
    /// Gets mutable access to the transaction.
    pub fn tx(&self) -> Result<TransactionHolder<'_, 'c>, actix_web::Error> {
        let tx = self.tx.borrow_mut();

        if tx.is_none() {
            return Err(ErrorInternalServerError("Transaction already consumed"));
        }

        Ok(TransactionHolder { tx })
    }
}

pub async fn wrap_with_tx(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, Error> {
    let app_state = req
        .app_data::<actix_web::web::Data<AppState>>()
        .ok_or_else(|| ErrorInternalServerError("App data is not properly configured"))?;

    let tx = app_state
        .pool
        .begin()
        .await
        .map_err(ErrorInternalServerError)?;

    let request_context = TransactionContext {
        tx: Rc::new(RefCell::new(Some(tx))),
    };

    req.extensions_mut().insert(request_context.clone());

    let resp = next.call(req).await;

    match resp {
        Ok(response) => {
            let status = response.status();

            if !(status.is_client_error() || status.is_server_error()) {
                // Take the transaction out, dropping the RefMut before await
                let tx_option = request_context.tx.borrow_mut().take();
                if let Some(tx) = tx_option {
                    tx.commit().await.map_err(ErrorInternalServerError)?;
                } else {
                    return Err(ErrorInternalServerError("Transaction already consumed"));
                }
            } else {
                // Take the transaction out, dropping the RefMut before await
                let tx_option = request_context.tx.borrow_mut().take();
                if let Some(tx) = tx_option {
                    tx.rollback().await.map_err(ErrorInternalServerError)?;
                } else {
                    return Err(ErrorInternalServerError("Transaction already consumed"));
                }
            }

            Ok(response)
        }
        Err(e) => {
            // Take the transaction out, dropping the RefMut before await
            let tx_option = request_context.tx.borrow_mut().take();
            if let Some(tx) = tx_option {
                tx.rollback().await.map_err(ErrorInternalServerError)?;
            } else {
                return Err(ErrorInternalServerError("Transaction already consumed"));
            }
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use super::*;
    use actix_http::Request;
    use actix_service::Service;
    use actix_web::middleware::Logger;
    use actix_web::{HttpResponse, middleware::from_fn, test, web};
    use sqlx::SqlitePool;

    use crate::app::init_handlebars;
    use crate::scraper::Scraper;
    use db::repository::users;

    static INIT: Once = Once::new();

    fn init() {
        INIT.call_once(|| {
            env_logger::init_from_env(env_logger::Env::new().default_filter_or("trace"));
        });
    }

    async fn init_app(
        pool: SqlitePool,
    ) -> impl Service<Request, Response = ServiceResponse<impl MessageBody>, Error = Error> {
        init();

        let app_state = web::Data::new(AppState {
            pool: pool.clone(),
            token_storage: crate::token_storage::TokenStorage::default(),
            scraper: Scraper::new(None).unwrap(),
            handlebars: init_handlebars().unwrap(),
        });

        test::init_service(
            actix_web::App::new()
                .app_data(app_state.clone())
                .wrap(from_fn(wrap_with_tx))
                .route("/test", web::post().to(test_create_user))
                .route("/test-fail", web::post().to(test_create_user_fail)),
        )
        .await
    }

    async fn test_create_user(
        tx: web::ReqData<TransactionContext<'_>>,
    ) -> actix_web::Result<HttpResponse> {
        let mut tx = tx.tx()?;

        let now = chrono::Utc::now().timestamp();

        // Insert a test user with all required fields
        sqlx::query(
            "INSERT INTO users (username, email, name, password_hash, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("testuser")
        .bind("test@example.com")
        .bind("Test User")
        .bind("testpasswordhash")
        .bind(now)
        .bind(now)
        .execute(tx.deref_mut().as_mut())
        .await
        .map_err(ErrorInternalServerError)?;

        Ok(HttpResponse::Created().body("User created"))
    }

    // Test helper endpoint that fails
    async fn test_create_user_fail(
        tx: web::ReqData<TransactionContext<'_>>,
    ) -> actix_web::Result<HttpResponse> {
        let mut tx = tx.tx()?;

        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            "INSERT INTO users (username, email, name, password_hash, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind("failuser")
        .bind("fail@example.com")
        .bind("Fail User")
        .bind("failpasswordhash")
        .bind(now)
        .bind(now)
        .execute(tx.deref_mut().as_mut())
        .await
        .map_err(ErrorInternalServerError)?;

        Err(ErrorInternalServerError("Simulated error"))
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_transaction_commits_on_success(pool: SqlitePool) {
        let app_state = web::Data::new(AppState {
            pool: pool.clone(),
            token_storage: crate::token_storage::TokenStorage::default(),
            scraper: Scraper::new(None).unwrap(),
            handlebars: init_handlebars().unwrap(),
        });

        let app = test::init_service(
            actix_web::App::new()
                .app_data(app_state.clone())
                .wrap(Logger::default())
                .wrap(from_fn(wrap_with_tx))
                .route("/test", web::post().to(test_create_user)),
        )
        .await;

        let req = test::TestRequest::post().uri("/test").to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        // Verify user was actually persisted
        let user = sqlx::query_as::<_, users::UserRow>("SELECT * FROM users WHERE username = ?")
            .bind("testuser")
            .fetch_optional(&pool)
            .await
            .unwrap();

        assert!(
            user.is_some(),
            "User should be persisted after successful request"
        );
        assert_eq!(user.unwrap().username, "testuser");
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_transaction_rolls_back_on_error(pool: SqlitePool) {
        let app = init_app(pool.clone()).await;

        // Make request that should fail
        let req = test::TestRequest::post().uri("/test-fail").to_request();

        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_server_error());

        // Verify user was NOT persisted due to rollback
        let user = sqlx::query_as::<_, users::UserRow>("SELECT * FROM users WHERE username = ?")
            .bind("failuser")
            .fetch_optional(&pool)
            .await
            .unwrap();

        assert!(
            user.is_none(),
            "User should NOT be persisted after failed request (rollback)"
        );
    }
}
