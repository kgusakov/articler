use std::path::Path;
use std::str::FromStr;

use sqlx::migrate::{Migrate, Migrator};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

const PER_USER_TAG_UNIQUENESS: i64 = 20_260_726_104_500;

/// `sqlx::test` applies every migration to an empty database before loading fixtures, so it can
/// never observe a migration that transforms pre-existing rows. These tests drive the real
/// `Migrator` instead: apply everything below the target version, seed, then apply the target.
async fn pool_migrated_below(version: i64) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite::memory:").unwrap();
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();

    let migrator = Migrator::new(Path::new("../../migrations")).await.unwrap();
    let mut conn = pool.acquire().await.unwrap();
    conn.ensure_migrations_table().await.unwrap();

    for migration in migrator
        .iter()
        .filter(|m| m.migration_type.is_up_migration() && m.version < version)
    {
        conn.apply(migration).await.unwrap();
    }

    pool
}

async fn apply(pool: &SqlitePool, version: i64) {
    let migrator = Migrator::new(Path::new("../../migrations")).await.unwrap();
    let migration = migrator
        .iter()
        .find(|m| m.migration_type.is_up_migration() && m.version == version)
        .expect("migration should exist");

    let mut conn = pool.acquire().await.unwrap();
    conn.apply(migration).await.unwrap();
}

async fn seed_cross_user_tags(pool: &SqlitePool) {
    sqlx::query(
        r"INSERT INTO users (id, username, email, name, password_hash, created_at, updated_at)
          VALUES (1, 'a', 'a@x', 'A', 'h', 0, 0), (2, 'b', 'b@x', 'B', 'h', 0, 0)",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r"INSERT INTO entries (
              id, user_id, url, hashed_url, given_url, hashed_given_url, title, content,
              content_text, reading_time, domain_name, created_at, updated_at
          ) VALUES
              (1, 1, 'u1', 'h1', 'u1', 'g1', 't1', 'c', 'c', 1, 'd', 0, 0),
              (2, 2, 'u2', 'h2', 'u2', 'g2', 't2', 'c', 'c', 1, 'd', 0, 0)",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r"INSERT INTO tags (id, user_id, label, slug)
          VALUES (1, 1, 'rust', 'rust'), (2, 1, 'db', 'db')",
    )
    .execute(pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO entry_tags (entry_id, tag_id) VALUES (1, 1), (1, 2), (2, 1)")
        .execute(pool)
        .await
        .unwrap();
}

async fn links(pool: &SqlitePool) -> Vec<(i64, i64)> {
    sqlx::query("SELECT entry_id, tag_id FROM entry_tags ORDER BY entry_id, tag_id")
        .fetch_all(pool)
        .await
        .unwrap()
        .iter()
        .map(|r| (r.get("entry_id"), r.get("tag_id")))
        .collect()
}

#[tokio::test]
async fn migration_keeps_owner_links_and_drops_cross_user_ones() {
    let pool = pool_migrated_below(PER_USER_TAG_UNIQUENESS).await;
    seed_cross_user_tags(&pool).await;

    assert_eq!(
        links(&pool).await,
        vec![(1, 1), (1, 2), (2, 1)],
        "seeded state: user 2's entry wrongly links to user 1's tag"
    );

    apply(&pool, PER_USER_TAG_UNIQUENESS).await;

    assert_eq!(
        links(&pool).await,
        vec![(1, 1), (1, 2)],
        "user 1's own links must survive; only the cross-owner link is dropped"
    );
}

#[tokio::test]
async fn migration_leaves_no_dangling_foreign_keys() {
    let pool = pool_migrated_below(PER_USER_TAG_UNIQUENESS).await;
    seed_cross_user_tags(&pool).await;
    apply(&pool, PER_USER_TAG_UNIQUENESS).await;

    let violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .unwrap()
        .len();

    assert_eq!(violations, 0, "migrated schema must have no FK violations");
}

#[tokio::test]
async fn migration_preserves_autoincrement_sequence() {
    let pool = pool_migrated_below(PER_USER_TAG_UNIQUENESS).await;
    seed_cross_user_tags(&pool).await;

    sqlx::query("INSERT INTO tags (id, user_id, label, slug) VALUES (9, 1, 'gone', 'gone')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM tags WHERE id = 9")
        .execute(&pool)
        .await
        .unwrap();

    apply(&pool, PER_USER_TAG_UNIQUENESS).await;

    let seq: i64 = sqlx::query_scalar("SELECT seq FROM sqlite_sequence WHERE name = 'tags'")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(seq, 9, "a deleted tag's id must never be handed out again");
}

#[tokio::test]
async fn migration_allows_same_label_per_user_afterwards() {
    let pool = pool_migrated_below(PER_USER_TAG_UNIQUENESS).await;
    seed_cross_user_tags(&pool).await;
    apply(&pool, PER_USER_TAG_UNIQUENESS).await;

    sqlx::query("INSERT INTO tags (user_id, label, slug) VALUES (2, 'rust', 'rust')")
        .execute(&pool)
        .await
        .expect("user 2 may own a label user 1 already has");

    let duplicate = sqlx::query("INSERT INTO tags (user_id, label, slug) VALUES (2, 'rust', 'rust')")
        .execute(&pool)
        .await;

    assert!(
        duplicate.is_err(),
        "one user still may not hold the same label twice"
    );
}
