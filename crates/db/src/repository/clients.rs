use sqlx::{Error, Row, prelude::FromRow, sqlite::SqliteRow};

use result::ArticlerResult;

use super::{CLIENTS_TABLE, Db, Timestamp};
use types::Id;

pub async fn create<'c, C>(
    conn: C,
    user_id: Id,
    client_name: &str,
    client_id: &str,
    client_secret: &str,
    created_at: Timestamp,
) -> ArticlerResult<ClientRow>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    Ok(sqlx::query_as::<_, ClientRow>(&format!("INSERT INTO {CLIENTS_TABLE} (name, client_id, client_secret, user_id, created_at) VALUES(?, ?, ?, ?, ?) RETURNING *;"))
            .bind(client_name)
            .bind(client_id)
            .bind(client_secret)
            .bind(user_id)
            .bind(created_at)
            .fetch_one(&mut *conn)
            .await?)
}

pub async fn delete_by_id<'c, C>(conn: C, user_id: Id, id: Id) -> ArticlerResult<bool>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    let result = sqlx::query(&format!(
        "DELETE FROM {CLIENTS_TABLE} WHERE user_id = ? AND id = ?"
    ))
    .bind(user_id)
    .bind(id)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn find_by_user_id_client_id_and_secret<'c, C>(
    conn: C,
    user_id: Id,
    client_id: &str,
    client_secret: &str,
) -> ArticlerResult<Option<ClientRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    let result = sqlx::query_as::<_, ClientRow>(&format!(
        "SELECT * FROM {CLIENTS_TABLE} WHERE user_id = ? AND client_id = ? AND client_secret = ?"
    ))
    .bind(user_id)
    .bind(client_id)
    .bind(client_secret)
    .fetch_optional(&mut *conn)
    .await?;

    Ok(result)
}

pub async fn find_by_client_id_and_secret<'c, C>(
    conn: C,
    client_id: &str,
    client_secret: &str,
) -> ArticlerResult<Option<ClientRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    let result = sqlx::query_as::<_, ClientRow>(&format!(
        "SELECT * FROM {CLIENTS_TABLE} WHERE client_id = ? AND client_secret = ?"
    ))
    .bind(client_id)
    .bind(client_secret)
    .fetch_optional(&mut *conn)
    .await?;

    Ok(result)
}

pub async fn find_by_client_name_and_user_id<'c, C>(
    conn: C,
    user_id: Id,
    client_name: &str,
) -> ArticlerResult<Option<ClientRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    let result = sqlx::query_as::<_, ClientRow>(&format!(
        "SELECT * FROM {CLIENTS_TABLE} WHERE user_id = ? AND name = ?"
    ))
    .bind(user_id)
    .bind(client_name)
    .fetch_optional(&mut *conn)
    .await?;

    Ok(result)
}

