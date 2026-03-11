pub mod error;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use db::repository::{Db, tokens};
use rand::{distr::Alphanumeric, prelude::*};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use sqlx::Connection;

use crate::error::Result;

type Id = i64;

const GC_PERIOD: i64 = 60;
const EXPIRATION_TIME: i64 = 60 * 60;
const REFRESH_TOKEN_EXPIRATION_TIME: i64 = 30 * 24 * 60 * 60;

#[derive(Debug, Clone, Copy)]
pub struct Claim {
    pub user_id: Id,
    pub client_id: Id,
}

#[derive(Debug)]
pub struct NewToken {
    pub access_token: String,
    pub expires_in: i64,
    pub refresh_token: String,
}

#[derive(Debug)]
struct InternalToken {
    claim: Claim,
    expires_at: i64,
}

struct TokenStorageInner {
    access_tokens: HashMap<String, InternalToken>,
}

pub struct TokenStorage {
    inner: Arc<Mutex<TokenStorageInner>>,
    now: Arc<dyn Fn() -> i64 + Send + Sync>,
    cancel_token: CancellationToken,
}

impl Clone for TokenStorage {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            now: Arc::clone(&self.now),
            cancel_token: self.cancel_token.clone(),
        }
    }
}

impl Drop for TokenStorage {
    fn drop(&mut self) {
        self.cancel_token.cancel();
    }
}

impl Default for TokenStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenStorage {
    #[must_use]
    pub fn new() -> Self {
        let cancel_token = CancellationToken::new();
        let storage = TokenStorage {
            inner: Arc::new(Mutex::new(TokenStorageInner {
                access_tokens: HashMap::new(),
            })),
            now: Arc::new(|| Utc::now().timestamp()),
            cancel_token: cancel_token.clone(),
        };
        tokio::spawn(storage.clone().gc_task(cancel_token));
        storage
    }

    #[cfg(test)]
    fn new_with_custom_timestamp_provider(provider: Arc<dyn Fn() -> i64 + Send + Sync>) -> Self {
        TokenStorage {
            inner: Arc::new(Mutex::new(TokenStorageInner {
                access_tokens: HashMap::new(),
            })),
            now: provider,
            cancel_token: CancellationToken::new(),
        }
    }

    async fn gc_task(self, cancel_token: CancellationToken) {
        let mut interval = tokio::time::interval(Duration::from_secs(GC_PERIOD as u64));
        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => break,
                _ = interval.tick() => {
                    let mut inner = self.inner.lock().await;
                    self.gc(&mut inner);
                }
            }
        }
    }

    pub async fn run_gc(&self) {
        let mut inner = self.inner.lock().await;
        self.gc(&mut inner);
    }

    pub async fn new_token<'c, C>(&self, conn: C, user_id: Id, client_id: Id) -> Result<NewToken>
    where
        C: sqlx::Acquire<'c, Database = Db>,
    {
        let mut inner = self.inner.lock().await;
        let access_token = generate_token();
        let refresh_token = generate_token();

        let now = self.now();

        inner.access_tokens.insert(
            access_token.clone(),
            InternalToken {
                claim: Claim { user_id, client_id },
                expires_at: now + EXPIRATION_TIME,
            },
        );

        let token_row = tokens::create(
            conn,
            &refresh_token,
            user_id,
            client_id,
            Utc::now().timestamp(),
            REFRESH_TOKEN_EXPIRATION_TIME,
        )
        .await?;

        Ok(NewToken {
            access_token,
            expires_in: EXPIRATION_TIME,
            refresh_token: token_row.token,
        })
    }

    pub async fn validate(&self, access_token: &str) -> Result<Option<Claim>> {
        let mut inner = self.inner.lock().await;
        let now = self.now();

        if let Some(t) = inner.access_tokens.get(access_token) {
            if t.expires_at < now {
                inner.access_tokens.remove(access_token);
                Ok(None)
            } else {
                Ok(Some(t.claim))
            }
        } else {
            Ok(None)
        }
    }

    pub async fn refresh<'c, C>(&self, conn: C, refresh_token: &str) -> Result<Option<NewToken>>
    where
        C: sqlx::Acquire<'c, Database = Db>,
    {
        let mut inner = self.inner.lock().await;

        let mut conn = conn.acquire().await?;

        let mut tx = conn.begin().await?;
        tokens::delete_expired(&mut tx).await?;
        tx.commit().await?;

        let mut tx = conn.begin().await?;

        let result = if let Some(internal_token) = tokens::find(&mut tx, refresh_token).await? {
            let now = self.now();

            let access_token = generate_token();
            let new_refresh_token = generate_token();

            let claim = Claim {
                user_id: internal_token.user_id,
                client_id: internal_token.client_id,
            };

            tokens::delete(&mut tx, refresh_token).await?;

            inner.access_tokens.insert(
                access_token.clone(),
                InternalToken {
                    claim,
                    expires_at: now + EXPIRATION_TIME,
                },
            );

            tokens::create(
                &mut tx,
                &new_refresh_token,
                internal_token.user_id,
                internal_token.client_id,
                Utc::now().timestamp(),
                EXPIRATION_TIME,
            )
            .await?;

            Ok(Some(NewToken {
                access_token,
                expires_in: 3600,
                refresh_token: new_refresh_token,
            }))
        } else {
            Ok(None)
        };

        tx.commit().await?;

        result
    }

    fn now(&self) -> i64 {
        (self.now)()
    }

    // TODO stupid O(n) algo for every call
    fn gc(&self, inner: &mut TokenStorageInner) {
        let now = self.now();

        inner.access_tokens.retain(|_, v| v.expires_at > now);
    }
}

