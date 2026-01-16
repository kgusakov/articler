use std::{collections::HashMap, fmt, sync::Mutex};

use chrono::Utc;
use rand::{distr::Alphanumeric, prelude::*};

type Id = i64;
type Result<T> = std::result::Result<T, Error>;

const EXPIRATION_TIME: i64 = 60 * 60; // one hour in seconds
const REFRESH_TOKEN_EXPIRATION_TIME: i64 = 30 * 24 * 60 * 60; // one month in seconds

// TODO fix global mutex and gc on every call (without calls it will produce memory leaks moreover)

#[derive(Debug, Clone)]
pub struct Error {}

// TODO implement normal display when error will in use
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Todo: write description")
    }
}

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

    pub fn new_with_custom_timestamp_provider(
        provider: Box<dyn Fn() -> i64 + Send + Sync>,
    ) -> Self {
        TokenStorage {
            inner: Mutex::new(TokenStorageInner {
                access_tokens: HashMap::new(),
                refresh_tokens: HashMap::new(),
            }),
            now: provider,
        }
    }

    pub fn new_token(&self, user_id: Id, client_id: Id) -> Result<NewToken> {
        // It's ok to unwrap - poison should be propagated
        let mut inner = self.inner.lock().unwrap();
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

        inner.refresh_tokens.insert(
            refresh_token.clone(),
            InternalToken {
                claim: Claim { user_id, client_id },
                expires_at: now + REFRESH_TOKEN_EXPIRATION_TIME,
            },
        );

        Ok(NewToken {
            access_token,
            expires_in: EXPIRATION_TIME,
            refresh_token,
        })
    }

    // TODO mut in validate looks like a bad pattern
    pub fn validate(&self, access_token: &str) -> Result<Option<Claim>> {
        // It's ok to unwrap - poison should be propagated
        let mut inner = self.inner.lock().unwrap();
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

    pub fn refresh(&self, refresh_token: &str) -> Result<Option<NewToken>> {
        // It's ok to unwrap - poison should be propagated
        let mut inner = self.inner.lock().unwrap();

        if let Some(internal_token) = inner.refresh_tokens.get(refresh_token) {
            let now = self.now();

            if internal_token.expires_at < now {
                inner.refresh_tokens.remove(refresh_token);

                Ok(None)
            } else {
                let access_token = generate_token();
                let new_refresh_token = generate_token();

                let claim = internal_token.claim;

                inner.refresh_tokens.remove(refresh_token);

                inner.access_tokens.insert(
                    access_token.to_string(),
                    InternalToken {
                        claim,
                        expires_at: now + EXPIRATION_TIME,
                    },
                );

                inner.refresh_tokens.insert(
                    new_refresh_token.clone(),
                    InternalToken {
                        claim,
                        expires_at: now + REFRESH_TOKEN_EXPIRATION_TIME,
                    },
                );

                Ok(Some(NewToken {
                    access_token,
                    expires_in: 3600,
                    refresh_token: new_refresh_token,
                }))
            }
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

    use super::*;

    #[test]
    fn test_new_token() {
        let storage = TokenStorage::new();
        let user_id = 1;
        let client_id = 100;

        let token = storage.new_token(user_id, client_id).unwrap();

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

    #[test]
    fn test_validate_success() {
        let storage = TokenStorage::new();
        let user_id = 1;
        let client_id = 100;

        let token = storage.new_token(user_id, client_id).unwrap();

        let claim = storage.validate(&token.access_token).unwrap();

        assert!(claim.is_some(), "Should find valid access token");
        let claim = claim.unwrap();
        assert_eq!(claim.user_id, user_id, "User ID should match");
        assert_eq!(claim.client_id, client_id, "Client ID should match");
    }

    #[test]
    fn test_validate_invalid_token() {
        let storage = TokenStorage::new();

        let claim = storage.validate("invalid_token").unwrap();

        assert!(claim.is_none(), "Should not find invalid token");
    }

    #[test]
    fn test_validate_refresh_token_as_access_token() {
        let storage = TokenStorage::new();
        let token = storage.new_token(1, 100).unwrap();

        let claim = storage.validate(&token.refresh_token).unwrap();

        assert!(
            claim.is_none(),
            "Refresh token should not validate as access token"
        );
    }

    #[test]
    fn test_refresh_success() {
        let storage = TokenStorage::new();
        let user_id = 1;
        let client_id = 100;

        let original_token = storage.new_token(user_id, client_id).unwrap();
        let original_access = original_token.access_token.clone();
        let original_refresh = original_token.refresh_token.clone();

        let new_token = storage.refresh(&original_refresh).unwrap();

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

        let claim = storage.validate(&new_token.access_token).unwrap();
        assert!(claim.is_some(), "New access token should be valid");
        assert_eq!(claim.unwrap().user_id, user_id);

        let refreshed_again = storage.refresh(&original_refresh).unwrap();
        assert!(
            refreshed_again.is_none(),
            "Original refresh token should be invalidated"
        );

        // New refresh token should work
        let refreshed_with_new = storage.refresh(&new_token.refresh_token).unwrap();
        assert!(
            refreshed_with_new.is_some(),
            "New refresh token should work"
        );
    }

    #[test]
    fn test_refresh_invalid_token() {
        let storage = TokenStorage::new();

        let result = storage.refresh("invalid_refresh_token").unwrap();

        assert!(result.is_none(), "Should not refresh with invalid token");
    }

    #[test]
    fn test_refresh_access_token_as_refresh_token() {
        let storage = TokenStorage::new();
        let token = storage.new_token(1, 100).unwrap();

        // Try to use access token as refresh token
        let result = storage.refresh(&token.access_token).unwrap();

        assert!(
            result.is_none(),
            "Access token should not work as refresh token"
        );
    }

    #[test]
    fn test_multiple_refresh_cycles() {
        let storage = TokenStorage::new();
        let user_id = 1;
        let client_id = 100;

        let token1 = storage.new_token(user_id, client_id).unwrap();
        let token2 = storage.refresh(&token1.refresh_token).unwrap().unwrap();
        let token3 = storage.refresh(&token2.refresh_token).unwrap().unwrap();

        let claim = storage.validate(&token3.access_token).unwrap().unwrap();
        assert_eq!(claim.user_id, user_id);

        assert!(
            storage.validate(&token1.access_token).unwrap().is_some(),
            "Old access tokens remain valid"
        );
        assert!(
            storage.refresh(&token1.refresh_token).unwrap().is_none(),
            "Old refresh token should be invalid"
        );
        assert!(
            storage.refresh(&token2.refresh_token).unwrap().is_none(),
            "Old refresh token should be invalid"
        );
    }

    #[test]
    fn test_concurrent_tokens_different_users() {
        let storage = TokenStorage::new();

        let token1 = storage.new_token(1, 100).unwrap();
        let token2 = storage.new_token(2, 200).unwrap();

        let claim1 = storage.validate(&token1.access_token).unwrap().unwrap();
        let claim2 = storage.validate(&token2.access_token).unwrap().unwrap();

        assert_eq!(claim1.user_id, 1);
        assert_eq!(claim1.client_id, 100);
        assert_eq!(claim2.user_id, 2);
        assert_eq!(claim2.client_id, 200);

        storage.refresh(&token1.refresh_token).unwrap();

        let claim2_after = storage.validate(&token2.access_token).unwrap().unwrap();
        assert_eq!(
            claim2_after.user_id, 2,
            "Other user's token should still work"
        );
    }

    #[test]
    fn test_access_token_expiration_on_validate() {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Box::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let token = storage.new_token(1, 100).unwrap();

        // Token should be valid immediately
        let claim = storage.validate(&token.access_token).unwrap();
        assert!(claim.is_some(), "Token should be valid when just created");

        // Move time forward past expiration (1 hour = 3600 seconds)
        current_time.store(1000 + EXPIRATION_TIME + 1, Relaxed);

        // Token should now be expired and return None
        let claim = storage.validate(&token.access_token).unwrap();
        assert!(
            claim.is_none(),
            "Expired access token should return None on validation"
        );

        // Token should be removed from storage (validating again still returns None)
        current_time.store(1000, Relaxed); // Reset time
        let claim = storage.validate(&token.access_token).unwrap();
        assert!(
            claim.is_none(),
            "Expired token should be removed from storage"
        );
    }

    #[test]
    fn test_refresh_token_expiration_on_refresh() {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Box::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let token = storage.new_token(1, 100).unwrap();

        // Refresh should work immediately
        let new_token = storage.refresh(&token.refresh_token).unwrap();
        assert!(
            new_token.is_some(),
            "Refresh token should work when just created"
        );
        let new_token = new_token.unwrap();

        // Move time forward past refresh token expiration (30 days)
        current_time.store(1000 + REFRESH_TOKEN_EXPIRATION_TIME + 1, Relaxed);

        // Refresh should now fail and return None
        let result = storage.refresh(&new_token.refresh_token).unwrap();
        assert!(result.is_none(), "Expired refresh token should return None");

        // Token should be removed from storage (refreshing again still returns None)
        current_time.store(1000, Relaxed); // Reset time
        let result = storage.refresh(&new_token.refresh_token).unwrap();
        assert!(
            result.is_none(),
            "Expired refresh token should be removed from storage"
        );
    }

    #[test]
    fn test_access_token_not_expired_before_expiration_time() {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Box::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let token = storage.new_token(1, 100).unwrap();

        // Move time forward but not past expiration
        current_time.store(1000 + EXPIRATION_TIME - 1, Relaxed);

        // Token should still be valid
        let claim = storage.validate(&token.access_token).unwrap();
        assert!(
            claim.is_some(),
            "Token should still be valid before expiration time"
        );
    }

    #[test]
    fn test_refresh_token_not_expired_before_expiration_time() {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Box::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let token = storage.new_token(1, 100).unwrap();

        // Move time forward but not past expiration
        current_time.store(1000 + REFRESH_TOKEN_EXPIRATION_TIME - 1, Relaxed);

        // Refresh should still work
        let result = storage.refresh(&token.refresh_token).unwrap();
        assert!(
            result.is_some(),
            "Refresh token should still work before expiration time"
        );
    }

    #[test]
    fn test_gc_removes_expired_tokens() {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Box::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let token1 = storage.new_token(1, 100).unwrap();

        // Move time forward to expire first token
        current_time.store(1000 + EXPIRATION_TIME + 1, atomic::Ordering::Relaxed);

        let token2 = storage.new_token(2, 200).unwrap();

        // Validate token2 (which triggers GC)
        let claim2 = storage.validate(&token2.access_token).unwrap();
        assert!(claim2.is_some(), "Token2 should be valid");

        // Token1 should be gone (removed by GC)
        current_time.store(1000, atomic::Ordering::Relaxed); // Reset time to when token1 was valid
        let claim1 = storage.validate(&token1.access_token).unwrap();
        assert!(claim1.is_none(), "Token1 should be removed by GC");
    }

    #[test]
    fn test_gc_preserves_valid_tokens() {
        let current_time = Arc::new(AtomicI64::new(1000i64));
        let time_clone = current_time.clone();

        let storage = TokenStorage::new_with_custom_timestamp_provider(Box::new(move || {
            time_clone.load(atomic::Ordering::Relaxed)
        }));

        let token1 = storage.new_token(1, 100).unwrap();
        let token2 = storage.new_token(2, 200).unwrap();

        // Move time forward but not enough to expire tokens
        current_time.store(1000 + EXPIRATION_TIME / 2, Relaxed);

        // Validate token1 (triggers GC)
        let claim1 = storage.validate(&token1.access_token).unwrap();
        assert!(claim1.is_some(), "Token1 should still be valid");

        // Token2 should still be there
        let claim2 = storage.validate(&token2.access_token).unwrap();
        assert!(
            claim2.is_some(),
            "Token2 should be preserved by GC since it's still valid"
        );
    }
}
