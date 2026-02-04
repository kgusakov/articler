use db::repository;
use result::ArticlerResult;
use helpers::verify_password;

pub async fn find_user(
    tx: &mut sqlx::Transaction<'_, db::repository::Db>,
    username: &str,
    password: &str,
) -> ArticlerResult<Option<repository::users::UserRow>> {
    if let Some(user_row) = repository::users::find_by_username(tx, username).await? {
        if verify_password(password, &user_row.password_hash)? {
            Ok(Some(user_row))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}
