use sqlx::{Row, prelude::FromRow, sqlite::SqliteRow};

use super::*;

pub async fn find_by_username_and_password(
    executor: impl sqlx::Executor<'_, Database = Db>,
    username: &str,
    password_hash: &str,
) -> super::Result<Option<UserRow>> {
    let result = sqlx::query_as::<_, UserRow>(&format!(
        "SELECT * FROM {} WHERE username = ? AND password_hash = ?",
        super::USERS_TABLE
    ))
    .bind(username)
    .bind(password_hash)
    .fetch_optional(executor)
    .await?;

    Ok(result)
}

pub async fn find_by_username(
    executor: impl sqlx::Executor<'_, Database = Db>,
    username: &str,
) -> Result<Option<UserRow>> {
    let result =
        sqlx::query_as::<_, UserRow>(&format!("SELECT * FROM {} WHERE username = ?", USERS_TABLE))
            .bind(username)
            .fetch_optional(executor)
            .await?;

    Ok(result)
}

#[derive(Debug, Clone)]
pub struct UserRow {
    pub id: Id,
    pub username: String,
    pub email: String,
    pub name: String,
    pub password_hash: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl<'r> FromRow<'r, SqliteRow> for UserRow {
    fn from_row(row: &'r SqliteRow) -> std::result::Result<UserRow, SqlxError> {
        Ok(UserRow {
            id: row.try_get("id")?,
            username: row.try_get("username")?,
            email: row.try_get("email")?,
            name: row.try_get("name")?,
            password_hash: row.try_get("password_hash")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{SqlitePool, pool};

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_find_by_username_and_password(pool: SqlitePool) {
        // Test successful lookup with correct credentials
        let user = 
            find_by_username_and_password(
                &pool,
                "wallabag",
                "$argon2id$v=19$m=19456,t=2,p=1$hsWWj4oOAFTK2vLl7YjG0w$L+KcI0YL/8L8s2ZRRA9caoqEiyYE48Drm36y1KFk2bg",
            )
            .await
            .unwrap();

        assert!(user.is_some(), "Should find user with correct credentials");
        let user = user.unwrap();
        assert_eq!(user.id, 1, "User should have id 1");
        assert_eq!(user.username, "wallabag", "Username should match");
        assert_eq!(user.email, "wallabag@wallabag.io", "Email should match");
        assert_eq!(user.name, "Walla Baggger", "Name should match");
        assert_eq!(
            user.password_hash,
            "$argon2id$v=19$m=19456,t=2,p=1$hsWWj4oOAFTK2vLl7YjG0w$L+KcI0YL/8L8s2ZRRA9caoqEiyYE48Drm36y1KFk2bg",
            "Password hash should match"
        );

        // Test failure with wrong password hash
        let no_user =
            find_by_username_and_password(&pool, "wallabag", "wrong_hash")
            .await
            .unwrap();

        assert!(
            no_user.is_none(),
            "Should not find user with wrong password hash"
        );

        // Test failure with non-existent username
        let no_user =
            find_by_username_and_password(
                &pool,
                "nonexistent",
                "$argon2id$v=19$m=19456,t=2,p=1$hsWWj4oOAFTK2vLl7YjG0w$L+KcI0YL/8L8s2ZRRA9caoqEiyYE48Drm36y1KFk2bg",
            )
            .await
            .unwrap();

        assert!(
            no_user.is_none(),
            "Should not find user with non-existent username"
        );

        // Test failure with both wrong username and password
        let no_user =
            find_by_username_and_password(&pool, "wrong_user", "wrong_hash")
            .await
            .unwrap();

        assert!(
            no_user.is_none(),
            "Should not find user with wrong username and password"
        );
    }
}