pub async fn find_by_user_id<'c, C>(conn: C, user_id: Id) -> ArticlerResult<Vec<ClientRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    let result = sqlx::query_as::<_, ClientRow>(&format!(
        "SELECT * FROM {CLIENTS_TABLE} WHERE user_id = ? ORDER BY id;"
    ))
    .bind(user_id)
    .fetch_all(&mut *conn)
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
    fn from_row(row: &'r SqliteRow) -> std::result::Result<ClientRow, Error> {
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
        let now = chrono::Utc::now().timestamp();
        let user_id = 1;

        let client = create(
            &pool,
            user_id,
            "Test Client",
            "test_client_id",
            "test_client_secret",
            now,
        )
        .await
        .unwrap();

        let expected_client = ClientRow {
            id: client.id, // ID is auto-generated, so we use the returned value
            name: "Test Client".to_owned(),
            client_id: "test_client_id".to_owned(),
            client_secret: "test_client_secret".to_owned(),
            user_id,
            created_at: now,
        };

        assert!(client.id > 0, "Client should have a positive id");
        assert_eq!(client, expected_client);

        let found_client = find_by_user_id_client_id_and_secret(
            &pool,
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
        let client = find_by_user_id_client_id_and_secret(&pool, 1, "client_1", "secret_1")
            .await
            .unwrap();

        let expected_client = ClientRow {
            id: 1,
            name: "Client 1".to_owned(),
            client_id: "client_1".to_owned(),
            client_secret: "secret_1".to_owned(),
            user_id: 1,
            created_at: 1_687_895_200,
        };

        assert_eq!(client, Some(expected_client));

        let client = find_by_user_id_client_id_and_secret(&pool, 1, "client_2", "secret_2")
            .await
            .unwrap();

        let expected_client_2 = ClientRow {
            id: 2,
            name: "Client 2".to_owned(),
            client_id: "client_2".to_owned(),
            client_secret: "secret_2".to_owned(),
            user_id: 1,
            created_at: 1_687_895_300,
        };

        assert_eq!(client, Some(expected_client_2));

        let no_client = find_by_user_id_client_id_and_secret(&pool, 1, "client_1", "wrong_secret")
            .await
            .unwrap();

        assert_eq!(no_client, None);

        let no_client = find_by_user_id_client_id_and_secret(&pool, 1, "wrong_client", "secret_1")
            .await
            .unwrap();

        assert_eq!(no_client, None);

        let no_client = find_by_user_id_client_id_and_secret(&pool, 999, "client_1", "secret_1")
            .await
            .unwrap();

        assert_eq!(no_client, None);

        let no_client =
            find_by_user_id_client_id_and_secret(&pool, 999, "wrong_client", "wrong_secret")
                .await
                .unwrap();

        assert_eq!(no_client, None);
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_find_by_client_name_and_user_id(pool: SqlitePool) {
        let client = find_by_client_name_and_user_id(&pool, 1, "Android app")
            .await
            .unwrap();

        let expected_client = ClientRow {
            id: 3,
            name: "Android app".to_owned(),
            client_id: "android_client_id".to_owned(),
            client_secret: "android_client_secret".to_owned(),
            user_id: 1,
            created_at: 1_687_895_400,
        };

        assert_eq!(client, Some(expected_client));

        let no_client = find_by_client_name_and_user_id(&pool, 999, "Android app")
            .await
            .unwrap();

        assert_eq!(no_client, None);

        let no_client = find_by_client_name_and_user_id(&pool, 1, "Nonexistent App")
            .await
            .unwrap();

        assert_eq!(no_client, None);

        let no_client = find_by_client_name_and_user_id(&pool, 999, "Nonexistent App")
            .await
            .unwrap();

        assert_eq!(no_client, None);
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql")
    )]
    async fn test_find_by_user_id(pool: SqlitePool) {
        let mut clients = find_by_user_id(&pool, 1).await.unwrap();
        clients.sort_by_key(|c| c.id);

        let expected_clients = vec![
            ClientRow {
                id: 1,
                name: "Client 1".to_owned(),
                client_id: "client_1".to_owned(),
                client_secret: "secret_1".to_owned(),
                user_id: 1,
                created_at: 1_687_895_200,
            },
            ClientRow {
                id: 2,
                name: "Client 2".to_owned(),
                client_id: "client_2".to_owned(),
                client_secret: "secret_2".to_owned(),
                user_id: 1,
                created_at: 1_687_895_300,
            },
            ClientRow {
                id: 3,
                name: "Android app".to_owned(),
                client_id: "android_client_id".to_owned(),
                client_secret: "android_client_secret".to_owned(),
                user_id: 1,
                created_at: 1_687_895_400,
            },
        ];

        assert_eq!(clients, expected_clients);

        let clients = find_by_user_id(&pool, 2).await.unwrap();

        let expected_clients = vec![ClientRow {
            id: 4,
            name: "Client 4".to_owned(),
            client_id: "client_4".to_owned(),
            client_secret: "secret_4".to_owned(),
            user_id: 2,
            created_at: 1_687_895_200,
        }];

        assert_eq!(clients, expected_clients);
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql")
    )]
    async fn test_find_by_user_id_nonexistent_user(pool: SqlitePool) {
        let clients = find_by_user_id(&pool, 999).await.unwrap();

        assert_eq!(clients, Vec::<ClientRow>::new());
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql")
    )]
    async fn test_delete_by_id(pool: SqlitePool) {
        let client_before = find_by_client_name_and_user_id(&pool, 1, "Client 1")
            .await
            .unwrap();
        assert!(
            client_before.is_some(),
            "Client should exist before deletion"
        );

        let deleted = delete_by_id(&pool, 1, 1).await.unwrap();
        assert!(deleted, "Delete should return true when client exists");

        let client_after = find_by_client_name_and_user_id(&pool, 1, "Client 1")
            .await
            .unwrap();
        assert_eq!(client_after, None, "Client should not exist after deletion");

        let deleted_again = delete_by_id(&pool, 1, 1).await.unwrap();
        assert!(
            !deleted_again,
            "Delete should return false when client doesn't exist"
        );

        let deleted_wrong_user = delete_by_id(&pool, 2, 2).await.unwrap();
        assert!(
            !deleted_wrong_user,
            "Delete should return false when user_id doesn't match"
        );

        let client_2_still_exists = find_by_client_name_and_user_id(&pool, 1, "Client 2")
            .await
            .unwrap();
        assert!(
            client_2_still_exists.is_some(),
            "Client 2 should still exist"
        );

        let deleted_nonexistent = delete_by_id(&pool, 1, 999).await.unwrap();
        assert!(
            !deleted_nonexistent,
            "Delete should return false for nonexistent client"
        );
    }
}
