pub mod error;

use db::repository::clients;
use db::repository::clients::ClientRow;
use db::repository::users;
use email_address::EmailAddress;
use helpers::{generate_client_id, generate_client_secret, hash_password};
use sqlx::Pool;
use sqlx::Sqlite;

use crate::error::{EmailInvalidSnafu, Result, UserNotFoundSnafu, UsernameBusySnafu};

pub async fn create_user(
    pool: &Pool<Sqlite>,
    username: &str,
    password: &str,
    name: &str,
    email: &str,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    if !EmailAddress::is_valid(email) {
        return EmailInvalidSnafu.fail();
    }

    if users::find_by_username(&mut *tx, username).await?.is_some() {
        return UsernameBusySnafu.fail();
    }

    let now = chrono::Utc::now().timestamp();
    users::create_user(
        &mut *tx,
        username,
        &hash_password(password)?,
        name,
        email,
        now,
        now,
    )
    .await?;

    tx.commit().await?;

    Ok(())
}

pub async fn create_client(
    pool: &Pool<Sqlite>,
    username: &str,
    client_name: &str,
) -> Result<ClientRow> {
    let mut tx = pool.begin().await?;

    if let Some(user) = users::find_by_username(&mut *tx, username).await? {
        let now = chrono::Utc::now().timestamp();
        let client = clients::create(
            &mut *tx,
            user.id,
            client_name,
            &generate_client_id(),
            &generate_client_secret(),
            now,
        )
        .await?;

        tx.commit().await?;

        Ok(client)
    } else {
        UserNotFoundSnafu.fail()
    }
}
