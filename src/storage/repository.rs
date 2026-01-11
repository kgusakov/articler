use std::{fmt::Display, sync::Arc};

use crate::models::Range;
use actix_web::cookie::time::Time;
use async_trait::async_trait;
use env_logger::builder;
use indexmap::IndexMap;
use sqlx::{
    Database, Error as SqlxError, QueryBuilder, Row, SqlitePool, prelude::*, query,
    query_builder::Separated, query_scalar, sqlite::SqliteRow,
};
use thiserror::Error;

const ENTRIES_TABLE: &str = "entries";
const TAGS_TABLE: &str = "tags";
const ENTRIES_TAG_TABLE: &str = "entry_tags";
const ANNOTATIONS_TABLE: &str = "annotations";
const ANNOTATION_RANGES_TABLE: &str = "annotation_ranges";
const SQLITE_LIMIT_VARIABLE_NUMBER: usize = 999;

type Result<T> = std::result::Result<T, DbError>;
type FullEntry = (EntryRow, Vec<TagRow>);
type Id = i64;
type Timestamp = i64;
type ReadingTIme = i32;

#[derive(Error, Debug)]
pub enum DbError {
    // TODO produce ugly wrapped SqliteError(Database(SqliteError { code: 1, message: "no such column: et.tag_id" }))
    #[error(transparent)]
    SqliteRepositoryError(#[from] SqlxError),
    #[error("Repository error: {0}")]
    RepositoryError(String),
}

pub struct TagRow {
    pub id: Id,
    pub label: String,
    pub slug: String,
}

impl<'r> FromRow<'r, SqliteRow> for TagRow {
    fn from_row(row: &'r SqliteRow) -> std::result::Result<TagRow, SqlxError> {
        Ok(TagRow {
            id: row.try_get("id")?,
            label: row.try_get("label")?,
            slug: row.try_get("slug")?,
        })
    }
}

#[derive(Debug)]
pub struct CreateTag {
    pub label: String,
    pub slug: String,
}

#[derive(Debug)]
pub struct UpdateTag {
    pub label: Option<String>,
    pub slug: Option<String>,
}

#[derive(Clone)]
pub struct SqliteTagRepository {
    pool: Arc<SqlitePool>,
}

impl SqliteTagRepository {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self { pool }
    }
}

// TODO implement transactions per web request

#[async_trait]
pub trait TagRepository: Send + Sync {
    async fn create_and_link_tags(
        &self,
        entry_id: Id,
        tags: &Vec<CreateTag>,
    ) -> Result<Vec<TagRow>>;

    async fn update_tags_by_entry_id(
        &self,
        entry_id: Id,
        tags: Vec<CreateTag>,
    ) -> Result<Vec<TagRow>>;

    async fn find_by_entry_id(&self, entry_id: Id) -> Result<Vec<TagRow>>;

    async fn get_all(&self) -> Result<Vec<TagRow>>;

    async fn delete_by_label(&self, label: &str) -> Result<Option<TagRow>>;
}

