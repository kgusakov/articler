pub mod error;

use std::sync::LazyLock;

use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{PasswordHashString, SaltString, rand_core::OsRng},
};
use rand::RngExt;
use rand::distr::Alphanumeric;
use sha1::{Digest, Sha1};
use url::Url;

use crate::error::Result;

static PASSWORD_HASHER: LazyLock<Argon2> = LazyLock::new(Argon2::default);

pub fn hash_url(url: &Url) -> String {
    format!("{:x}", Sha1::digest(url.as_str()))
}

pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(OsRng);
    let hash: PasswordHashString = PASSWORD_HASHER
        .hash_password(password.as_bytes(), &salt)?
        .into();

    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    Ok(PASSWORD_HASHER
        .verify_password(password.as_bytes(), &PasswordHash::new(hash)?)
        .is_ok())
}

pub fn generate_uid() -> String {
    format!("{:x}", rand::random::<u64>())
}

pub fn generate_client_id() -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}

pub fn generate_client_secret() -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(48)
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{hash_password, verify_password};

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
