use std::fmt::Display;

use chrono::Utc;
use indexmap::IndexMap;
use sqlx::{
    Acquire, Database, Encode, FromRow, QueryBuilder, Row, Type, query_builder::Separated,
    sqlite::SqliteRow,
};

use super::{Db, ENTRIES_TABLE, ENTRIES_TAG_TABLE, TAGS_TABLE, Timestamp};
use crate::error::{NotSupportedYetSnafu, Result};
use types::{Id, ReadingTime};

pub type FullEntry = (EntryRow, Vec<crate::repository::tags::TagRow>);

pub async fn find_all<'c, C>(
    conn: C,
    params: &FindParams,
) -> Result<Vec<(EntryRow, Vec<crate::repository::tags::TagRow>)>>
where
    C: Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;
    let mut q_builder = QueryBuilder::new(format!(
        r"SELECT e.*, t.id as tag_id, t.label as tag_label, t.slug as tag_slug FROM {ENTRIES_TABLE} as e LEFT JOIN {ENTRIES_TAG_TABLE} et on et.entry_id = e.id LEFT JOIN {TAGS_TABLE} t on t.id = et.tag_id
        WHERE e.id in (
            SELECT e2.id FROM {ENTRIES_TABLE} e2"
    ));

    if params.search.is_some() {
        q_builder.push(" JOIN entries_fts ON entries_fts.rowid = e2.id");
    }

    q_builder.push(" WHERE e2.user_id = ");
    q_builder.push_bind(params.user_id);

    if let Some(ref search) = params.search {
        q_builder.push(" AND entries_fts MATCH ");
        q_builder.push_bind(search.clone());
    }

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
        return NotSupportedYetSnafu {
            msg: "Detail metadata mode is not supported yet",
        }
        .fail();
    }

    // TODO implement domain_name filtering
    if params.domain_name.is_some() {
        return NotSupportedYetSnafu {
            msg: "Domain filtering is not supported yet",
        }
        .fail();
    }

    // TODO implement tags filtering
    if params.tags.is_some() {
        return NotSupportedYetSnafu {
            msg: "Tags filtering is not supported yet",
        }
        .fail();
    }

    let raw_rows = q_builder.build().fetch_all(&mut *conn).await?;

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

pub async fn exists_by_id<'c, C>(conn: C, user_id: Id, id: Id) -> Result<bool>
where
    C: Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    let result: i32 = sqlx::query_scalar(&format!(
        "SELECT EXISTS(SELECT 1 FROM {ENTRIES_TABLE} WHERE user_id = ? AND id = ?)",
    ))
    .bind(user_id)
    .bind(id)
    .fetch_one(&mut *conn)
    .await?;

    Ok(result == 1)
}

pub async fn delete_tag_by_tag_id<'c, C>(conn: C, user_id: Id, id: Id, tag_id: Id) -> Result<bool>
where
    C: Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    let result = sqlx::query(&format!(
        r"DELETE FROM {ENTRIES_TAG_TABLE} WHERE tag_id = ? AND entry_id in (SELECT id FROM {ENTRIES_TABLE} WHERE id = ? AND user_id = ?)"
    ))
    .bind(tag_id)
    .bind(id)
    .bind(user_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn count<'c, C>(conn: C, params: &FindParams) -> Result<i64>
where
    C: Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    // TODO rewrite this funny stupid count
    let mut q_builder = QueryBuilder::new(format!(
        r"SELECT COUNT(DISTINCT e.id) FROM {ENTRIES_TABLE} as e LEFT JOIN {ENTRIES_TAG_TABLE} et on et.entry_id = e.id LEFT JOIN {TAGS_TABLE} t on t.id = et.tag_id",
    ));

    if params.search.is_some() {
        q_builder.push(" JOIN entries_fts ON entries_fts.rowid = e.id");
    }

    q_builder.push(" WHERE e.user_id = ");
    q_builder.push_bind(params.user_id);

    if let Some(ref search) = params.search {
        q_builder.push(" AND entries_fts MATCH ");
        q_builder.push_bind(search.clone());
    }

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
    if params.detail == Some(Detail::Metadata) {
        return NotSupportedYetSnafu {
            msg: "Detail metadata mode is not supported yet",
        }
        .fail();
    }

    // TODO implement domain_name filtering
    if params.domain_name.is_some() {
        return NotSupportedYetSnafu {
            msg: "Domain filtering is not supported yet",
        }
        .fail();
    }

    // TODO implement tags filtering
    if params.tags.is_some() {
        return NotSupportedYetSnafu {
            msg: "Tags filtering is not supported yet",
        }
        .fail();
    }

    Ok(q_builder.build().fetch_one(&mut *conn).await?.get(0))
}

