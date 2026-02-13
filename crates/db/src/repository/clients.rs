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

pub async fn find_by_user_id(
    tx: &mut sqlx::Transaction<'_, Db>,
    user_id: Id,
) -> ArticlerResult<Vec<ClientRow>> {
    let result = sqlx::query_as::<_, ClientRow>(&format!(
        "SELECT * FROM {} WHERE user_id = ? ORDER BY id;",
        CLIENTS_TABLE
    ))
    .bind(user_id)
    .fetch_all(tx.deref_mut())
    .await?;

    Ok(result)
}

#[derive(Debug, Clone, PartialEq)]
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
        let expected_client = ClientRow {
            id: client.id, // ID is auto-generated, so we use the returned value
            name: "Test Client".to_string(),
            client_id: "test_client_id".to_string(),
            client_secret: "test_client_secret".to_string(),
            user_id,
            created_at: now,
        };

        assert!(client.id > 0, "Client should have a positive id");
        assert_eq!(client, expected_client);

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
        assert_eq!(found_client.unwrap(), expected_client);
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

        let expected_client = ClientRow {
            id: 1,
            name: "Client 1".to_string(),
            client_id: "client_1".to_string(),
            client_secret: "secret_1".to_string(),
            user_id: 1,
            created_at: 1687895200,
        };

        assert_eq!(client, Some(expected_client));

        // Test successful lookup for second client
        let client = find_by_user_id_client_id_and_secret(&mut tx, 1, "client_2", "secret_2")
            .await
            .unwrap();

        let expected_client_2 = ClientRow {
            id: 2,
            name: "Client 2".to_string(),
            client_id: "client_2".to_string(),
            client_secret: "secret_2".to_string(),
            user_id: 1,
            created_at: 1687895300,
        };

        assert_eq!(client, Some(expected_client_2));

        // Test failure with wrong client_secret
        let no_client =
            find_by_user_id_client_id_and_secret(&mut tx, 1, "client_1", "wrong_secret")
                .await
                .unwrap();

        assert_eq!(no_client, None);

        // Test failure with wrong client_id
        let no_client =
            find_by_user_id_client_id_and_secret(&mut tx, 1, "wrong_client", "secret_1")
                .await
                .unwrap();

        assert_eq!(no_client, None);

        // Test failure with wrong user_id
        let no_client = find_by_user_id_client_id_and_secret(&mut tx, 999, "client_1", "secret_1")
            .await
            .unwrap();

        assert_eq!(no_client, None);

        // Test failure with all wrong parameters
        let mut tx = pool.begin().await.unwrap();
        let no_client =
            find_by_user_id_client_id_and_secret(&mut tx, 999, "wrong_client", "wrong_secret")
                .await
                .unwrap();

        assert_eq!(no_client, None);
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

        let expected_client = ClientRow {
            id: 3,
            name: "Android app".to_string(),
            client_id: "android_client_id".to_string(),
            client_secret: "android_client_secret".to_string(),
            user_id: 1,
            created_at: 1687895400,
        };

        assert_eq!(client, Some(expected_client));

        // Test failure with wrong user_id
        let no_client = find_by_client_name_and_user_id(&mut tx, 999, "Android app")
            .await
            .unwrap();

        assert_eq!(no_client, None);

        // Test failure with wrong client name
        let no_client = find_by_client_name_and_user_id(&mut tx, 1, "Nonexistent App")
            .await
            .unwrap();

        assert_eq!(no_client, None);

        // Test failure with both wrong parameters
        let no_client = find_by_client_name_and_user_id(&mut tx, 999, "Nonexistent App")
            .await
            .unwrap();

        assert_eq!(no_client, None);
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql")
    )]
    async fn test_find_by_user_id(pool: SqlitePool) {
        // Test finding all clients for user 1 (should have 3 clients)
        let mut tx = pool.begin().await.unwrap();
        let mut clients = find_by_user_id(&mut tx, 1).await.unwrap();
        clients.sort_by_key(|c| c.id);

        let expected_clients = vec![
            ClientRow {
                id: 1,
                name: "Client 1".to_string(),
                client_id: "client_1".to_string(),
                client_secret: "secret_1".to_string(),
                user_id: 1,
                created_at: 1687895200,
            },
            ClientRow {
                id: 2,
                name: "Client 2".to_string(),
                client_id: "client_2".to_string(),
                client_secret: "secret_2".to_string(),
                user_id: 1,
                created_at: 1687895300,
            },
            ClientRow {
                id: 3,
                name: "Android app".to_string(),
                client_id: "android_client_id".to_string(),
                client_secret: "android_client_secret".to_string(),
                user_id: 1,
                created_at: 1687895400,
            },
        ];

        assert_eq!(clients, expected_clients);

        // Test finding all clients for user 2 (should have 1 client)
        let mut tx = pool.begin().await.unwrap();
        let clients = find_by_user_id(&mut tx, 2).await.unwrap();

        let expected_clients = vec![ClientRow {
            id: 4,
            name: "Client 4".to_string(),
            client_id: "client_4".to_string(),
            client_secret: "secret_4".to_string(),
            user_id: 2,
            created_at: 1687895200,
        }];

        assert_eq!(clients, expected_clients);
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql")
    )]
    async fn test_find_by_user_id_nonexistent_user(pool: SqlitePool) {
        // Test finding clients for non-existent user (should return empty vector)
        let mut tx = pool.begin().await.unwrap();
        let clients = find_by_user_id(&mut tx, 999).await.unwrap();

        assert_eq!(clients, Vec::<ClientRow>::new());
    }
}
