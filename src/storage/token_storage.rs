use std::{collections::HashMap, io::Error, ops::Deref, sync::LazyLock};

use actix_web::cookie::Expiration;
use chrono::Utc;
use rand::{distr::Alphanumeric, prelude::*, rng};
use thiserror::Error;

type Id = i64;
type Result<T> = std::result::Result<T, Error>;

const RNG: LazyLock<ThreadRng> = LazyLock::new(|| rng());
const EXPIRATION_TIME: i64 = 60 * 60; // one hour in seconds
const REFRESH_TOKEN_EXPIRATION_TIME: i64 = 30 * 24 * 60 * 60; // one month in seconds

// TODO VERY naive implementation of token storage:
// - no expirations (!!!)
// - no bookkeeping at all
// - non thread-safe

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

pub struct TokenStorage {
    access_tokens: HashMap<String, InternalToken>,
    refresh_tokens: HashMap<String, InternalToken>,
}

impl TokenStorage {
    pub fn new() -> Self {
        TokenStorage {
            access_tokens: HashMap::new(),
            refresh_tokens: HashMap::new(),
        }
    }
}

impl TokenStorage {
    pub fn new_token(&mut self, user_id: Id, client_id: Id) -> Result<NewToken> {
        let access_token = generate_token();
        let refresh_token = generate_token();

        let now = Utc::now().timestamp();

        self.access_tokens.insert(
            access_token.clone(),
            InternalToken {
                claim: Claim { user_id, client_id },
                expires_at: now + EXPIRATION_TIME,
            },
        );

        self.refresh_tokens.insert(
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

    pub fn validate(&self, access_token: &str) -> Result<Option<Claim>> {
        let now = Utc::now().timestamp();

        if let Some(t) = self.access_tokens.get(access_token) {
            if t.expires_at < now {
                // TODO we should clean this token here
                Ok(None)
            } else {
                Ok(Some(t.claim))
            }
        } else {
            Ok(None)
        }
    }

    pub fn refresh(&mut self, refresh_token: &str) -> Result<Option<NewToken>> {
        if let Some(&InternalToken { claim, .. }) = self.refresh_tokens.get(refresh_token) {
            let access_token = generate_token();
            let new_refresh_token = generate_token();

            let now = Utc::now().timestamp();

            self.access_tokens.insert(
                access_token.to_string(),
                InternalToken {
                    claim: claim,
                    expires_at: now + EXPIRATION_TIME,
                },
            );

            self.refresh_tokens.remove(refresh_token);

            self.refresh_tokens.insert(
                new_refresh_token.clone(),
                InternalToken {
                    claim: claim,
                    expires_at: now + REFRESH_TOKEN_EXPIRATION_TIME,
                },
            );

            Ok(Some(NewToken {
                access_token,
                expires_in: 3600,
                refresh_token: new_refresh_token,
            }))
        } else {
            Ok(None)
        }
    }
}

fn generate_token() -> String {
    RNG.sample_iter(Alphanumeric)
        // TODO this 86 is a mimic to popular jwt token size, as in original API. Maybe doesn't need
        .take(86)
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_token() {
        let mut storage = TokenStorage::new();
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
        let mut storage = TokenStorage::new();
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
        let mut storage = TokenStorage::new();
        let token = storage.new_token(1, 100).unwrap();

        let claim = storage.validate(&token.refresh_token).unwrap();

        assert!(
            claim.is_none(),
            "Refresh token should not validate as access token"
        );
    }

    #[test]
    fn test_refresh_success() {
        let mut storage = TokenStorage::new();
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
        let mut storage = TokenStorage::new();

        let result = storage.refresh("invalid_refresh_token").unwrap();

        assert!(result.is_none(), "Should not refresh with invalid token");
    }

    #[test]
    fn test_refresh_access_token_as_refresh_token() {
        let mut storage = TokenStorage::new();
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
        let mut storage = TokenStorage::new();
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
        let mut storage = TokenStorage::new();

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
}