pub async fn create<'c, C>(
    conn: C,
    entry: CreateEntry,
    tags: &[crate::repository::tags::CreateTag],
) -> Result<(EntryRow, Vec<crate::repository::tags::TagRow>)>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut tx = conn.begin().await?;

    let id: i64 = sqlx::query_scalar(
        r"
        INSERT INTO entries (
            user_id, url, hashed_url, given_url, hashed_given_url, title, content, content_text, is_archived, archived_at,
            is_starred, starred_at, created_at, updated_at, mimetype,
            language, reading_time, domain_name, preview_picture,
            origin_url, published_at, published_by, is_public, uid
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        RETURNING id
        ",
    )
    .bind(entry.user_id)
    .bind(entry.url)
    .bind(entry.hashed_url)
    .bind(entry.given_url)
    .bind(entry.hashed_given_url)
    .bind(entry.title)
    .bind(entry.content)
    .bind(entry.content_text)
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
    .fetch_one(&mut *tx)
    .await?;

    if !tags.is_empty() {
        crate::repository::tags::create_and_link(&mut *tx, id, tags).await?;
    }

    let entry = sqlx::query_as::<_, EntryRow>("SELECT * FROM entries WHERE id = ?")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;

    let tags = sqlx::query_as::<_, crate::repository::tags::TagRow>(&format!(
        r"
        SELECT t.* FROM {ENTRIES_TAG_TABLE} as et
        LEFT JOIN {TAGS_TABLE} t on t.id = et.tag_id
        WHERE et.entry_id = ?
        "
    ))
    .bind(entry.id)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok((entry, tags))
}

pub async fn find_by_id<'c, C>(conn: C, user_id: Id, id: Id) -> Result<Option<FullEntry>>
where
    C: Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    let mut conn = conn.begin().await?;

    let entry = sqlx::query_as::<_, EntryRow>(&format!(
        "SELECT * FROM {ENTRIES_TABLE} WHERE user_id = ? AND id = ?"
    ))
    .bind(user_id)
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;

    let Some(entry) = entry else {
        return Ok(None);
    };

    let tags = sqlx::query_as::<_, crate::repository::tags::TagRow>(&format!(
        r"
        SELECT t.* FROM {ENTRIES_TAG_TABLE} as et
        LEFT JOIN {TAGS_TABLE} t on t.id = et.tag_id
        WHERE et.entry_id = ?
        "
    ))
    .bind(id)
    .fetch_all(&mut *conn)
    .await?;

    conn.commit().await?;

    Ok(Some((entry, tags)))
}

pub async fn update_by_id<'c, C>(conn: C, user_id: Id, id: Id, update: UpdateEntry) -> Result<bool>
where
    C: Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    let mut query_builder = QueryBuilder::new(format!("UPDATE {ENTRIES_TABLE} SET "));

    let mut separated = query_builder.separated(", ");

    if let Some(title) = update.title {
        separated.push("title = ");
        push_bind_or_default(&mut separated, title);
    }

    if let Some(content) = update.content {
        separated.push("content = ");
        push_bind_or_default(&mut separated, content);
    }

    if let Some(content_text) = update.content_text {
        separated.push("content_text = ");
        push_bind_or_default(&mut separated, content_text);
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

    let result = query_builder.build().execute(&mut *conn).await?;

    Ok(result.rows_affected() > 0)
}

