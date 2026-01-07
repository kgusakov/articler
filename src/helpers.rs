use sha1::{Digest, Sha1};

pub fn hash_str(st: &str) -> String {
    format!("{:x}", Sha1::digest(st))
}

pub fn generate_uid() -> String {
    format!("{:x}", rand::random::<u64>())
}