fn generate_token() -> String {
    // TODO check that it is secure enough for token
    ThreadRng::default()
        .sample_iter(Alphanumeric)
        .take(64)
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{self, AtomicI64, Ordering::Relaxed},
    };

    use sqlx::SqlitePool;

    use super::*;

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../../crates/server/tests/fixtures/users.sql")
    )]
    async fn test_new_token(pool: SqlitePool) {
        let storage = TokenStorage::new();
        let user_id = 1;
        let client_id = 1;

        let token = storage.new_token(&pool, user_id, client_id).await.unwrap();

        assert!(
            token.access_token.chars().all(char::is_alphanumeric),
            "Token should only contain alphanumeric characters"
        );
        assert!(
            token.refresh_token.chars().all(char::is_alphanumeric),
            "Token should only contain alphanumeric characters"
        );
        assert!(token.expires_in > 0, "Expiry should be positive number");

        assert_ne!(
            token.access_token, token.refresh_token,
            "Access and refresh tokens should be different"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../../crates/server/tests/fixtures/users.sql")
    )]
    async fn test_validate_success(pool: SqlitePool) {
        let storage = TokenStorage::new();
        let user_id = 1;
        let client_id = 1;

        let token = storage.new_token(&pool, user_id, client_id).await.unwrap();

        let claim = storage.validate(&token.access_token).await.unwrap();

        assert!(claim.is_some(), "Should find valid access token");
        let claim = claim.unwrap();
        assert_eq!(claim.user_id, user_id, "User ID should match");
        assert_eq!(claim.client_id, client_id, "Client ID should match");
    }

    #[tokio::test]
    async fn test_validate_invalid_token() {
        let storage = TokenStorage::new();

        let claim = storage.validate("invalid_token").await.unwrap();

        assert!(claim.is_none(), "Should not find invalid token");
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../../crates/server/tests/fixtures/users.sql")
    )]
    async fn test_validate_refresh_token_as_access_token(pool: SqlitePool) {
        let storage = TokenStorage::new();
        let token = storage.new_token(&pool, 1, 1).await.unwrap();

        let claim = storage.validate(&token.refresh_token).await.unwrap();

        assert!(
            claim.is_none(),
            "Refresh token should not validate as access token"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../../crates/server/tests/fixtures/users.sql")
    )]
    async fn test_refresh_success(pool: SqlitePool) {
        let storage = TokenStorage::new();
        let user_id = 1;
        let client_id = 1;

        let original_token = storage.new_token(&pool, user_id, client_id).await.unwrap();
        let original_access = original_token.access_token.clone();
        let original_refresh = original_token.refresh_token.clone();

        let new_token = storage.refresh(&pool, &original_refresh).await.unwrap();

        assert!(new_token.is_some(), "Should successfully refresh token");
        let new_token = new_token.unwrap();

        assert_ne!(
            new_token.access_token, original_access,
            "New access token should be different"
        );
        assert_ne!(
            new_token.refresh_token, original_refresh,
            "New refresh token should be different"
        );

        let claim = storage.validate(&new_token.access_token).await.unwrap();
        assert!(claim.is_some(), "New access token should be valid");
        assert_eq!(claim.unwrap().user_id, user_id);

        let refreshed_again = storage.refresh(&pool, &original_refresh).await.unwrap();
        assert!(
            refreshed_again.is_none(),
            "Original refresh token should be invalidated"
        );

        let refreshed_with_new = storage
            .refresh(&pool, &new_token.refresh_token)
            .await
            .unwrap();
        assert!(
            refreshed_with_new.is_some(),
            "New refresh token should work"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../../crates/server/tests/fixtures/users.sql")
    )]
    async fn test_refresh_invalid_token(pool: SqlitePool) {
        let storage = TokenStorage::new();

        let result = storage
            .refresh(&pool, "invalid_refresh_token")
            .await
            .unwrap();

        assert!(result.is_none(), "Should not refresh with invalid token");
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../../crates/server/tests/fixtures/users.sql")
    )]
    async fn test_refresh_access_token_as_refresh_token(pool: SqlitePool) {
        let storage = TokenStorage::new();
        let token = storage.new_token(&pool, 1, 1).await.unwrap();

        let result = storage.refresh(&pool, &token.access_token).await.unwrap();

        assert!(
            result.is_none(),
            "Access token should not work as refresh token"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../../crates/server/tests/fixtures/users.sql")
    )]
    async fn test_multiple_refresh_cycles(pool: SqlitePool) {
        let storage = TokenStorage::new();
        let user_id = 1;
        let client_id = 1;

        let token1 = storage.new_token(&pool, user_id, client_id).await.unwrap();
        let token2 = storage
            .refresh(&pool, &token1.refresh_token)
            .await
            .unwrap()
            .unwrap();
        let token3 = storage
            .refresh(&pool, &token2.refresh_token)
            .await
            .unwrap()
            .unwrap();

        let claim = storage
            .validate(&token3.access_token)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claim.user_id, user_id);

        assert!(
            storage
                .validate(&token1.access_token)
                .await
                .unwrap()
                .is_some(),
            "Old access tokens remain valid"
        );
        assert!(
            storage
                .refresh(&pool, &token1.refresh_token)
                .await
                .unwrap()
                .is_none(),
            "Old refresh token should be invalid"
        );
        assert!(
            storage
                .refresh(&pool, &token2.refresh_token)
                .await
                .unwrap()
                .is_none(),
            "Old refresh token should be invalid"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../../crates/server/tests/fixtures/users.sql")
    )]
    async fn test_concurrent_tokens_different_users(pool: SqlitePool) {
        let storage = TokenStorage::new();

        let token1 = storage.new_token(&pool, 1, 1).await.unwrap();
        let token2 = storage.new_token(&pool, 2, 4).await.unwrap();

        let claim1 = storage
            .validate(&token1.access_token)
            .await
            .unwrap()
            .unwrap();
        let claim2 = storage
            .validate(&token2.access_token)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(claim1.user_id, 1);
        assert_eq!(claim1.client_id, 1);
        assert_eq!(claim2.user_id, 2);
        assert_eq!(claim2.client_id, 4);

        storage.refresh(&pool, &token1.refresh_token).await.unwrap();

        let claim2_after = storage
            .validate(&token2.access_token)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            claim2_after.user_id, 2,
            "Other user's token should still work"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../../crates/server/tests/fixtures/users.sql")
    )]
    async fn test_access_token_expiration_on_validate(pool: SqlitePool) {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Arc::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let token = storage.new_token(&pool, 1, 1).await.unwrap();

        let claim = storage.validate(&token.access_token).await.unwrap();
        assert!(claim.is_some(), "Token should be valid when just created");

        current_time.store(1000 + EXPIRATION_TIME + 1, Relaxed);

        let claim = storage.validate(&token.access_token).await.unwrap();
        assert!(
            claim.is_none(),
            "Expired access token should return None on validation"
        );

        current_time.store(1000, Relaxed);
        let claim = storage.validate(&token.access_token).await.unwrap();
        assert!(
            claim.is_none(),
            "Expired token should be removed from storage"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../../crates/server/tests/fixtures/users.sql")
    )]
    // TODO reimplement test to test db-based expiration behaviour
    #[ignore = "Refresh tokens are now stored in database which uses real timestamps, so mocked time doesn't work"]
    async fn test_refresh_token_expiration_on_refresh(pool: SqlitePool) {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Arc::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let token = storage.new_token(&pool, 1, 1).await.unwrap();

        let new_token = storage.refresh(&pool, &token.refresh_token).await.unwrap();
        assert!(
            new_token.is_some(),
            "Refresh token should work when just created"
        );
        let new_token = new_token.unwrap();

        current_time.store(1000 + REFRESH_TOKEN_EXPIRATION_TIME + 1, Relaxed);

        let result = storage
            .refresh(&pool, &new_token.refresh_token)
            .await
            .unwrap();
        assert!(result.is_none(), "Expired refresh token should return None");

        current_time.store(1000, Relaxed);
        let result = storage
            .refresh(&pool, &new_token.refresh_token)
            .await
            .unwrap();
        assert!(
            result.is_none(),
            "Expired refresh token should be removed from storage"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../../crates/server/tests/fixtures/users.sql")
    )]
    async fn test_access_token_not_expired_before_expiration_time(pool: SqlitePool) {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Arc::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let token = storage.new_token(&pool, 1, 1).await.unwrap();

        current_time.store(1000 + EXPIRATION_TIME - 1, Relaxed);

        let claim = storage.validate(&token.access_token).await.unwrap();
        assert!(
            claim.is_some(),
            "Token should still be valid before expiration time"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../../crates/server/tests/fixtures/users.sql")
    )]
    async fn test_refresh_token_still_valid_after_reboot(pool: SqlitePool) {
        let storage = TokenStorage::new();

        let token = storage.new_token(&pool, 1, 1).await.unwrap();

        let storage = TokenStorage::new();

        let new_refresh_token = storage.refresh(&pool, &token.refresh_token).await.unwrap();

        assert!(new_refresh_token.is_some(), "New refresh token received");
        assert_ne!(
            token.refresh_token,
            new_refresh_token.unwrap().refresh_token,
            "New refresh token is not the same as old one"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../../crates/server/tests/fixtures/users.sql")
    )]
    async fn test_gc_removes_expired_tokens(pool: SqlitePool) {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Arc::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let token1 = storage.new_token(&pool, 1, 1).await.unwrap();

        current_time.store(1000 + EXPIRATION_TIME + 1, atomic::Ordering::Relaxed);

        let token2 = storage.new_token(&pool, 2, 4).await.unwrap();

        let claim2 = storage.validate(&token2.access_token).await.unwrap();
        assert!(claim2.is_some(), "Token2 should be valid");

        storage.run_gc().await;

        current_time.store(1000, atomic::Ordering::Relaxed);
        let claim1 = storage.validate(&token1.access_token).await.unwrap();
        assert!(claim1.is_none(), "Token1 should be removed by GC");
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../../crates/server/tests/fixtures/users.sql")
    )]
    async fn test_gc_preserves_valid_tokens(pool: SqlitePool) {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Arc::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let token1 = storage.new_token(&pool, 1, 1).await.unwrap();
        let token2 = storage.new_token(&pool, 2, 4).await.unwrap();

        // Move time forward but not enough to expire tokens
        current_time.store(1000 + EXPIRATION_TIME / 2, Relaxed);

        storage.run_gc().await;

        // Token1 should still be valid
        let claim1 = storage.validate(&token1.access_token).await.unwrap();
        assert!(claim1.is_some(), "Token1 should still be valid");

        // Token2 should still be there
        let claim2 = storage.validate(&token2.access_token).await.unwrap();
        assert!(
            claim2.is_some(),
            "Token2 should be preserved by GC since it's still valid"
        );
    }
}
