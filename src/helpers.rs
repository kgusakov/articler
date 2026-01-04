use std::str::from_utf8;

use sha1::{Digest, Sha1};
use url::Url;

pub fn hash_str(st: &str) -> String {
    format!("{:x}", Sha1::digest(st))
}
