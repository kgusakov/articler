pub mod error;

use db::repository::{self, Db};
use helpers::verify_password;

use crate::error::Result;

pub async fn find_user<'c, C>(
    conn: C,
    username: &str,
    password: &str,
) -> Result<Option<repository::users::UserRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;
    if let Some(user_row) = repository::users::find_by_username(&mut *conn, username).await? {
        if verify_password(password, &user_row.password_hash)? {
            Ok(Some(user_row))
        } else {
            Ok(None)
        }
    } else {
        Ok(None)
    }
}
