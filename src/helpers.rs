use std::sync::{Arc, LazyLock};

use actix_web::error::ErrorInternalServerError;
use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{self, PasswordHashString, SaltString, rand_core::OsRng},
};
use sha1::{Digest, Sha1};

use crate::storage::repository::{UserRepository, UserRow};

static PASSWORD_HASHER: LazyLock<Argon2> = LazyLock::new(Argon2::default);

pub fn hash_str(st: &str) -> String {
    format!("{:x}", Sha1::digest(st))
}

pub fn hash_password(password: &str) -> Result<String, password_hash::errors::Error> {
    let salt = SaltString::generate(OsRng);
    let hash: PasswordHashString = PASSWORD_HASHER
        .hash_password(password.as_bytes(), &salt)?
        .into();

    Ok(hash.as_str().to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, password_hash::errors::Error> {
    Ok(PASSWORD_HASHER
        .verify_password(password.as_bytes(), &PasswordHash::new(hash)?)
        .is_ok())
}

pub fn generate_uid() -> String {
    format!("{:x}", rand::random::<u64>())
}

pub async fn find_user(
    user_repository: &Arc<dyn UserRepository>,
    username: &str,
    password: &str,
) -> actix_web::Result<Option<UserRow>> {
    if let Some(user_row) = user_repository
        .find_by_username(username)
        .await
        .map_err(ErrorInternalServerError)?
    {
        if verify_password(password, &user_row.password_hash).map_err(ErrorInternalServerError)? {
            Ok(Some(user_row))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use crate::helpers::{hash_password, verify_password};

    #[test]
    fn hash_passowrd_test() {
        let password = "password";
        let hash = hash_password(password).unwrap();

        assert!(
            verify_password(password, &hash).unwrap(),
            "Correct password pass verification"
        );

        assert!(
            !verify_password("wrong_password", &hash).unwrap(),
            "Incorrect password doesn't pass verification"
        );
    }
}
