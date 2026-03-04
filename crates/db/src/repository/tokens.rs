use sqlx::{prelude::*, sqlite::SqliteRow};

use crate::error::Result;
use crate::repository::{Db, TOKENS_TABLE, Timestamp};
use types::Id;

pub async fn create<'c, C>(
    conn: C,
    token: &str,
    user_id: Id,
    client_id: Id,
    created_at: Timestamp,
    expires_in: i64,
) -> Result<TokenRow>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    Ok(sqlx::query_as::<_, TokenRow>(&format!(
        "INSERT INTO {TOKENS_TABLE} (token, created_at, expires_at, user_id, client_id) VALUES(?, ?, strftime('%s', 'now') + ?, ?, ?) RETURNING *;"
    ))
    .bind(token)
    .bind(created_at)
    .bind(expires_in)
    .bind(user_id)
    .bind(client_id)
    .fetch_one(&mut *conn)
    .await?)
}

pub async fn delete<'c, C>(conn: C, token: &str) -> Result<Option<TokenRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    Ok(sqlx::query_as::<_, TokenRow>(&format!(
        "DELETE FROM {TOKENS_TABLE} WHERE token = ? RETURNING *"
    ))
    .bind(token)
    .fetch_optional(&mut *conn)
    .await?)
}

pub async fn delete_expired<'c, C>(conn: C) -> Result<()>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    sqlx::query(&format!(
        "DELETE FROM {TOKENS_TABLE} WHERE expires_at <= strftime('%s', 'now');"
    ))
    .execute(&mut *conn)
    .await?;

    Ok(())
}

pub async fn find<'c, C>(conn: C, token: &str) -> Result<Option<TokenRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    Ok(
        sqlx::query_as::<_, TokenRow>(&format!("SELECT * FROM {TOKENS_TABLE} WHERE token = ?;"))
            .bind(token)
            .fetch_optional(&mut *conn)
            .await?,
    )
}

pub struct TokenRow {
    pub id: Id,
    pub token: String,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
    pub user_id: Id,
    pub client_id: Id,
}

impl<'r> FromRow<'r, SqliteRow> for TokenRow {
    fn from_row(row: &'r SqliteRow) -> std::result::Result<TokenRow, sqlx::Error> {
        Ok(TokenRow {
            id: row.try_get("id")?,
            token: row.try_get("token")?,
            created_at: row.try_get("created_at")?,
            expires_at: row.try_get("expires_at")?,
            user_id: row.try_get("user_id")?,
            client_id: row.try_get("client_id")?,
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
    async fn test_create_token(pool: SqlitePool) {
        let token_str = "test_token_123";
        let user_id = 1;
        let client_id = 1;
        let created_at = chrono::Utc::now().timestamp();
        let expires_in = 3600; // 1 hour

        let token = create(&pool, token_str, user_id, client_id, created_at, expires_in)
            .await
            .unwrap();

        assert!(token.id > 0, "Token should have a positive id");
        assert_eq!(token.token, token_str);
        assert_eq!(token.user_id, user_id);
        assert_eq!(token.client_id, client_id);
        assert_eq!(token.created_at, created_at);
        assert!(
            token.expires_at > created_at,
            "Expires_at should be in the future"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql")
    )]
    async fn test_find_token_success(pool: SqlitePool) {
        let token_str = "findable_token";
        let user_id = 1;
        let client_id = 1;
        let created_at = chrono::Utc::now().timestamp();
        let expires_in = 3600;

        let created = create(&pool, token_str, user_id, client_id, created_at, expires_in)
            .await
            .unwrap();

        let found = find(&pool, token_str).await.unwrap();

        assert!(found.is_some(), "Should find the created token");
        let found = found.unwrap();
        assert_eq!(found.id, created.id);
        assert_eq!(found.token, created.token);
        assert_eq!(found.user_id, created.user_id);
        assert_eq!(found.client_id, created.client_id);
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql")
    )]
    async fn test_find_token_not_found(pool: SqlitePool) {
        let found = find(&pool, "nonexistent_token").await.unwrap();

        assert!(found.is_none(), "Should not find nonexistent token");
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql")
    )]
    async fn test_delete_token_success(pool: SqlitePool) {
        let token_str = "deletable_token";
        create(&pool, token_str, 1, 1, chrono::Utc::now().timestamp(), 3600)
            .await
            .unwrap();

        let before_delete = find(&pool, token_str).await.unwrap();
        assert!(
            before_delete.is_some(),
            "Token should exist before deletion"
        );

        delete(&pool, token_str).await.unwrap();

        let found = find(&pool, token_str).await.unwrap();
        assert!(found.is_none(), "Token should be deleted");
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql")
    )]
    async fn test_delete_token_not_found(pool: SqlitePool) {
        let deleted = delete(&pool, "nonexistent_token").await.unwrap();

        assert!(
            deleted.is_none(),
            "Should return None for nonexistent token"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql")
    )]
    async fn test_delete_expired_tokens(pool: SqlitePool) {
        let now = chrono::Utc::now().timestamp();

        let expired_token = "expired_token";
        create(&pool, expired_token, 1, 1, now, -7200)
            .await
            .unwrap();

        let valid_token = "valid_token";
        create(&pool, valid_token, 1, 1, now, 3600).await.unwrap();

        delete_expired(&pool).await.unwrap();

        let found_expired = find(&pool, expired_token).await.unwrap();
        assert!(found_expired.is_none(), "Expired token should be deleted");

        let found_valid = find(&pool, valid_token).await.unwrap();
        assert!(found_valid.is_some(), "Valid token should not be deleted");
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql")
    )]
    async fn test_create_token_with_different_users_and_clients(pool: SqlitePool) {
        let now = chrono::Utc::now().timestamp();

        let token1 = create(&pool, "token_1_1", 1, 1, now, 3600).await.unwrap();
        let token2 = create(&pool, "token_1_2", 1, 2, now, 3600).await.unwrap();
        let token3 = create(&pool, "token_2_4", 2, 4, now, 3600).await.unwrap();

        assert_eq!(token1.user_id, 1);
        assert_eq!(token1.client_id, 1);
        assert_eq!(token2.user_id, 1);
        assert_eq!(token2.client_id, 2);
        assert_eq!(token3.user_id, 2);
        assert_eq!(token3.client_id, 4);
    }
}
