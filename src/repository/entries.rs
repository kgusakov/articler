use std::fmt::Display;

use indexmap::IndexMap;
use sqlx::{
    Database, Encode, FromRow, QueryBuilder, Row, Type, query_builder::Separated, sqlite::SqliteRow,
};

use super::{
    Db, DbError, ENTRIES_TABLE, ENTRIES_TAG_TABLE, Id, ReadingTime, Result, TAGS_TABLE, Timestamp,
};

pub type FullEntry = (EntryRow, Vec<crate::repository::tags::TagRow>);

pub async fn find_all(
    executor: impl sqlx::Executor<'_, Database = Db>,
    params: &EntriesCriteria,
) -> Result<Vec<(EntryRow, Vec<crate::repository::tags::TagRow>)>> {
    let mut q_builder = QueryBuilder::new(format!(
        r#"SELECT e.*, t.id as tag_id, t.label as tag_label, t.slug as tag_slug FROM {ENTRIES_TABLE} as e LEFT JOIN {ENTRIES_TAG_TABLE} et on et.entry_id = e.id LEFT JOIN {TAGS_TABLE} t on t.id = et.tag_id
        WHERE e.id in (
            SELECT id FROM {ENTRIES_TABLE}
            WHERE user_id = "#
    ));

    q_builder.push_bind(params.user_id);

    if let Some(a) = params.archive {
        q_builder.push(" AND is_archived = ");
        q_builder.push_bind(a);
    }

    if let Some(s) = params.starred {
        q_builder.push(" AND is_starred = ");
        q_builder.push_bind(s);
    }

    if let Some(p) = params.public {
        q_builder.push(" AND is_public = ");
        q_builder.push_bind(p);
    }

    if let Some(d) = params.since {
        q_builder.push(" AND updated_at > ");
        q_builder.push_bind(d);
    }

    if let Some(column) = &params.sort {
        q_builder.push(" ORDER BY ");
        q_builder.push(column.to_string());

        if let Some(order) = &params.order {
            q_builder.push(" ");
            q_builder.push(order.to_string());
        }
    }

    if let Some(pp) = params.per_page {
        q_builder.push(" LIMIT ");
        q_builder.push_bind(pp);

        if let Some(p) = params.page {
            q_builder.push(" OFFSET ");
            q_builder.push_bind((p - 1) * pp);
        }
    }
    q_builder.push(")");

    if let Some(column) = &params.sort {
        q_builder.push(" ORDER BY ");
        q_builder.push(column.to_string());

        if let Some(order) = &params.order {
            q_builder.push(" ");
            q_builder.push(order.to_string());
        }
    }

    // TODO implement detail filtering
    if params.detail == Some(Detail::Metadata) {
        return Err(DbError::RepositoryError(
            "Detail metadata mode is not supported yet".into(),
        ));
    }

    // TODO implement domain_name filtering
    if params.domain_name.is_some() {
        return Err(DbError::RepositoryError(
            "Domain filtering is not supported yet".into(),
        ));
    }

    // TODO implement tags filtering
    if params.tags.is_some() {
        return Err(DbError::RepositoryError(
            "Tags filtering is not supported yet".into(),
        ));
    }

    let raw_rows = q_builder.build().fetch_all(executor).await?;

    let mut entrs = IndexMap::<i32, Vec<&SqliteRow>>::new();

    for r in &raw_rows {
        let id: i32 = r.try_get("id")?;
        entrs.entry(id).and_modify(|v| v.push(r)).or_insert(vec![r]);
    }

    let mut entrs_with_relations = vec![];

    for e in entrs {
        let mut tags = vec![];

        for r in &e.1 {
            tags.push(crate::repository::tags::TagRow {
                id: r.try_get("tag_id")?,
                user_id: r.try_get("user_id")?,
                label: r.try_get("tag_label")?,
                slug: r.try_get("tag_slug")?,
            });
        }

        entrs_with_relations.push((EntryRow::from_row(e.1[0])?, tags));
    }

    Ok(entrs_with_relations)
}

pub async fn exists_by_id(
    executor: impl sqlx::Executor<'_, Database = Db>,
    user_id: Id,
    id: Id,
) -> Result<bool> {
    let result: i32 = sqlx::query_scalar(&format!(
        "SELECT EXISTS(SELECT 1 FROM {ENTRIES_TABLE} WHERE user_id = ? AND id = ?)",
    ))
    .bind(user_id)
    .bind(id)
    .fetch_one(executor)
    .await?;

    Ok(result == 1)
}

pub async fn delete_tag_by_tag_id(
    executor: impl sqlx::Executor<'_, Database = Db>,
    user_id: Id,
    id: Id,
    tag_id: Id,
) -> Result<bool> {
    let result = sqlx::query(&format!(
        r#"DELETE FROM {ENTRIES_TAG_TABLE} WHERE tag_id = ? AND entry_id in (SELECT id FROM {ENTRIES_TABLE} WHERE id = ? AND user_id = ?)"#
    ))
    .bind(tag_id)
    .bind(id)
    .bind(user_id)
    .execute(executor)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn count(
    executor: impl sqlx::Executor<'_, Database = Db>,
    params: &EntriesCriteria,
) -> Result<i64> {
    // TODO rewrite this funny stupid count
    let mut q_builder = QueryBuilder::new(format!(
        r#"SELECT COUNT(DISTINCT e.id) FROM {ENTRIES_TABLE} as e LEFT JOIN {ENTRIES_TAG_TABLE} et on et.entry_id = e.id LEFT JOIN {TAGS_TABLE} t on t.id = et.tag_id"#,
    ));
    q_builder.push(" WHERE e.user_id = ");
    q_builder.push_bind(params.user_id);

    if let Some(a) = params.archive {
        q_builder.push(" AND is_archived = ");
        q_builder.push_bind(a);
    }

    if let Some(s) = params.starred {
        q_builder.push(" AND is_starred = ");
        q_builder.push_bind(s);
    }

    if let Some(p) = params.public {
        q_builder.push(" AND is_public = ");
        q_builder.push_bind(p);
    }

    if let Some(d) = params.since {
        q_builder.push(" AND updated_at > ");
        q_builder.push_bind(d);
    }

    // TODO implement detail filtering
    if params.detail != Some(Detail::Full) {
        return Err(DbError::RepositoryError(
            "Detail metadata mode is not supported yet".into(),
        ));
    }

    // TODO implement domain_name filtering
    if params.domain_name.is_some() {
        return Err(DbError::RepositoryError(
            "Domain filtering is not supported yet".into(),
        ));
    }

    // TODO implement tags filtering
    if params.tags.is_some() {
        return Err(DbError::RepositoryError(
            "Tags filtering is not supported yet".into(),
        ));
    }

    Ok(q_builder.build().fetch_one(executor).await?.get(0))
}

pub async fn create(
    pool: &sqlx::SqlitePool,
    entry: CreateEntry,
    tags: &[crate::repository::tags::CreateTag],
) -> Result<(EntryRow, Vec<crate::repository::tags::TagRow>)> {
    let id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO entries (
            user_id, url, hashed_url, given_url, hashed_given_url, title, content, is_archived, archived_at,
            is_starred, starred_at, created_at, updated_at, mimetype,
            language, reading_time, domain_name, preview_picture,
            origin_url, published_at, published_by, is_public, uid
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        RETURNING id
        "#,
    )
    .bind(entry.user_id)
    .bind(entry.url)
    .bind(entry.hashed_url)
    .bind(entry.given_url)
    .bind(entry.hashed_given_url)
    .bind(entry.title)
    .bind(entry.content)
    .bind(entry.is_archived)
    .bind(entry.archived_at)
    .bind(entry.is_starred)
    .bind(entry.starred_at)
    .bind(entry.created_at)
    .bind(entry.updated_at)
    .bind(entry.mimetype)
    .bind(entry.language)
    .bind(entry.reading_time)
    .bind(entry.domain_name)
    .bind(entry.preview_picture)
    .bind(entry.origin_url)
    .bind(entry.published_at)
    .bind(entry.published_by)
    .bind(entry.is_public)
    .bind(entry.uid)
    .fetch_one(pool)
    .await?;

    if !tags.is_empty() {
        crate::repository::tags::create_and_link_tags(pool, id, tags).await?;
    }

    let entry = sqlx::query_as::<_, EntryRow>("SELECT * FROM entries WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;

    let tags = sqlx::query_as::<_, crate::repository::tags::TagRow>(&format!(
        r#"
        SELECT t.* FROM {} as et
        LEFT JOIN {} t on t.id = et.tag_id
        WHERE et.entry_id = ?
        "#,
        ENTRIES_TAG_TABLE, TAGS_TABLE
    ))
    .bind(entry.id)
    .fetch_all(pool)
    .await?;

    Ok((entry, tags))
}

pub async fn find_by_id(pool: &sqlx::SqlitePool, user_id: Id, id: Id) -> Result<Option<FullEntry>> {
    let entry = sqlx::query_as::<_, EntryRow>(&format!(
        "SELECT * FROM {ENTRIES_TABLE} WHERE user_id = ? AND id = ?"
    ))
    .bind(user_id)
    .bind(id)
    .fetch_optional(pool)
    .await?;

    let entry = match entry {
        Some(e) => e,
        None => return Ok(None),
    };

    let tags = sqlx::query_as::<_, crate::repository::tags::TagRow>(&format!(
        r#"
        SELECT t.* FROM {} as et
        LEFT JOIN {} t on t.id = et.tag_id
        WHERE et.entry_id = ?
        "#,
        ENTRIES_TAG_TABLE, TAGS_TABLE
    ))
    .bind(id)
    .fetch_all(pool)
    .await?;

    Ok(Some((entry, tags)))
}

pub async fn update_by_id(
    executor: impl sqlx::Executor<'_, Database = Db>,
    user_id: Id,
    id: Id,
    update: UpdateEntry,
) -> Result<bool> {
    let mut query_builder = QueryBuilder::new(format!("UPDATE {} SET ", ENTRIES_TABLE));

    let mut separated = query_builder.separated(", ");

    if let Some(title) = update.title {
        separated.push("title = ");
        push_bind_or_default(&mut separated, title);
    }

    if let Some(content) = update.content {
        separated.push("content = ");
        push_bind_or_default(&mut separated, content);
    }

    if let Some(is_archived) = update.is_archived {
        separated.push("is_archived = ");
        push_bind_or_default(&mut separated, is_archived);
    }

    if let Some(archived_at) = update.archived_at {
        separated.push("archived_at = ");
        push_bind_or_default(&mut separated, archived_at);
    }

    if let Some(is_starred) = update.is_starred {
        separated.push("is_starred = ");
        push_bind_or_default(&mut separated, is_starred);
    }

    if let Some(starred_at) = update.starred_at {
        separated.push("starred_at = ");
        push_bind_or_default(&mut separated, starred_at);
    }

    if let Some(language) = update.language {
        separated.push("language = ");
        push_bind_or_default(&mut separated, language);
    }

    if let Some(reading_time) = update.reading_time {
        separated.push("reading_time = ");
        push_bind_or_default(&mut separated, reading_time);
    }

    if let Some(preview_picture) = update.preview_picture {
        separated.push("preview_picture = ");
        push_bind_or_default(&mut separated, preview_picture);
    }

    if let Some(origin_url) = update.origin_url {
        separated.push("origin_url = ");
        push_bind_or_default(&mut separated, origin_url);
    }

    if let Some(published_at) = update.published_at {
        separated.push("published_at = ");
        push_bind_or_default(&mut separated, published_at);
    }

    if let Some(published_by) = update.published_by {
        separated.push("published_by = ");
        push_bind_or_default(&mut separated, published_by);
    }

    if let Some(is_public) = update.is_public {
        separated.push("is_public = ");
        push_bind_or_default(&mut separated, is_public);
    }

    if let Some(uid) = update.uid {
        separated.push("uid = ");
        push_bind_or_default(&mut separated, uid);
    }

    separated.push("updated_at = ");
    separated.push_bind_unseparated(update.updated_at);

    query_builder.push(" WHERE id = ");
    query_builder.push_bind(id);

    query_builder.push(" AND user_id = ");
    query_builder.push_bind(user_id);

    let result = query_builder.build().execute(executor).await?;

    Ok(result.rows_affected() > 0)
}

pub async fn delete_by_id(
    executor: impl sqlx::Executor<'_, Database = Db>,
    user_id: Id,
    id: Id,
) -> Result<bool> {
    let result = sqlx::query("DELETE FROM entries WHERE user_id = ? AND id = ?")
        .bind(user_id)
        .bind(id)
        .execute(executor)
        .await?;

    Ok(result.rows_affected() > 0)
}

fn push_bind_or_default<'qb, 'args, DB, T>(
    builder: &mut Separated<'qb, 'args, DB, &str>,
    value: Option<T>,
) where
    DB: Database,
    T: 'args + Encode<'args, DB> + Type<DB>,
{
    match value {
        Some(v) => builder.push_bind_unseparated(v),
        // SQLite is not support DEFAULT in UPDATE query
        None => builder.push_unseparated("NULL"),
    };
}

#[derive(Debug)]
pub struct EntryRow {
    pub id: Id,
    pub user_id: Id,
    pub url: String,
    pub hashed_url: Option<String>,
    pub given_url: Option<String>,
    pub hashed_given_url: Option<String>,
    pub title: String,
    pub content: String,
    pub is_archived: bool,
    pub archived_at: Option<Timestamp>,
    pub is_starred: bool,
    pub starred_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub mimetype: Option<String>,
    pub language: Option<String>,
    pub reading_time: ReadingTime,
    pub domain_name: String,
    pub preview_picture: Option<String>,
    pub origin_url: Option<String>,
    pub published_at: Option<Timestamp>,
    pub published_by: Option<String>,
    pub is_public: Option<bool>,
    pub uid: Option<String>,
}

impl<'r> sqlx::FromRow<'r, SqliteRow> for EntryRow {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> std::result::Result<EntryRow, sqlx::Error> {
        Ok(EntryRow {
            id: row.try_get("id")?,
            user_id: row.try_get("user_id")?,
            url: row.try_get("url")?,
            hashed_url: row.try_get("hashed_url")?,
            given_url: row.try_get("given_url")?,
            hashed_given_url: row.try_get("hashed_given_url")?,
            title: row.try_get("title")?,
            content: row.try_get("content")?,
            is_archived: row.try_get("is_archived")?,
            archived_at: row.try_get("archived_at")?,
            is_starred: row.try_get("is_starred")?,
            starred_at: row.try_get("starred_at")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            mimetype: row.try_get("mimetype")?,
            language: row.try_get("language")?,
            reading_time: row.try_get("reading_time")?,
            domain_name: row.try_get("domain_name")?,
            preview_picture: row.try_get("preview_picture")?,
            origin_url: row.try_get("origin_url")?,
            published_at: row.try_get("published_at")?,
            published_by: row.try_get("published_by")?,
            is_public: row.try_get("is_public")?,
            uid: row.try_get("uid")?,
        })
    }
}

#[derive(Debug)]
pub struct CreateEntry {
    pub user_id: Id,
    pub url: String,
    pub hashed_url: String,
    pub given_url: String,
    pub hashed_given_url: String,
    pub title: String,
    pub content: String,
    pub is_archived: bool,
    pub archived_at: Option<Timestamp>,
    pub is_starred: bool,
    pub starred_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub mimetype: Option<String>,
    pub language: Option<String>,
    pub reading_time: i32,
    pub domain_name: String,
    pub preview_picture: Option<String>,
    pub origin_url: Option<String>,
    pub published_at: Option<Timestamp>,
    pub published_by: Option<String>,
    pub is_public: Option<bool>,
    pub uid: Option<String>,
}

// None - don't update anything
// Some(None) - update to default value
// Some(Some(v)) - update to v
type UpdateField<T> = Option<Option<T>>;

#[derive(Debug)]
pub struct UpdateEntry {
    pub title: UpdateField<String>,
    pub content: UpdateField<String>,
    pub is_archived: UpdateField<bool>,
    pub archived_at: UpdateField<Timestamp>,
    pub is_starred: UpdateField<bool>,
    pub starred_at: UpdateField<Timestamp>,
    pub updated_at: Timestamp,
    pub language: UpdateField<String>,
    pub reading_time: UpdateField<ReadingTime>,
    pub preview_picture: UpdateField<String>,
    pub origin_url: UpdateField<String>,
    pub published_at: UpdateField<Timestamp>,
    pub published_by: UpdateField<String>,
    pub is_public: UpdateField<bool>,
    pub uid: UpdateField<String>,
}

pub enum SortColumn {
    Created,
    Updated,
    Archived,
}

impl Display for SortColumn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SortColumn::Created => write!(f, "created_at"),
            SortColumn::Updated => write!(f, "updated_at"),
            SortColumn::Archived => write!(f, "archived_at"),
        }
    }
}

