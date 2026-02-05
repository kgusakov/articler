use db::ArticlerResult;
use db::repository::users;
use email_address::EmailAddress;
use helpers::hash_password;
use sqlx::Pool;
use sqlx::Sqlite;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliErrors {
    #[error("This username is already busy")]
    UsernameBusy,
    #[error("Email address is invalid")]
    EmailInvalid,
}

pub async fn create_user(
    pool: &Pool<Sqlite>,
    username: &str,
    password: &str,
    name: &str,
    email: &str,
) -> ArticlerResult<()> {
    let mut tx = pool.begin().await?;

    if !EmailAddress::is_valid(email) {
        return Err(CliErrors::EmailInvalid.into());
    }

    if users::find_by_username(&mut tx, username).await?.is_some() {
        return Err(CliErrors::UsernameBusy.into());
    }

    let now = chrono::Utc::now().timestamp();
    users::create_user(
        &mut tx,
        username,
        &hash_password(password)?,
        name,
        email,
        now,
        now,
    )
    .await?;

    Ok(())
}
