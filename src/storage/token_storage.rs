use std::{collections::HashMap, io::Error};

use chrono::Utc;
use serde::{Deserialize, Serialize};

type Id = i64;
type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Serialize, Deserialize)]
struct Claim {
    user_id: Id,
    client_id: Id,
}

pub struct Token {
    access_token: String,
    expires_in: i64,
    refresh_token: String,
}

pub struct TokenStorage {}

impl TokenStorage {
    pub fn new_token(&self, user_id: Id, client_id: Id) -> Result<Token> {
        todo!()
    }

    pub fn validate(&self, token: String) -> Result<Option<Claim>> {
        todo!()
    }

    pub fn refresh(&self, refresh_token: String) -> Result<Option<Token>> {
        todo!()
    }
}
