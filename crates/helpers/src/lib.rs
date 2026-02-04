use std::sync::LazyLock;

use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{PasswordHashString, SaltString, rand_core::OsRng},
};
use sha1::{Digest, Sha1};

static PASSWORD_HASHER: LazyLock<Argon2> = LazyLock::new(Argon2::default);

pub fn hash_str(st: &str) -> String {
    format!("{:x}", Sha1::digest(st))
}

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(OsRng);
    let hash: PasswordHashString = PASSWORD_HASHER
        .hash_password(password.as_bytes(), &salt)?
        .into();

    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
    Ok(PASSWORD_HASHER
        .verify_password(password.as_bytes(), &PasswordHash::new(hash)?)
        .is_ok())
}

pub fn generate_uid() -> String {
    format!("{:x}", rand::random::<u64>())
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
