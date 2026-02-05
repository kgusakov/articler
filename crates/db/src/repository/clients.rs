use std::ops::DerefMut;

use sqlx::{Row, prelude::FromRow, sqlite::SqliteRow};

use result::ArticlerResult;

use super::*;

pub async fn create_client(
    tx: &mut sqlx::Transaction<'_, Db>,
    user_id: Id,
    client_name: &str,
    client_id: &str,
    client_secret: &str,
    created_at: Timestamp,
) -> ArticlerResult<ClientRow> {
    Ok(sqlx::query_as::<_, ClientRow>(&format!("INSERT INTO {CLIENTS_TABLE} (name, client_id, client_secret, user_id, created_at) VALUES(?, ?, ?, ?, ?) RETURNING *;"))
            .bind(client_name)
            .bind(client_id)
            .bind(client_secret)
            .bind(user_id)
            .bind(created_at)
            .fetch_one(tx.deref_mut())
            .await?)
}

pub async fn find_by_user_id_client_id_and_secret(
    executor: &mut sqlx::Transaction<'_, Db>,
    user_id: Id,
    client_id: &str,
    client_secret: &str,
) -> ArticlerResult<Option<ClientRow>> {
    let result = sqlx::query_as::<_, ClientRow>(&format!(
        "SELECT * FROM {} WHERE user_id = ? AND client_id = ? AND client_secret = ?",
        CLIENTS_TABLE
    ))
    .bind(user_id)
    .bind(client_id)
    .bind(client_secret)
    .fetch_optional(executor.deref_mut())
    .await?;

    Ok(result)
}

pub async fn find_by_client_id_and_secret(
    executor: &mut sqlx::Transaction<'_, Db>,
    client_id: &str,
    client_secret: &str,
) -> ArticlerResult<Option<ClientRow>> {
    let result = sqlx::query_as::<_, ClientRow>(&format!(
        "SELECT * FROM {} WHERE client_id = ? AND client_secret = ?",
        CLIENTS_TABLE
    ))
    .bind(client_id)
    .bind(client_secret)
    .fetch_optional(executor.deref_mut())
    .await?;

    Ok(result)
}

pub async fn find_by_client_name_and_user_id(
    tx: &mut sqlx::Transaction<'_, Db>,
    user_id: Id,
    client_name: &str,
) -> ArticlerResult<Option<ClientRow>> {
    let result = sqlx::query_as::<_, ClientRow>(&format!(
        "SELECT * FROM {} WHERE user_id = ? AND name = ?",
        CLIENTS_TABLE
    ))
    .bind(user_id)
    .bind(client_name)
    .fetch_optional(tx.deref_mut())
    .await?;

    Ok(result)
}

#[derive(Debug, Clone)]
pub struct ClientRow {
    pub id: Id,
    pub name: String,
    pub client_id: String,
    pub client_secret: String,
    pub user_id: Id,
    pub created_at: Timestamp,
}