pub async fn delete_by_id<'c, C>(conn: C, user_id: Id, id: Id) -> Result<bool>
where
    C: Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;

    let result = sqlx::query("DELETE FROM entries WHERE user_id = ? AND id = ?")
        .bind(user_id)
        .bind(id)
        .execute(&mut *conn)
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

#[derive(Debug, PartialEq)]
pub struct EntryRow {
    pub id: Id,
    pub user_id: Id,
    pub url: String,
    pub hashed_url: Option<String>,
    pub given_url: Option<String>,
    pub hashed_given_url: Option<String>,
    pub title: String,
    pub content: String,
    pub content_text: String,
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
            content_text: row.try_get("content_text")?,
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
    pub content_text: String,
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

// None - don't update anything
// Some(None) - update to default value
// Some(Some(v)) - update to v
type UpdateField<T> = Option<Option<T>>;

#[derive(Debug)]
pub struct UpdateEntry {
    pub title: UpdateField<String>,
    pub content: UpdateField<String>,
    pub content_text: UpdateField<String>,
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

impl Default for UpdateEntry {
    fn default() -> Self {
        Self {
            title: None,
            content: None,
            content_text: None,
            is_archived: None,
            archived_at: None,
            is_starred: None,
            starred_at: None,
            updated_at: Utc::now().timestamp(),
            language: None,
            reading_time: None,
            preview_picture: None,
            origin_url: None,
            published_at: None,
            published_by: None,
            is_public: None,
            uid: None,
        }
    }
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
pub struct FindParams {
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
    pub search: Option<String>,
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
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_exists_by_id(pool: SqlitePool) {
        let exists = exists_by_id(&pool, 1, 1).await.unwrap();
        assert!(exists, "Entry 1 should exist");

        let not_exists = exists_by_id(&pool, 1, 999).await.unwrap();
        assert!(!not_exists, "Entry 999 should not exist");
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_delete_tag_by_tag_id(pool: SqlitePool) {
        let tags_before = crate::repository::tags::find_by_entry_id(&pool, 1, 2)
            .await
            .unwrap();
        assert_eq!(tags_before.len(), 2, "Entry 2 should have 2 tags initially");

        let deleted = delete_tag_by_tag_id(&pool, 1, 2, 1).await.unwrap();
        assert!(
            deleted,
            "Should successfully delete existing tag association"
        );

        let tags_after = crate::repository::tags::find_by_entry_id(&pool, 1, 2)
            .await
            .unwrap();
        assert_eq!(
            tags_after.len(),
            1,
            "Entry 2 should have 1 tag after deletion"
        );
        assert_eq!(tags_after[0].id, 2, "Only label2 should remain");

        let not_deleted = delete_tag_by_tag_id(&pool, 1, 2, 1).await.unwrap();
        assert!(
            !not_deleted,
            "Should return false for non-existent association"
        );

        let not_deleted = delete_tag_by_tag_id(&pool, 1, 999, 1).await.unwrap();
        assert!(!not_deleted, "Should return false for non-existent entry");
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_entry_delete_by_id(pool: SqlitePool) {
        let mut conn = pool.acquire().await.unwrap();

        let entry_before = find_by_id(&mut *conn, 1, 1).await.unwrap();
        assert!(entry_before.is_some(), "Entry 1 should exist");

        let deleted = delete_by_id(&mut *conn, 1, 1).await.unwrap();
        assert!(deleted, "Should return true when entry is deleted");

        let entry_after = find_by_id(&mut *conn, 1, 1).await.unwrap();
        assert!(
            entry_after.is_none(),
            "Entry 1 should not exist after deletion"
        );

        let not_deleted = delete_by_id(&mut *conn, 1, 1).await.unwrap();
        assert!(!not_deleted, "Should return false when entry doesn't exist");

        let not_deleted = delete_by_id(&mut *conn, 1, 999).await.unwrap();
        assert!(!not_deleted, "Should return false for non-existent entry");

        let entry_2 = find_by_id(&mut *conn, 1, 2).await.unwrap();
        assert!(entry_2.is_some(), "Entry 2 should still exist");
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_update_entry_with_invalid_updated_at(pool: SqlitePool) {
        let update = UpdateEntry {
            title: None,
            content: None,
            content_text: None,
            is_archived: Some(Some(true)),
            archived_at: Some(Some(1_701_787_200)),
            is_starred: None,
            starred_at: None,
            updated_at: 0, // Invalid: 0 < 1_701_428_400
            language: None,
            reading_time: None,
            preview_picture: None,
            origin_url: None,
            published_at: None,
            published_by: None,
            is_public: None,
            uid: None,
        };

        let result = update_by_id(&pool, 1, 1, update).await;

        assert!(
            result.is_err(),
            "Update should fail when updated_at < created_at"
        );

        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("CHECK") || err_msg.contains("constraint"),
            "Error should mention constraint violation, got: {err_msg}"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_find_by_id(pool: SqlitePool) {
        let mut conn = pool.acquire().await.unwrap();

        let (entry, tags) = find_by_id(&mut *conn, 1, 1).await.unwrap().unwrap();
        assert_eq!(
            entry,
            EntryRow {
                id: 1,
                user_id: 1,
                url: "https://a.com/1".to_owned(),
                hashed_url: Some("hash1".to_owned()),
                given_url: Some("https://a.com/g1".to_owned()),
                hashed_given_url: Some("ghash1".to_owned()),
                title: "title1".to_owned(),
                content: "<span>content1</span>".to_owned(),
                content_text: "content1".to_owned(),
                is_archived: false,
                archived_at: None,
                is_starred: false,
                starred_at: None,
                created_at: 1_701_428_400,
                updated_at: 1_702_220_700,
                mimetype: Some("text/html".to_owned()),
                language: Some("en".to_owned()),
                reading_time: 8,
                domain_name: "a.com".to_owned(),
                preview_picture: Some("https://a.com/pic1.jpg".to_owned()),
                origin_url: Some("https://a.com/o1".to_owned()),
                published_at: Some(1_701_424_800),
                published_by: Some("author1".to_owned()),
                is_public: Some(false),
                uid: None,
            }
        );
        assert!(tags.is_empty(), "Entry 1 should have no tags");

        let (entry, tags) = find_by_id(&mut *conn, 1, 2).await.unwrap().unwrap();
        assert_eq!(entry.id, 2);
        assert_eq!(
            tags,
            vec![
                crate::repository::tags::TagRow {
                    id: 1,
                    user_id: 1,
                    label: "label1".to_owned(),
                    slug: "slug1".to_owned(),
                },
                crate::repository::tags::TagRow {
                    id: 2,
                    user_id: 1,
                    label: "label2".to_owned(),
                    slug: "slug2".to_owned(),
                },
            ]
        );

        let result = find_by_id(&mut *conn, 1, 999).await.unwrap();
        assert!(result.is_none(), "Entry 999 should not exist");

        let result = find_by_id(&mut *conn, 999, 1).await.unwrap();
        assert!(result.is_none(), "Entry 1 should not be found for user 999");
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_update_by_id(pool: SqlitePool) {
        let mut conn = pool.acquire().await.unwrap();

        let updated = update_by_id(
            &mut *conn,
            1,
            1,
            UpdateEntry {
                title: Some(Some("new title".to_owned())),
                content: Some(Some("new content".to_owned())),
                content_text: Some(Some("new text content".to_owned())),
                is_archived: Some(Some(true)),
                archived_at: Some(Some(1_702_000_000)),
                is_starred: Some(Some(true)),
                starred_at: Some(Some(1_702_000_001)),
                updated_at: 1_702_300_000,
                language: Some(Some("fr".to_owned())),
                reading_time: Some(Some(42)),
                preview_picture: Some(Some("https://new.pic/img.jpg".to_owned())),
                origin_url: Some(Some("https://new.origin".to_owned())),
                published_at: Some(Some(1_702_000_002)),
                published_by: Some(Some("new author".to_owned())),
                is_public: Some(Some(true)),
                uid: Some(Some("new-uid".to_owned())),
            },
        )
        .await
        .unwrap();
        assert!(updated);

        let (entry, _) = find_by_id(&mut *conn, 1, 1).await.unwrap().unwrap();
        assert_eq!(
            entry,
            EntryRow {
                id: 1,
                user_id: 1,
                url: "https://a.com/1".to_owned(),
                hashed_url: Some("hash1".to_owned()),
                given_url: Some("https://a.com/g1".to_owned()),
                hashed_given_url: Some("ghash1".to_owned()),
                title: "new title".to_owned(),
                content: "new content".to_owned(),
                content_text: "new text content".to_owned(),
                is_archived: true,
                archived_at: Some(1_702_000_000),
                is_starred: true,
                starred_at: Some(1_702_000_001),
                created_at: 1_701_428_400,
                updated_at: 1_702_300_000,
                mimetype: Some("text/html".to_owned()),
                language: Some("fr".to_owned()),
                reading_time: 42,
                domain_name: "a.com".to_owned(),
                preview_picture: Some("https://new.pic/img.jpg".to_owned()),
                origin_url: Some("https://new.origin".to_owned()),
                published_at: Some(1_702_000_002),
                published_by: Some("new author".to_owned()),
                is_public: Some(true),
                uid: Some("new-uid".to_owned()),
            }
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_update_by_id_partial(pool: SqlitePool) {
        let mut conn = pool.acquire().await.unwrap();

        let updated = update_by_id(
            &mut *conn,
            1,
            1,
            UpdateEntry {
                title: Some(Some("only title changed".to_owned())),
                updated_at: 1_702_300_000,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(updated);

        let (entry, _) = find_by_id(&mut *conn, 1, 1).await.unwrap().unwrap();
        assert_eq!(
            entry,
            EntryRow {
                id: 1,
                user_id: 1,
                url: "https://a.com/1".to_owned(),
                hashed_url: Some("hash1".to_owned()),
                given_url: Some("https://a.com/g1".to_owned()),
                hashed_given_url: Some("ghash1".to_owned()),
                title: "only title changed".to_owned(),
                content: "<span>content1</span>".to_owned(),
                content_text: "content1".to_owned(),
                is_archived: false,
                archived_at: None,
                is_starred: false,
                starred_at: None,
                created_at: 1_701_428_400,
                updated_at: 1_702_300_000,
                mimetype: Some("text/html".to_owned()),
                language: Some("en".to_owned()),
                reading_time: 8,
                domain_name: "a.com".to_owned(),
                preview_picture: Some("https://a.com/pic1.jpg".to_owned()),
                origin_url: Some("https://a.com/o1".to_owned()),
                published_at: Some(1_701_424_800),
                published_by: Some("author1".to_owned()),
                is_public: Some(false),
                uid: None,
            }
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_update_by_id_set_to_null(pool: SqlitePool) {
        let mut conn = pool.acquire().await.unwrap();

        let updated = update_by_id(
            &mut *conn,
            1,
            1,
            UpdateEntry {
                language: Some(None),
                preview_picture: Some(None),
                origin_url: Some(None),
                published_at: Some(None),
                published_by: Some(None),
                updated_at: 1_702_300_000,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(updated);

        let (entry, _) = find_by_id(&mut *conn, 1, 1).await.unwrap().unwrap();
        assert_eq!(entry.language, None);
        assert_eq!(entry.preview_picture, None);
        assert_eq!(entry.origin_url, None);
        assert_eq!(entry.published_at, None);
        assert_eq!(entry.published_by, None);
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_update_by_id_nonexistent(pool: SqlitePool) {
        let updated = update_by_id(
            &pool,
            1,
            999,
            UpdateEntry {
                title: Some(Some("nope".to_owned())),
                updated_at: 1_702_300_000,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(!updated, "Should return false for non-existent entry");

        let updated = update_by_id(
            &pool,
            999,
            1,
            UpdateEntry {
                title: Some(Some("nope".to_owned())),
                updated_at: 1_702_300_000,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(!updated, "Should return false for wrong user_id");
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql")
    )]
    async fn test_find_all_search_phrase(pool: SqlitePool) {
        create(
            &pool,
            CreateEntry {
                user_id: 1,
                url: "https://phrase.com/1".to_owned(),
                hashed_url: "phrase_hash1".to_owned(),
                given_url: "https://phrase.com/1".to_owned(),
                hashed_given_url: "phrase_ghash1".to_owned(),
                title: "Phrase Test".to_owned(),
                content: "<p>the quick brown fox jumps</p>".to_owned(),
                content_text: "the quick brown fox jumps".to_owned(),
                is_archived: false,
                archived_at: None,
                is_starred: false,
                starred_at: None,
                created_at: 1_700_000_000,
                updated_at: 1_700_000_001,
                mimetype: None,
                language: None,
                reading_time: 1,
                domain_name: "phrase.com".to_owned(),
                preview_picture: None,
                origin_url: None,
                published_at: None,
                published_by: None,
                is_public: None,
                uid: None,
            },
            &[],
        )
        .await
        .unwrap();

        let contiguous = find_all(
            &pool,
            &FindParams {
                user_id: 1,
                search: Some("\"quick brown\"".to_owned()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(contiguous.len(), 1, "contiguous phrase should match");

        let non_contiguous_phrase = find_all(
            &pool,
            &FindParams {
                user_id: 1,
                search: Some("\"quick fox\"".to_owned()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert!(
            non_contiguous_phrase.is_empty(),
            "non-contiguous phrase should not match"
        );

        let both_words_present = find_all(
            &pool,
            &FindParams {
                user_id: 1,
                search: Some("quick fox".to_owned()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(
            both_words_present.len(),
            1,
            "unquoted AND query should match when both words are present"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_find_all_search_exact(pool: SqlitePool) {
        let results = find_all(
            &pool,
            &FindParams {
                user_id: 1,
                search: Some("content1".to_owned()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id, 1);
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_find_all_search_common_substring(pool: SqlitePool) {
        let results = find_all(
            &pool,
            &FindParams {
                user_id: 1,
                search: Some("content".to_owned()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 6);
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_find_all_search_no_match(pool: SqlitePool) {
        let results = find_all(
            &pool,
            &FindParams {
                user_id: 1,
                search: Some("xyznotfound".to_owned()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert!(results.is_empty());
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_find_all_search_combined_with_filter(pool: SqlitePool) {
        let results = find_all(
            &pool,
            &FindParams {
                user_id: 1,
                search: Some("content".to_owned()),
                archive: Some(true),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 3);
        let ids: Vec<Id> = results.iter().map(|(e, _)| e.id).collect();
        assert!(ids.contains(&2));
        assert!(ids.contains(&4));
        assert!(ids.contains(&6));
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_count_with_search(pool: SqlitePool) {
        let count_all = count(
            &pool,
            &FindParams {
                user_id: 1,
                search: Some("content".to_owned()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(count_all, 6);

        let count_specific = count(
            &pool,
            &FindParams {
                user_id: 1,
                search: Some("content3".to_owned()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(count_specific, 1);

        let count_none = count(
            &pool,
            &FindParams {
                user_id: 1,
                search: Some("xyznotfound".to_owned()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        assert_eq!(count_none, 0);
    }
}
