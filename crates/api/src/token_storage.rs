use std::collections::HashMap;

use chrono::Utc;
use db::repository::{Db, tokens};
use rand::{distr::Alphanumeric, prelude::*};

use result::ArticlerResult;
use tokio::sync::Mutex;

type Id = i64;

const EXPIRATION_TIME: i64 = 60 * 60; // one hour in seconds
const REFRESH_TOKEN_EXPIRATION_TIME: i64 = 30 * 24 * 60 * 60; // one month in seconds

// TODO fix global mutex and gc on every call (without calls it will produce memory leaks moreover)

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
    refresh_tokens: HashMap<String, InternalToken>,
}

pub struct TokenStorage {
    inner: Mutex<TokenStorageInner>,
    now: Box<dyn Fn() -> i64 + Send + Sync>,
}

impl Default for TokenStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenStorage {
    pub fn new() -> Self {
        TokenStorage {
            inner: Mutex::new(TokenStorageInner {
                access_tokens: HashMap::new(),
                refresh_tokens: HashMap::new(),
            }),
            now: Box::new(|| Utc::now().timestamp()),
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn new_with_custom_timestamp_provider(provider: Box<dyn Fn() -> i64 + Send + Sync>) -> Self {
        TokenStorage {
            inner: Mutex::new(TokenStorageInner {
                access_tokens: HashMap::new(),
                refresh_tokens: HashMap::new(),
            }),
            now: provider,
        }
    }

    pub async fn new_token(
        &self,
        tx: &mut sqlx::Transaction<'_, Db>,
        user_id: Id,
        client_id: Id,
    ) -> ArticlerResult<NewToken> {
        // It's ok to unwrap - poison should be propagated
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

        let token_row = tokens::create_token(
            tx,
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

    pub async fn validate(&self, access_token: &str) -> ArticlerResult<Option<Claim>> {
        // It's ok to unwrap - poison should be propagated
        let mut inner = self.inner.lock().await;
        // TODO gc on every validate is a bad pattern
        self.gc(&mut inner);

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

    pub async fn refresh(
        &self,
        tx: &mut sqlx::Transaction<'_, Db>,
        refresh_token: &str,
    ) -> ArticlerResult<Option<NewToken>> {
        // It's ok to unwrap - poison should be propagated
        let mut inner = self.inner.lock().await;

        tokens::delete_expired_tokens(tx).await?;

        if let Some(internal_token) = tokens::find_token(tx, refresh_token).await? {
            let now = self.now();

            let access_token = generate_token();
            let new_refresh_token = generate_token();

            let claim = Claim {
                user_id: internal_token.user_id,
                client_id: internal_token.client_id,
            };

            inner.refresh_tokens.remove(refresh_token);
            tokens::delete_token(tx, refresh_token).await?;

            inner.access_tokens.insert(
                access_token.to_string(),
                InternalToken {
                    claim,
                    expires_at: now + EXPIRATION_TIME,
                },
            );

            tokens::create_token(
                tx,
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
        }
    }

    fn now(&self) -> i64 {
        (self.now)()
    }

    // TODO stupid O(n) algo for every call
    fn gc(&self, inner: &mut TokenStorageInner) {
        let now = self.now();

        inner.access_tokens.retain(|_, v| v.expires_at > now);
        inner.refresh_tokens.retain(|_, v| v.expires_at > now);
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
        fixtures("../tests/fixtures/users.sql")
    )]
    async fn test_new_token(pool: SqlitePool) {
        let storage = TokenStorage::new();
        let user_id = 1;
        let client_id = 1;

        let mut tx = pool.begin().await.unwrap();

        let token = storage
            .new_token(&mut tx, user_id, client_id)
            .await
            .unwrap();

        assert!(
            token.access_token.chars().all(|c| c.is_alphanumeric()),
            "Token should only contain alphanumeric characters"
        );
        assert!(
            token.refresh_token.chars().all(|c| c.is_alphanumeric()),
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
        fixtures("../tests/fixtures/users.sql")
    )]
    async fn test_validate_success(pool: SqlitePool) {
        let storage = TokenStorage::new();
        let user_id = 1;
        let client_id = 1;

        let mut tx = pool.begin().await.unwrap();

        let token = storage
            .new_token(&mut tx, user_id, client_id)
            .await
            .unwrap();

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
        fixtures("../tests/fixtures/users.sql")
    )]
    async fn test_validate_refresh_token_as_access_token(pool: SqlitePool) {
        let storage = TokenStorage::new();
        let mut tx = pool.begin().await.unwrap();
        let token = storage.new_token(&mut tx, 1, 1).await.unwrap();

        let claim = storage.validate(&token.refresh_token).await.unwrap();

        assert!(
            claim.is_none(),
            "Refresh token should not validate as access token"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../tests/fixtures/users.sql")
    )]
    async fn test_refresh_success(pool: SqlitePool) {
        let storage = TokenStorage::new();
        let user_id = 1;
        let client_id = 1;

        let mut tx = pool.begin().await.unwrap();
        let original_token = storage
            .new_token(&mut tx, user_id, client_id)
            .await
            .unwrap();
        let original_access = original_token.access_token.clone();
        let original_refresh = original_token.refresh_token.clone();

        let new_token = storage.refresh(&mut tx, &original_refresh).await.unwrap();

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

        let refreshed_again = storage.refresh(&mut tx, &original_refresh).await.unwrap();
        assert!(
            refreshed_again.is_none(),
            "Original refresh token should be invalidated"
        );

        // New refresh token should work
        let refreshed_with_new = storage
            .refresh(&mut tx, &new_token.refresh_token)
            .await
            .unwrap();
        assert!(
            refreshed_with_new.is_some(),
            "New refresh token should work"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../tests/fixtures/users.sql")
    )]
    async fn test_refresh_invalid_token(pool: SqlitePool) {
        let storage = TokenStorage::new();

        let mut tx = pool.begin().await.unwrap();

        let result = storage
            .refresh(&mut tx, "invalid_refresh_token")
            .await
            .unwrap();

        assert!(result.is_none(), "Should not refresh with invalid token");
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../tests/fixtures/users.sql")
    )]
    async fn test_refresh_access_token_as_refresh_token(pool: SqlitePool) {
        let storage = TokenStorage::new();
        let mut tx = pool.begin().await.unwrap();
        let token = storage.new_token(&mut tx, 1, 1).await.unwrap();

        // Try to use access token as refresh token
        let result = storage.refresh(&mut tx, &token.access_token).await.unwrap();

        assert!(
            result.is_none(),
            "Access token should not work as refresh token"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../tests/fixtures/users.sql")
    )]
    async fn test_multiple_refresh_cycles(pool: SqlitePool) {
        let storage = TokenStorage::new();
        let user_id = 1;
        let client_id = 1;

        let mut tx = pool.begin().await.unwrap();
        let token1 = storage
            .new_token(&mut tx, user_id, client_id)
            .await
            .unwrap();
        let token2 = storage
            .refresh(&mut tx, &token1.refresh_token)
            .await
            .unwrap()
            .unwrap();
        let token3 = storage
            .refresh(&mut tx, &token2.refresh_token)
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
                .refresh(&mut tx, &token1.refresh_token)
                .await
                .unwrap()
                .is_none(),
            "Old refresh token should be invalid"
        );
        assert!(
            storage
                .refresh(&mut tx, &token2.refresh_token)
                .await
                .unwrap()
                .is_none(),
            "Old refresh token should be invalid"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../tests/fixtures/users.sql")
    )]
    async fn test_concurrent_tokens_different_users(pool: SqlitePool) {
        let storage = TokenStorage::new();

        let mut tx = pool.begin().await.unwrap();
        let token1 = storage.new_token(&mut tx, 1, 1).await.unwrap();
        let token2 = storage.new_token(&mut tx, 2, 4).await.unwrap();

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

        storage
            .refresh(&mut tx, &token1.refresh_token)
            .await
            .unwrap();

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
        fixtures("../tests/fixtures/users.sql")
    )]
    async fn test_access_token_expiration_on_validate(pool: SqlitePool) {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Box::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let mut tx = pool.begin().await.unwrap();
        let token = storage.new_token(&mut tx, 1, 1).await.unwrap();

        // Token should be valid immediately
        let claim = storage.validate(&token.access_token).await.unwrap();
        assert!(claim.is_some(), "Token should be valid when just created");

        // Move time forward past expiration (1 hour = 3600 seconds)
        current_time.store(1000 + EXPIRATION_TIME + 1, Relaxed);

        // Token should now be expired and return None
        let claim = storage.validate(&token.access_token).await.unwrap();
        assert!(
            claim.is_none(),
            "Expired access token should return None on validation"
        );

        // Token should be removed from storage (validating again still returns None)
        current_time.store(1000, Relaxed); // Reset time
        let claim = storage.validate(&token.access_token).await.unwrap();
        assert!(
            claim.is_none(),
            "Expired token should be removed from storage"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../tests/fixtures/users.sql")
    )]
    // TODO reimplement test to test db-based expiration behaviour
    #[ignore = "Refresh tokens are now stored in database which uses real timestamps, so mocked time doesn't work"]
    async fn test_refresh_token_expiration_on_refresh(pool: SqlitePool) {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Box::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let mut tx = pool.begin().await.unwrap();
        let token = storage.new_token(&mut tx, 1, 1).await.unwrap();

        // Refresh should work immediately
        let new_token = storage
            .refresh(&mut tx, &token.refresh_token)
            .await
            .unwrap();
        assert!(
            new_token.is_some(),
            "Refresh token should work when just created"
        );
        let new_token = new_token.unwrap();

        // Move time forward past refresh token expiration (30 days)
        current_time.store(1000 + REFRESH_TOKEN_EXPIRATION_TIME + 1, Relaxed);

        // Refresh should now fail and return None
        let result = storage
            .refresh(&mut tx, &new_token.refresh_token)
            .await
            .unwrap();
        assert!(result.is_none(), "Expired refresh token should return None");

        // Token should be removed from storage (refreshing again still returns None)
        current_time.store(1000, Relaxed); // Reset time
        let result = storage
            .refresh(&mut tx, &new_token.refresh_token)
            .await
            .unwrap();
        assert!(
            result.is_none(),
            "Expired refresh token should be removed from storage"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../tests/fixtures/users.sql")
    )]
    async fn test_access_token_not_expired_before_expiration_time(pool: SqlitePool) {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Box::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let mut tx = pool.begin().await.unwrap();
        let token = storage.new_token(&mut tx, 1, 1).await.unwrap();

        // Move time forward but not past expiration
        current_time.store(1000 + EXPIRATION_TIME - 1, Relaxed);

        // Token should still be valid
        let claim = storage.validate(&token.access_token).await.unwrap();
        assert!(
            claim.is_some(),
            "Token should still be valid before expiration time"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../tests/fixtures/users.sql")
    )]
    async fn test_refresh_token_still_valid_after_reboot(pool: SqlitePool) {
        let storage = TokenStorage::new();

        let mut tx = pool.begin().await.unwrap();
        let token = storage.new_token(&mut tx, 1, 1).await.unwrap();
        tx.commit().await.unwrap();

        let storage = TokenStorage::new();

        let mut tx = pool.begin().await.unwrap();
        let new_refresh_token = storage
            .refresh(&mut tx, &token.refresh_token)
            .await
            .unwrap();

        assert!(new_refresh_token.is_some(), "New refresh token received");
        assert_ne!(
            token.refresh_token,
            new_refresh_token.unwrap().refresh_token,
            "New refresh token is not the same as old one"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../tests/fixtures/users.sql")
    )]
    async fn test_gc_removes_expired_tokens(pool: SqlitePool) {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Box::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let mut tx = pool.begin().await.unwrap();
        let token1 = storage.new_token(&mut tx, 1, 1).await.unwrap();

        // Move time forward to expire first token
        current_time.store(1000 + EXPIRATION_TIME + 1, atomic::Ordering::Relaxed);

        let token2 = storage.new_token(&mut tx, 2, 4).await.unwrap();

        // Validate token2 (which triggers GC)
        let claim2 = storage.validate(&token2.access_token).await.unwrap();
        assert!(claim2.is_some(), "Token2 should be valid");

        // Token1 should be gone (removed by GC)
        current_time.store(1000, atomic::Ordering::Relaxed); // Reset time to when token1 was valid
        let claim1 = storage.validate(&token1.access_token).await.unwrap();
        assert!(claim1.is_none(), "Token1 should be removed by GC");
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../tests/fixtures/users.sql")
    )]
    async fn test_gc_preserves_valid_tokens(pool: SqlitePool) {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Box::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let mut tx = pool.begin().await.unwrap();
        let token1 = storage.new_token(&mut tx, 1, 1).await.unwrap();
        let token2 = storage.new_token(&mut tx, 2, 4).await.unwrap();

        // Move time forward but not enough to expire tokens
        current_time.store(1000 + EXPIRATION_TIME / 2, Relaxed);

        // Validate token1 (triggers GC)
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