impl<'r> FromRow<'r, SqliteRow> for ClientRow {
    fn from_row(row: &'r SqliteRow) -> std::result::Result<ClientRow, SqlxError> {
        Ok(ClientRow {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            client_id: row.try_get("client_id")?,
            client_secret: row.try_get("client_secret")?,
            user_id: row.try_get("user_id")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql")
    )]
    async fn test_create_client(pool: SqlitePool) {
        let mut tx = pool.begin().await.unwrap();

        let now = chrono::Utc::now().timestamp();
        let user_id = 1;

        // Create a new client
        let client = create_client(
            &mut tx,
            user_id,
            "Test Client",
            "test_client_id",
            "test_client_secret",
            now,
        )
        .await
        .unwrap();

        // Verify the created client has the correct fields
        assert_eq!(client.name, "Test Client");
        assert_eq!(client.client_id, "test_client_id");
        assert_eq!(client.client_secret, "test_client_secret");
        assert_eq!(client.user_id, user_id);
        assert_eq!(client.created_at, now);
        assert!(client.id > 0, "Client should have a positive id");

        // Verify client is in database by finding it
        let found_client = find_by_user_id_client_id_and_secret(
            &mut tx,
            user_id,
            "test_client_id",
            "test_client_secret",
        )
        .await
        .unwrap();

        assert!(found_client.is_some(), "Should find newly created client");
        let found_client = found_client.unwrap();
        assert_eq!(found_client.id, client.id);
        assert_eq!(found_client.name, client.name);
        assert_eq!(found_client.client_id, client.client_id);
        assert_eq!(found_client.client_secret, client.client_secret);
        assert_eq!(found_client.user_id, client.user_id);
        assert_eq!(found_client.created_at, client.created_at);
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_find_by_user_id_client_id_and_secret(pool: SqlitePool) {
        // Test successful lookup with correct credentials
        let mut tx = pool.begin().await.unwrap();
        let client = find_by_user_id_client_id_and_secret(&mut tx, 1, "client_1", "secret_1")
            .await
            .unwrap();

        assert!(
            client.is_some(),
            "Should find client with correct credentials"
        );
        let client = client.unwrap();
        assert_eq!(client.id, 1, "Client should have id 1");
        assert_eq!(client.user_id, 1, "Client should belong to user 1");
        assert_eq!(client.client_id, "client_1", "Client ID should match");
        assert_eq!(
            client.client_secret, "secret_1",
            "Client secret should match"
        );
        assert_eq!(
            client.created_at, 1687895200,
            "Created timestamp should match"
        );

        // Test successful lookup for second client
        let client = find_by_user_id_client_id_and_secret(&mut tx, 1, "client_2", "secret_2")
            .await
            .unwrap();

        assert!(
            client.is_some(),
            "Should find second client with correct credentials"
        );
        let client = client.unwrap();
        assert_eq!(client.id, 2, "Client should have id 2");
        assert_eq!(client.client_id, "client_2", "Client ID should match");

        // Test failure with wrong client_secret
        let no_client =
            find_by_user_id_client_id_and_secret(&mut tx, 1, "client_1", "wrong_secret")
                .await
                .unwrap();

        assert!(
            no_client.is_none(),
            "Should not find client with wrong secret"
        );

        // Test failure with wrong client_id
        let no_client =
            find_by_user_id_client_id_and_secret(&mut tx, 1, "wrong_client", "secret_1")
                .await
                .unwrap();

        assert!(
            no_client.is_none(),
            "Should not find client with wrong client_id"
        );

        // Test failure with wrong user_id
        let no_client = find_by_user_id_client_id_and_secret(&mut tx, 999, "client_1", "secret_1")
            .await
            .unwrap();

        assert!(
            no_client.is_none(),
            "Should not find client with wrong user_id"
        );

        // Test failure with all wrong parameters
        let mut tx = pool.begin().await.unwrap();
        let no_client =
            find_by_user_id_client_id_and_secret(&mut tx, 999, "wrong_client", "wrong_secret")
                .await
                .unwrap();

        assert!(
            no_client.is_none(),
            "Should not find client with all wrong parameters"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_find_by_client_name_and_user_id(pool: SqlitePool) {
        // Test successful lookup with valid user_id and client name
        let mut tx = pool.begin().await.unwrap();
        let client = find_by_client_name_and_user_id(&mut tx, 1, "Android app")
            .await
            .unwrap();

        assert!(
            client.is_some(),
            "Should find client with user_id=1 and name='Android app'"
        );

        let client = client.unwrap();
        assert_eq!(client.id, 3, "Client id should be 3");
        assert_eq!(client.user_id, 1, "User id should be 1");
        assert_eq!(
            client.client_id, "android_client_id",
            "Client ID should match"
        );
        assert_eq!(
            client.client_secret, "android_client_secret",
            "Client secret should match"
        );

        // Test failure with wrong user_id
        let no_client = find_by_client_name_and_user_id(&mut tx, 999, "Android app")
            .await
            .unwrap();

        assert!(
            no_client.is_none(),
            "Should not find client with wrong user_id"
        );

        // Test failure with wrong client name
        let no_client = find_by_client_name_and_user_id(&mut tx, 1, "Nonexistent App")
            .await
            .unwrap();

        assert!(
            no_client.is_none(),
            "Should not find client with wrong name"
        );

        // Test failure with both wrong parameters
        let no_client = find_by_client_name_and_user_id(&mut tx, 999, "Nonexistent App")
            .await
            .unwrap();

        assert!(
            no_client.is_none(),
            "Should not find client with all wrong parameters"
        );
    }
}