#[async_trait]
impl TagRepository for SqliteTagRepository {
    /* Return Vec of tags, which was linked to entry_id. Vec consists of ALL tags, even tags, which was already linked before and included in tags argument. */
    async fn create_and_link_tags(
        &self,
        entry_id: Id,
        tags: &Vec<CreateTag>,
    ) -> Result<Vec<TagRow>> {
        if tags.is_empty() {
            return Ok(vec![]);
        }

        if tags.len() > SQLITE_LIMIT_VARIABLE_NUMBER / 2 {
            return Err(DbError::RepositoryError(
                format!(
                    "Too many tags: {} exceeds limit of {}",
                    tags.len(),
                    SQLITE_LIMIT_VARIABLE_NUMBER / 2
                )
                .into(),
            ));
        }

        let mut tag_builder = QueryBuilder::new("INSERT INTO tags (label, slug) ");
        tag_builder.push_values(tags.iter(), |mut b, tag| {
            // TODO can we remove cloning?
            b.push_bind(tag.label.clone()).push_bind(tag.slug.clone());
        });
        tag_builder.push(" ON CONFLICT DO NOTHING");
        tag_builder.build().execute(self.pool.as_ref()).await?;

        let mut insert_query =
            QueryBuilder::new(format!(r#"INSERT INTO {} SELECT "#, ENTRIES_TAG_TABLE));
        insert_query.push(entry_id);
        insert_query.push(format!(
            " as entry_id, id as tag_id FROM {} WHERE label IN (",
            TAGS_TABLE
        ));
        let mut separated = insert_query.separated(", ");
        for tag in tags {
            // TODO remove clone()?
            separated.push_bind(tag.label.clone());
        }
        separated.push_unseparated(") ON CONFLICT DO NOTHING");

        insert_query.build().execute(self.pool.as_ref()).await?;

        let mut get_tags =
            QueryBuilder::new(format!("SELECT * from {} WHERE label IN (", TAGS_TABLE));

        let mut tags_separated = get_tags.separated(", ");
        for tag in tags {
            // TODO remove clone()?
            tags_separated.push_bind(tag.label.clone());
        }
        tags_separated.push_unseparated(")");

        Ok(get_tags
            .build_query_as::<TagRow>()
            .fetch_all(self.pool.as_ref())
            .await?)
    }

    async fn update_tags_by_entry_id(
        &self,
        entry_id: Id,
        tags: Vec<CreateTag>,
    ) -> Result<Vec<TagRow>> {
        let result_tags = self.create_and_link_tags(entry_id, &tags).await?;

        let mut builder = QueryBuilder::new(format!(
            "DELETE FROM {} WHERE entry_id = ",
            ENTRIES_TAG_TABLE
        ));

        builder.push_bind(entry_id);

        builder.push(format!(
            r#"
             AND tag_id NOT IN (
                SELECT id FROM {} t WHERE t.label IN (
        "#,
            TAGS_TABLE
        ));

        let mut separated = builder.separated(", ");
        for t in tags.into_iter() {
            separated.push_bind(t.label);
        }

        separated.push_unseparated("))");

        builder.build().execute(self.pool.as_ref()).await?;

        Ok(result_tags)
    }

    async fn find_by_entry_id(&self, entry_id: Id) -> Result<Vec<TagRow>> {
        // TODO why manual ? + Ok() here needed for type inference?
        Ok(sqlx::query_as::<_, TagRow>(&format!(
            r#"
            SELECT t.* FROM {} t
            INNER JOIN {} et ON et.entry_id = ? AND et.tag_id = t.id
        "#,
            TAGS_TABLE, ENTRIES_TAG_TABLE
        ))
        .bind(entry_id)
        .fetch_all(self.pool.as_ref())
        .await?)
    }

    async fn get_all(&self) -> Result<Vec<TagRow>> {
        Ok(
            sqlx::query_as::<_, TagRow>(&format!("SELECT * FROM {} t", TAGS_TABLE))
                .fetch_all(self.pool.as_ref())
                .await?,
        )
    }

    async fn delete_by_label(&self, label: &str) -> Result<Option<TagRow>> {
        Ok(sqlx::query_as::<_, TagRow>(&format!(
            "DELETE FROM {} WHERE label = ? RETURNING *",
            TAGS_TABLE
        ))
        .bind(label)
        .fetch_optional(self.pool.as_ref())
        .await?)
    }
}

impl<'r> FromRow<'r, SqliteRow> for EntryRow {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> std::result::Result<EntryRow, SqlxError> {
        Ok(EntryRow {
            id: row.try_get("id")?,
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

pub struct AnnotationRow {
    pub id: Id,
    pub annotator_schema_version: String,
    pub text: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub quote: String,
}

impl AnnotationRow {
    pub fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self> {
        Ok(AnnotationRow {
            id: row.try_get("id")?,
            annotator_schema_version: row.try_get("annotator_schema_version")?,
            text: row.try_get("text")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            quote: row.try_get("quote")?,
        })
    }
}

impl Range {
    pub fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self> {
        Ok(Range {
            id: row.try_get("id")?,
            start: row.try_get("start")?,
            end: row.try_get("end")?,
            start_offset: row.try_get("start_offset")?,
            end_offset: row.try_get("end_offset")?,
        })
    }
}

#[derive(Debug)]
pub struct CreateEntry {
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
    pub reading_time: UpdateField<ReadingTIme>,
    pub preview_picture: UpdateField<String>,
    pub origin_url: UpdateField<String>,
    pub published_at: UpdateField<Timestamp>,
    pub published_by: UpdateField<String>,
    pub is_public: UpdateField<bool>,
    pub uid: UpdateField<String>,
}

#[derive(Debug)]
pub struct EntryRow {
    pub id: Id,
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
    pub reading_time: ReadingTIme,
    pub domain_name: String,
    pub preview_picture: Option<String>,
    pub origin_url: Option<String>,
    pub published_at: Option<Timestamp>,
    pub published_by: Option<String>,
    pub is_public: Option<bool>,
    pub uid: Option<String>,
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

#[derive(PartialEq)]
pub enum Detail {
    Metadata,
    Full,
}

#[derive(Default)]
pub struct EntriesCriteria {
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

#[async_trait]
pub trait EntryRepository: Send + Sync {
    async fn find_all(&self, params: &EntriesCriteria) -> Result<Vec<(EntryRow, Vec<TagRow>)>>;

    async fn count(&self, params: &EntriesCriteria) -> Result<i64>;

    async fn create(
        &self,
        params: CreateEntry,
        tags: &Vec<CreateTag>,
    ) -> Result<(EntryRow, Vec<TagRow>)>;

    async fn find_by_id(&self, id: Id) -> Result<Option<FullEntry>>;

    async fn exists_by_id(&self, id: Id) -> Result<bool>;

    async fn update_by_id(&self, id: Id, update: UpdateEntry) -> Result<bool>;

    async fn delete_by_id(&self, id: Id) -> Result<bool>;

    async fn delete_tag_by_tag_id(&self, id: Id, tag_id: Id) -> Result<bool>;
}

#[derive(Clone)]
pub struct SqliteEntryRepository {
    pool: Arc<SqlitePool>,
    tag_repo: Arc<dyn TagRepository>,
}

impl<'a> SqliteEntryRepository {
    pub fn new(pool: Arc<SqlitePool>, tag_repo: Arc<dyn TagRepository>) -> Self {
        Self { pool, tag_repo }
    }
}

#[async_trait]
impl EntryRepository for SqliteEntryRepository {
    async fn find_all(&self, params: &EntriesCriteria) -> Result<Vec<(EntryRow, Vec<TagRow>)>> {
        let mut q_builder = QueryBuilder::new(format!(
            r#"SELECT e.*, t.id as tag_id, t.label as tag_label, t.slug as tag_slug FROM {} as e LEFT JOIN {} et on et.entry_id = e.id LEFT JOIN {} t on t.id = et.tag_id
            WHERE e.id in (
                SELECT id FROM {}
                WHERE 1=1"#,
            ENTRIES_TABLE, ENTRIES_TAG_TABLE, TAGS_TABLE, ENTRIES_TABLE
        ));

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
            q_builder.push_bind(column.to_string());

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
        if params.detail != Some(Detail::Full) {
            return Err(DbError::RepositoryError(
                "Detail metadata mode is not supported yet".into(),
            ));
        }

        // TODO implement domain_name filtering
        if let Some(_) = params.domain_name {
            return Err(DbError::RepositoryError(
                "Domain filtering is not supported yet".into(),
            ));
        }

        // TODO implement tags filtering
        if let Some(_) = params.tags {
            return Err(DbError::RepositoryError(
                "Tags filtering is not supported yet".into(),
            ));
        }

        let raw_rows = q_builder.build().fetch_all(self.pool.as_ref()).await?;

        let mut entrs = IndexMap::<i32, Vec<&SqliteRow>>::new();

        for r in &raw_rows {
            let id: i32 = r.try_get("id")?;
            entrs.entry(id).and_modify(|v| v.push(r)).or_insert(vec![r]);
        }

        let mut entrs_with_relations = vec![];

        for e in entrs {
            let mut tags = vec![];

            for r in &e.1 {
                tags.push(TagRow {
                    id: r.try_get("tag_id")?,
                    label: r.try_get("tag_label")?,
                    slug: r.try_get("tag_slug")?,
                });
            }

            entrs_with_relations.push((EntryRow::from_row(e.1[0])?, tags));
        }

        Ok(entrs_with_relations)
    }

    async fn exists_by_id(&self, id: Id) -> Result<bool> {
        let result: i32 = sqlx::query_scalar(&format!(
            "SELECT EXISTS(SELECT 1 FROM {} WHERE id = ?)",
            ENTRIES_TABLE
        ))
        .bind(id)
        .fetch_one(self.pool.as_ref())
        .await?;

        Ok(result == 1)
    }

    async fn delete_tag_by_tag_id(&self, id: Id, tag_id: Id) -> Result<bool> {
        let result = sqlx::query(&format!(
            "DELETE FROM {}  WHERE entry_id = ? AND tag_id = ?",
            ENTRIES_TAG_TABLE
        ))
        .bind(id)
        .bind(tag_id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn count(&self, params: &EntriesCriteria) -> Result<i64> {
        // TODO rewrite this funny stupid count
        let mut q_builder = QueryBuilder::new(format!(
            r#"SELECT COUNT(DISTINCT e.id) FROM {} as e LEFT JOIN {} et on et.entry_id = e.id LEFT JOIN {} t on t.id = et.tag_id"#,
            ENTRIES_TABLE, ENTRIES_TAG_TABLE, TAGS_TABLE,
        ));
        q_builder.push(" WHERE 1=1");

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
        if let Some(_) = params.domain_name {
            return Err(DbError::RepositoryError(
                "Domain filtering is not supported yet".into(),
            ));
        }

        // TODO implement tags filtering
        if let Some(_) = params.tags {
            return Err(DbError::RepositoryError(
                "Tags filtering is not supported yet".into(),
            ));
        }

        Ok(q_builder
            .build()
            .fetch_one(self.pool.as_ref())
            .await?
            .get(0))
    }

    async fn create(
        &self,
        entry: CreateEntry,
        tags: &Vec<CreateTag>,
    ) -> Result<(EntryRow, Vec<TagRow>)> {
        let now = chrono::Utc::now().timestamp();

        let id = sqlx::query_scalar!(
                r#"
                INSERT INTO entries (
                    url, hashed_url, given_url, hashed_given_url, title, content, is_archived, archived_at,
                    is_starred, starred_at, created_at, updated_at, mimetype,
                    language, reading_time, domain_name, preview_picture,
                    origin_url, published_at, published_by, is_public, uid
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                RETURNING id
                "#,
                entry.url,
                entry.hashed_url,
                entry.given_url,
                entry.hashed_given_url,
                entry.title,
                entry.content,
                entry.is_archived,
                entry.archived_at,
                entry.is_starred,
                entry.starred_at,
                now,
                now,
                entry.mimetype,
                entry.language,
                entry.reading_time,
                entry.domain_name,
                entry.preview_picture,
                entry.origin_url,
                entry.published_at,
                entry.published_by,
                entry.is_public,
                entry.uid,
            )
            .fetch_one(self.pool.as_ref())
            .await?;

        if !tags.is_empty() {
            self.tag_repo.create_and_link_tags(id, tags).await?;
        }

        let entry = sqlx::query_as::<_, EntryRow>("SELECT * FROM entries WHERE id = ?")
            .bind(id)
            .fetch_one(self.pool.as_ref())
            .await?;

        let tags = sqlx::query_as::<_, TagRow>(&format!(
            r#"
            SELECT t.* FROM {} as et
            LEFT JOIN {} t on t.id = et.tag_id
            WHERE et.entry_id = ?
            "#,
            ENTRIES_TAG_TABLE, TAGS_TABLE
        ))
        .bind(entry.id)
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok((entry, tags))
    }

    async fn find_by_id(&self, id: Id) -> Result<Option<FullEntry>> {
        let entry = sqlx::query_as::<_, EntryRow>("SELECT * FROM entries WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool.as_ref())
            .await?;

        let entry = match entry {
            Some(e) => e,
            None => return Ok(None),
        };

        let tags = sqlx::query_as::<_, TagRow>(&format!(
            r#"
            SELECT t.* FROM {} as et
            LEFT JOIN {} t on t.id = et.tag_id
            WHERE et.entry_id = ?
            "#,
            ENTRIES_TAG_TABLE, TAGS_TABLE
        ))
        .bind(id)
        .fetch_all(self.pool.as_ref())
        .await?;

        Ok(Some((entry, tags)))
    }

    async fn update_by_id(&self, id: Id, update: UpdateEntry) -> Result<bool> {
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

        let result = query_builder.build().execute(self.pool.as_ref()).await?;

        Ok(result.rows_affected() > 0)
    }

    async fn delete_by_id(&self, id: Id) -> Result<bool> {
        let result = sqlx::query("DELETE FROM entries WHERE id = ?")
            .bind(id)
            .execute(self.pool.as_ref())
            .await?;

        Ok(result.rows_affected() > 0)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../tests/fixtures/entries.sql")
    )]
    async fn test_exists_by_id(pool: SqlitePool) {
        let pool = Arc::new(pool);
        let tag_repo = Arc::new(SqliteTagRepository::new(pool.clone()));
        let entry_repo = SqliteEntryRepository::new(pool.clone(), tag_repo);

        let exists = entry_repo.exists_by_id(1).await.unwrap();
        assert!(exists, "Entry 1 should exist");

        let not_exists = entry_repo.exists_by_id(999).await.unwrap();
        assert!(!not_exists, "Entry 999 should not exist");
    }

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../tests/fixtures/entries.sql")
    )]
    async fn test_delete_tag_by_tag_id(pool: SqlitePool) {
        let pool = Arc::new(pool);
        let tag_repo = Arc::new(SqliteTagRepository::new(pool.clone()));
        let entry_repo = SqliteEntryRepository::new(pool.clone(), tag_repo.clone());

        // Entry 2 initially has 2 tags (label1/id=1, label2/id=2)
        let tags_before = tag_repo.find_by_entry_id(2).await.unwrap();
        assert_eq!(tags_before.len(), 2, "Entry 2 should have 2 tags initially");

        // Delete tag_id=1 from entry 2
        let deleted = entry_repo.delete_tag_by_tag_id(2, 1).await.unwrap();
        assert!(
            deleted,
            "Should successfully delete existing tag association"
        );

        // Verify only 1 tag remains
        let tags_after = tag_repo.find_by_entry_id(2).await.unwrap();
        assert_eq!(
            tags_after.len(),
            1,
            "Entry 2 should have 1 tag after deletion"
        );
        assert_eq!(tags_after[0].id, 2, "Only label2 should remain");

        // Try to delete same tag again - should return false
        let not_deleted = entry_repo.delete_tag_by_tag_id(2, 1).await.unwrap();
        assert!(
            !not_deleted,
            "Should return false for non-existent association"
        );

        // Try to delete tag from non-existent entry
        let not_deleted = entry_repo.delete_tag_by_tag_id(999, 1).await.unwrap();
        assert!(!not_deleted, "Should return false for non-existent entry");
    }

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../tests/fixtures/entries.sql")
    )]
    async fn test_delete_by_label(pool: SqlitePool) {
        let pool = Arc::new(pool);
        let tag_repo = Arc::new(SqliteTagRepository::new(pool.clone()));

        // Verify initial 6 tags from fixtures
        let initial_tags = tag_repo.get_all().await.unwrap();
        assert_eq!(initial_tags.len(), 6, "Should have 6 tags initially");

        // Delete "label1" by label
        let deleted_tag = tag_repo.delete_by_label("label1").await.unwrap();
        assert!(deleted_tag.is_some(), "Should return deleted tag");
        let deleted = deleted_tag.unwrap();
        assert_eq!(deleted.label, "label1", "Deleted tag should have label 'label1'");
        assert_eq!(deleted.slug, "slug1", "Deleted tag should have slug 'slug1'");

        // Verify only 5 tags remain
        let tags_after = tag_repo.get_all().await.unwrap();
        assert_eq!(tags_after.len(), 5, "Should have 5 tags after deletion");

        // Verify CASCADE behavior: entry 2 should lose label1 but keep label2
        let entry_tags = tag_repo.find_by_entry_id(2).await.unwrap();
        assert_eq!(entry_tags.len(), 1, "Entry 2 should have 1 tag after cascade");
        assert_eq!(entry_tags[0].label, "label2", "Entry 2 should only have label2");

        // Try deleting non-existent label
        let not_deleted = tag_repo.delete_by_label("nonexistent").await.unwrap();
        assert!(not_deleted.is_none(), "Should return None for non-existent label");

        // Verify count unchanged after failed deletion
        let final_tags = tag_repo.get_all().await.unwrap();
        assert_eq!(final_tags.len(), 5, "Should still have 5 tags after failed deletion");
    }
}
