use types::Id;

pub mod error;
pub mod models;
pub mod oauth;
pub mod wallabag;

#[derive(Debug, Clone)]
pub(crate) struct UserInfo {
    pub user_id: Id,
    #[expect(dead_code)]
    pub client_id: Id,
}
