use db::repository::Id;

pub mod oauth;
pub mod wallabag;

#[derive(Debug, Clone)]
pub(in crate::rest) struct UserInfo {
    pub user_id: Id,
    #[expect(dead_code)]
    pub client_id: Id,
}
