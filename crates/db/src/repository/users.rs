use sqlx::{Error, Row, prelude::FromRow, sqlite::SqliteRow};

use result::ArticlerResult;

use super::{Db, Id, Timestamp, USERS_TABLE};

pub async fn create_user<'c, C>(
    conn: C,
    username: &str,
    password_hash: &str,
    name: &str,
    email: &str,
    created_at: Timestamp,
    updated_at: Timestamp,
) -> ArticlerResult<UserRow>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    Ok(sqlx::query_as::<_, UserRow>(&format!("INSERT INTO {USERS_TABLE} (username, email, name, password_hash, created_at, updated_at) VALUES(?, ?, ?, ?, ?, ?) RETURNING *;"))
            .bind(username)
            .bind(email)
            .bind(name)
            .bind(password_hash)
            .bind(created_at)
            .bind(updated_at)
            .fetch_one(&mut *conn)
            .await?)
}

pub async fn find_by_username_and_password<'c, C>(
    conn: C,
    username: &str,
    password_hash: &str,
) -> ArticlerResult<Option<UserRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    let result = sqlx::query_as::<_, UserRow>(&format!(
        "SELECT * FROM {} WHERE username = ? AND password_hash = ?",
        super::USERS_TABLE
    ))
    .bind(username)
    .bind(password_hash)
    .fetch_optional(&mut *conn)
    .await?;

    Ok(result)
}

pub async fn find_by_username<'c, C>(conn: C, username: &str) -> ArticlerResult<Option<UserRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    let result =
        sqlx::query_as::<_, UserRow>(&format!("SELECT * FROM {USERS_TABLE} WHERE username = ?"))
            .bind(username)
            .fetch_optional(&mut *conn)
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
    fn from_row(row: &'r SqliteRow) -> std::result::Result<UserRow, Error> {
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
    use sqlx::SqlitePool;

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_create_user(pool: SqlitePool) {
        let now = chrono::Utc::now().timestamp();
        let password_hash = "$argon2id$v=19$m=19456,t=2,p=1$test$testhash";

        let user = create_user(
            &pool,
            "testuser",
            password_hash,
            "Test User",
            "test@example.com",
            now,
            now,
        )
        .await
        .unwrap();

        assert_eq!(user.username, "testuser");
        assert_eq!(user.password_hash, password_hash);
        assert_eq!(user.name, "Test User");
        assert_eq!(user.email, "test@example.com");
        assert_eq!(user.created_at, now);
        assert_eq!(user.updated_at, now);
        assert!(user.id > 0, "User should have a positive id");

        let found_user = find_by_username(&pool, "testuser").await.unwrap();

        assert!(found_user.is_some(), "Should find newly created user");
        let found_user = found_user.unwrap();
        assert_eq!(found_user.id, user.id);
        assert_eq!(found_user.username, user.username);
        assert_eq!(found_user.email, user.email);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn test_create_user_duplicate_username(pool: SqlitePool) {
        let mut tx = pool.begin().await.unwrap();

        let now = chrono::Utc::now().timestamp();
        let password_hash = "$argon2id$v=19$m=19456,t=2,p=1$test$testhash";

        let first_user = create_user(
            &pool,
            "duplicateuser",
            password_hash,
            "First User",
            "first@example.com",
            now,
            now,
        )
        .await
        .unwrap();

        assert_eq!(first_user.username, "duplicateuser");

        let result = create_user(
            &mut *tx,
            "duplicateuser",
            "$argon2id$v=19$m=19456,t=2,p=1$other$otherhash",
            "Second User",
            "second@example.com",
            now,
            now,
        )
        .await;

        assert!(
            result.is_err(),
            "Should fail when creating user with duplicate username"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_find_by_username(pool: SqlitePool) {
        let user = find_by_username(&pool, "wallabag").await.unwrap();

        assert!(user.is_some(), "Should find existing user");
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

        let no_user = find_by_username(&pool, "nonexistent").await.unwrap();
        assert!(no_user.is_none(), "Should not find non-existent user");
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_find_by_username_and_password(pool: SqlitePool) {
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

        let no_user = find_by_username_and_password(&pool, "wallabag", "wrong_hash")
            .await
            .unwrap();

        assert!(
            no_user.is_none(),
            "Should not find user with wrong password hash"
        );

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

        let no_user = find_by_username_and_password(&pool, "wrong_user", "wrong_hash")
            .await
            .unwrap();

        assert!(
            no_user.is_none(),
            "Should not find user with wrong username and password"
        );
    }
}