pub enum SortOrder {
    Asc,
    Desc,
}

impl Display for SortOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SortOrder::Asc => write!(f, "ASC"),
            SortOrder::Desc => write!(f, "DESC"),
        }
    }
}

#[derive(Default)]
pub struct EntriesCriteria {
    pub user_id: Id,
    pub archive: Option<bool>,
    pub starred: Option<bool>,
    pub sort: Option<SortColumn>,
    pub order: Option<SortOrder>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub since: Option<Timestamp>,
    pub public: Option<bool>,
    pub detail: Option<Detail>,
    pub domain_name: Option<String>,
}

#[derive(PartialEq)]
pub enum Detail {
    Full,
    Metadata,
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_exists_by_id(pool: SqlitePool) {
        let exists = exists_by_id(&pool, 1, 1).await.unwrap();
        assert!(exists, "Entry 1 should exist");

        let not_exists = exists_by_id(&pool, 1, 999).await.unwrap();
        assert!(!not_exists, "Entry 999 should not exist");
    }

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_delete_tag_by_tag_id(pool: SqlitePool) {
        // Entry 2 initially has 2 tags (label1/id=1, label2/id=2)
        let tags_before = crate::repository::tags::find_by_entry_id(&pool, 1, 2)
            .await
            .unwrap();
        assert_eq!(tags_before.len(), 2, "Entry 2 should have 2 tags initially");

        // Delete tag_id=1 from entry 2
        let deleted = delete_tag_by_tag_id(&pool, 1, 2, 1).await.unwrap();
        assert!(
            deleted,
            "Should successfully delete existing tag association"
        );

        // Verify only 1 tag remains
        let tags_after = crate::repository::tags::find_by_entry_id(&pool, 1, 2)
            .await
            .unwrap();
        assert_eq!(
            tags_after.len(),
            1,
            "Entry 2 should have 1 tag after deletion"
        );
        assert_eq!(tags_after[0].id, 2, "Only label2 should remain");

        // Try to delete same tag again - should return false
        let not_deleted = delete_tag_by_tag_id(&pool, 1, 2, 1).await.unwrap();
        assert!(
            !not_deleted,
            "Should return false for non-existent association"
        );

        // Try to delete tag from non-existent entry
        let not_deleted = delete_tag_by_tag_id(&pool, 1, 999, 1).await.unwrap();
        assert!(!not_deleted, "Should return false for non-existent entry");
    }

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_entry_delete_by_id(pool: SqlitePool) {
        // Verify entry 1 exists
        let entry_before = find_by_id(&pool, 1, 1).await.unwrap();
        assert!(entry_before.is_some(), "Entry 1 should exist");

        // Delete entry 1
        let deleted = delete_by_id(&pool, 1, 1).await.unwrap();
        assert!(deleted, "Should return true when entry is deleted");

        // Verify entry 1 no longer exists
        let entry_after = find_by_id(&pool, 1, 1).await.unwrap();
        assert!(
            entry_after.is_none(),
            "Entry 1 should not exist after deletion"
        );

        // Try deleting the same entry again
        let not_deleted = delete_by_id(&pool, 1, 1).await.unwrap();
        assert!(!not_deleted, "Should return false when entry doesn't exist");

        // Try deleting non-existent entry
        let not_deleted = delete_by_id(&pool, 1, 999).await.unwrap();
        assert!(!not_deleted, "Should return false for non-existent entry");

        // Verify entry 2 still exists (wasn't affected)
        let entry_2 = find_by_id(&pool, 1, 2).await.unwrap();
        assert!(entry_2.is_some(), "Entry 2 should still exist");
    }
}
