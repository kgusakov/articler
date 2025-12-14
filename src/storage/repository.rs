use std::{error::Error, sync::Arc};

use crate::models::{Annotation, Entry, Range, Tag};
use async_trait::async_trait;
use sqlx::{
    Database, Decode, Error as SqlxError, Row, Sqlite, SqlitePool,
    prelude::*,
    sqlite::{SqliteTypeInfo, SqliteValueRef},
};
use url::Url;

impl Tag {
    pub fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, SqlxError> {
        Ok(Tag {
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

#[async_trait]
pub trait TagRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<Tag>, SqlxError>;
    async fn find_by_slug(&self, slug: &str) -> Result<Option<Tag>, SqlxError>;
    async fn find_all(&self) -> Result<Vec<Tag>, SqlxError>;
    async fn find_by_entry_id(&self, entry_id: i32) -> Result<Vec<Tag>, SqlxError>;
    async fn create(&self, tag: CreateTag) -> Result<Tag, SqlxError>;
    async fn update(&self, id: i32, tag: UpdateTag) -> Result<Tag, SqlxError>;
    async fn delete(&self, id: i32) -> Result<bool, SqlxError>;
    async fn count(&self) -> Result<i64, SqlxError>;
}

#[async_trait]
impl TagRepository for SqliteTagRepository {
    async fn find_by_id(&self, id: i32) -> sqlx::Result<Option<Tag>, SqlxError> {
        let row = sqlx::query("SELECT id, label, slug FROM tags WHERE id = ?")
            .bind(id)
            .fetch_optional(self.pool.as_ref())
            .await?;

        match row {
            Some(r) => Ok(Some(Tag::from_row(&r)?)),
            None => Ok(None),
        }
    }

    async fn find_by_slug(&self, slug: &str) -> Result<Option<Tag>, SqlxError> {
        let row = sqlx::query("SELECT id, label, slug FROM tags WHERE slug = ?")
            .bind(slug)
            .fetch_optional(self.pool.as_ref())
            .await?;

        match row {
            Some(r) => Ok(Some(Tag::from_row(&r)?)),
            None => Ok(None),
        }
    }

    async fn find_all(&self) -> Result<Vec<Tag>, SqlxError> {
        let rows = sqlx::query("SELECT id, label, slug FROM tags ORDER BY label")
            .fetch_all(self.pool.as_ref())
            .await?;

        rows.iter().map(|row| Tag::from_row(row)).collect()
    }

    async fn find_by_entry_id(&self, entry_id: i32) -> Result<Vec<Tag>, SqlxError> {
        let rows = sqlx::query(
            "SELECT t.id, t.label, t.slug 
             FROM tags t
             INNER JOIN entry_tags et ON t.id = et.tag_id
             WHERE et.entry_id = ?
             ORDER BY t.label",
        )
        .bind(entry_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.iter().map(|row| Tag::from_row(row)).collect()
    }

    async fn create(&self, tag: CreateTag) -> Result<Tag, SqlxError> {
        let result = sqlx::query("INSERT INTO tags (label, slug) VALUES (?, ?)")
            .bind(&tag.label)
            .bind(&tag.slug)
            .execute(self.pool.as_ref())
            .await?;

        let id = result.last_insert_rowid() as i32;

        self.find_by_id(id).await?.ok_or(SqlxError::RowNotFound)
    }

    async fn update(&self, id: i32, tag: UpdateTag) -> Result<Tag, SqlxError> {
        let mut updates = Vec::new();

        if tag.label.is_some() {
            updates.push("label = ?");
        }
        if tag.slug.is_some() {
            updates.push("slug = ?");
        }

        if updates.is_empty() {
            // Nothing to update
            return self.find_by_id(id).await?.ok_or(SqlxError::RowNotFound);
        }

        let query_str = format!("UPDATE tags SET {} WHERE id = ?", updates.join(", "));

        let mut query = sqlx::query(&query_str);

        if let Some(ref label) = tag.label {
            query = query.bind(label);
        }
        if let Some(ref slug) = tag.slug {
            query = query.bind(slug);
        }

        query.bind(id).execute(self.pool.as_ref()).await?;

        self.find_by_id(id).await?.ok_or(SqlxError::RowNotFound)
    }

    async fn delete(&self, id: i32) -> Result<bool, SqlxError> {
        let result = sqlx::query("DELETE FROM tags WHERE id = ?")
            .bind(id)
            .execute(self.pool.as_ref())
            .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn count(&self) -> Result<i64, SqlxError> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM tags")
            .fetch_one(self.pool.as_ref())
            .await?;

        Ok(row.try_get("count")?)
    }
}

struct DbUrl(Url);

impl Type<Sqlite> for DbUrl {
    fn type_info() -> SqliteTypeInfo {
        <String as Type<Sqlite>>::type_info()
    }
}

impl<'r> Decode<'r, Sqlite> for DbUrl {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let text = <String as Decode<Sqlite>>::decode(value)?;
        Url::parse(&text).map_err(Into::into).map(DbUrl)
    }
}

impl Into<Url> for DbUrl {
    fn into(self) -> Url {
        self.0
    }
}

impl Entry {
    pub fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, SqlxError> {
        Ok(Entry {
            id: row.try_get("id")?,
            url: row.try_get::<DbUrl, _>("url")?.into(),
            hashed_url: row.try_get("hashed_url")?,
            given_url: row
                .try_get::<Option<DbUrl>, _>("given_url")
                .map(|u| u.map(|_u| Into::<Url>::into(_u)))?,
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
            origin_url: row
                .try_get::<Option<DbUrl>, _>("given_url")
                .map(|u| u.map(|_u| Into::<Url>::into(_u)))?,
            published_at: row.try_get("published_at")?,
            published_by: row.try_get("published_by")?,
            is_public: row.try_get("is_public")?,
            uid: row.try_get("uid")?,
            tags: vec![],
            annotations: None,
        })
    }
}

impl Annotation {
    pub fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, SqlxError> {
        Ok(Annotation {
            id: row.try_get("id")?,
            annotator_schema_version: row.try_get("annotator_schema_version")?,
            text: row.try_get("text")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
            quote: row.try_get("quote")?,
            ranges: vec![],
        })
    }
}

impl Range {
    pub fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, SqlxError> {
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
    pub hashed_url: Option<String>,
    pub given_url: Option<String>,
    pub hashed_given_url: Option<String>,
    pub title: String,
    pub content: String,
    pub is_archived: bool,
    pub archived_at: Option<i64>,
    pub is_starred: bool,
    pub starred_at: Option<i64>,
    pub mimetype: Option<String>,
    pub language: Option<String>,
    pub reading_time: i32,
    pub domain_name: String,
    pub preview_picture: Option<String>,
    pub origin_url: Option<String>,
    pub published_at: Option<i64>,
    pub published_by: Option<String>,
    pub is_public: Option<bool>,
    pub uid: Option<String>,
}

#[derive(Debug)]
pub struct UpdateEntry {
    pub url: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub is_archived: Option<bool>,
    pub archived_at: Option<i64>,
    pub is_starred: Option<bool>,
    pub starred_at: Option<i64>,
    pub mimetype: Option<String>,
    pub language: Option<String>,
    pub reading_time: Option<i32>,
    pub preview_picture: Option<String>,
    pub is_public: Option<bool>,
}

#[async_trait]
pub trait EntryRepository: Send + Sync {
    async fn find_by_id(&self, id: i32) -> Result<Option<Entry>, SqlxError>;
    async fn find_by_id_with_relations(&self, id: i32) -> Result<Option<Entry>, SqlxError>;
    async fn find_all(&self, limit: i32, offset: i32) -> Result<Vec<Entry>, SqlxError>;
    async fn find_all_with_relations(
        &self,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<Entry>, SqlxError>;
    async fn find_by_tag(&self, tag_id: i32) -> Result<Vec<Entry>, SqlxError>;
    async fn find_archived(&self, limit: i32, offset: i32) -> Result<Vec<Entry>, SqlxError>;
    async fn find_starred(&self, limit: i32, offset: i32) -> Result<Vec<Entry>, SqlxError>;
    async fn create(&self, entry: CreateEntry) -> Result<Entry, SqlxError>;
    async fn update(&self, id: i32, entry: UpdateEntry) -> Result<Entry, SqlxError>;
    async fn delete(&self, id: i32) -> Result<bool, SqlxError>;
    async fn count(&self) -> Result<i64, SqlxError>;
    async fn add_tag(&self, entry_id: i32, tag_id: i32) -> Result<(), SqlxError>;
    async fn remove_tag(&self, entry_id: i32, tag_id: i32) -> Result<(), SqlxError>;
    async fn archive(&self, id: i32) -> Result<(), SqlxError>;
    async fn unarchive(&self, id: i32) -> Result<(), SqlxError>;
    async fn star(&self, id: i32) -> Result<(), SqlxError>;
    async fn unstar(&self, id: i32) -> Result<(), SqlxError>;
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

    // Private helper methods for loading relationships
    async fn load_tags(&self, entry_id: i32) -> Result<Vec<Tag>, SqlxError> {
        self.tag_repo.find_by_entry_id(entry_id).await
    }

    async fn load_annotations(&self, entry_id: i32) -> Result<Vec<Annotation>, SqlxError> {
        let rows = sqlx::query(
            "SELECT id, annotator_schema_version, text, created_at, updated_at, quote
             FROM annotations
             WHERE entry_id = ?",
        )
        .bind(entry_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        let mut annotations = Vec::new();
        for row in rows.iter() {
            let mut annotation = Annotation::from_row(row)?;
            annotation.ranges = self.load_ranges(annotation.id).await?;
            annotations.push(annotation);
        }

        Ok(annotations)
    }

    async fn load_ranges(&self, annotation_id: i32) -> Result<Vec<Range>, SqlxError> {
        let rows = sqlx::query(
            "SELECT id, start, end, start_offset, end_offset
             FROM annotation_ranges
             WHERE annotation_id = ?",
        )
        .bind(annotation_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.iter().map(|row| Range::from_row(row)).collect()
    }

    fn get_current_timestamp() -> i64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    }
}

#[async_trait]
impl EntryRepository for SqliteEntryRepository {
    async fn find_by_id(&self, id: i32) -> Result<Option<Entry>, SqlxError> {
        let row = sqlx::query(
            "SELECT id, url, hashed_url, given_url, hashed_given_url, title, content,
                    is_archived, archived_at, is_starred, starred_at, created_at, updated_at,
                    mimetype, language, reading_time, domain_name, preview_picture, origin_url,
                    published_at, published_by, is_public, uid
             FROM entries
             WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool.as_ref())
        .await?;

        match row {
            Some(r) => Ok(Some(Entry::from_row(&r)?)),
            None => Ok(None),
        }
    }

    async fn find_by_id_with_relations(&self, id: i32) -> Result<Option<Entry>, SqlxError> {
        let row = sqlx::query(
            "SELECT id, url, hashed_url, given_url, hashed_given_url, title, content,
                    is_archived, archived_at, is_starred, starred_at, created_at, updated_at,
                    mimetype, language, reading_time, domain_name, preview_picture, origin_url,
                    published_at, published_by, is_public, uid
             FROM entries
             WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(self.pool.as_ref())
        .await?;

        if let Some(r) = row {
            let mut entry = Entry::from_row(&r)?;

            // Load tags
            entry.tags = self.load_tags(id).await?;

            // Load annotations
            let annotations = self.load_annotations(id).await?;
            entry.annotations = if annotations.is_empty() {
                None
            } else {
                Some(annotations)
            };

            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }

    async fn find_all(&self, limit: i32, offset: i32) -> Result<Vec<Entry>, SqlxError> {
        let rows = sqlx::query(
            "SELECT id, url, hashed_url, given_url, hashed_given_url, title, content,
                    is_archived, archived_at, is_starred, starred_at, created_at, updated_at,
                    mimetype, language, reading_time, domain_name, preview_picture, origin_url,
                    published_at, published_by, is_public, uid
             FROM entries
             ORDER BY created_at DESC
             LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.iter().map(|row| Entry::from_row(row)).collect()
    }

    async fn find_all_with_relations(
        &self,
        limit: i32,
        offset: i32,
    ) -> Result<Vec<Entry>, SqlxError> {
        let rows = sqlx::query(
            "SELECT id, url, hashed_url, given_url, hashed_given_url, title, content,
                    is_archived, archived_at, is_starred, starred_at, created_at, updated_at,
                    mimetype, language, reading_time, domain_name, preview_picture, origin_url,
                    published_at, published_by, is_public, uid
             FROM entries
             ORDER BY created_at DESC
             LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool.as_ref())
        .await?;

        let mut entries = Vec::new();
        for row in rows.iter() {
            let mut entry = Entry::from_row(row)?;
            entry.tags = self.load_tags(entry.id).await?;

            // TODO replace multiple selects by one join
            let annotations = self.load_annotations(entry.id).await?;
            entry.annotations = if annotations.is_empty() {
                None
            } else {
                Some(annotations)
            };

            entries.push(entry);
        }

        Ok(entries)
    }

    async fn find_by_tag(&self, tag_id: i32) -> Result<Vec<Entry>, SqlxError> {
        let rows = sqlx::query(
            "SELECT e.id, e.url, e.hashed_url, e.given_url, e.hashed_given_url, e.title, e.content,
                    e.is_archived, e.archived_at, e.is_starred, e.starred_at, e.created_at, e.updated_at,
                    e.mimetype, e.language, e.reading_time, e.domain_name, e.preview_picture, e.origin_url,
                    e.published_at, e.published_by, e.is_public, e.uid
             FROM entries e
             INNER JOIN entry_tags et ON e.id = et.entry_id
             WHERE et.tag_id = ?
             ORDER BY e.created_at DESC"
        )
        .bind(tag_id)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.iter().map(|row| Entry::from_row(row)).collect()
    }

    async fn find_archived(&self, limit: i32, offset: i32) -> Result<Vec<Entry>, SqlxError> {
        let rows = sqlx::query(
            "SELECT id, url, hashed_url, given_url, hashed_given_url, title, content,
                    is_archived, archived_at, is_starred, starred_at, created_at, updated_at,
                    mimetype, language, reading_time, domain_name, preview_picture, origin_url,
                    published_at, published_by, is_public, uid
             FROM entries
             WHERE is_archived = 1
             ORDER BY archived_at DESC
             LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.iter().map(|row| Entry::from_row(row)).collect()
    }

    async fn find_starred(&self, limit: i32, offset: i32) -> Result<Vec<Entry>, SqlxError> {
        let rows = sqlx::query(
            "SELECT id, url, hashed_url, given_url, hashed_given_url, title, content,
                    is_archived, archived_at, is_starred, starred_at, created_at, updated_at,
                    mimetype, language, reading_time, domain_name, preview_picture, origin_url,
                    published_at, published_by, is_public, uid
             FROM entries
             WHERE is_starred = 1
             ORDER BY starred_at DESC
             LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool.as_ref())
        .await?;

        rows.iter().map(|row| Entry::from_row(row)).collect()
    }

    async fn create(&self, entry: CreateEntry) -> Result<Entry, SqlxError> {
        let now = Self::get_current_timestamp();

        let result = sqlx::query(
            "INSERT INTO entries (
                url, hashed_url, given_url, hashed_given_url, title, content,
                is_archived, archived_at, is_starred, starred_at, created_at, updated_at,
                mimetype, language, reading_time, domain_name, preview_picture, origin_url,
                published_at, published_by, is_public, uid
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&entry.url)
        .bind(&entry.hashed_url)
        .bind(&entry.given_url)
        .bind(&entry.hashed_given_url)
        .bind(&entry.title)
        .bind(&entry.content)
        .bind(entry.is_archived)
        .bind(entry.archived_at)
        .bind(entry.is_starred)
        .bind(entry.starred_at)
        .bind(now)
        .bind(now)
        .bind(&entry.mimetype)
        .bind(&entry.language)
        .bind(entry.reading_time)
        .bind(&entry.domain_name)
        .bind(&entry.preview_picture)
        .bind(&entry.origin_url)
        .bind(entry.published_at)
        .bind(&entry.published_by)
        .bind(entry.is_public)
        .bind(&entry.uid)
        .execute(self.pool.as_ref())
        .await?;

        let id = result.last_insert_rowid() as i32;

        self.find_by_id(id).await?.ok_or(SqlxError::RowNotFound)
    }

    async fn update(&self, id: i32, entry: UpdateEntry) -> Result<Entry, SqlxError> {
        let now = Self::get_current_timestamp();

        let mut updates = vec!["updated_at = ?"];

        if entry.url.is_some() {
            updates.push("url = ?");
        }
        if entry.title.is_some() {
            updates.push("title = ?");
        }
        if entry.content.is_some() {
            updates.push("content = ?");
        }
        if entry.is_archived.is_some() {
            updates.push("is_archived = ?");
        }
        if entry.archived_at.is_some() {
            updates.push("archived_at = ?");
        }
        if entry.is_starred.is_some() {
            updates.push("is_starred = ?");
        }
        if entry.starred_at.is_some() {
            updates.push("starred_at = ?");
        }
        if entry.mimetype.is_some() {
            updates.push("mimetype = ?");
        }
        if entry.language.is_some() {
            updates.push("language = ?");
        }
        if entry.reading_time.is_some() {
            updates.push("reading_time = ?");
        }
        if entry.preview_picture.is_some() {
            updates.push("preview_picture = ?");
        }
        if entry.is_public.is_some() {
            updates.push("is_public = ?");
        }

        let query_str = format!("UPDATE entries SET {} WHERE id = ?", updates.join(", "));

        let mut query = sqlx::query(&query_str).bind(now);

        if let Some(ref url) = entry.url {
            query = query.bind(url);
        }
        if let Some(ref title) = entry.title {
            query = query.bind(title);
        }
        if let Some(ref content) = entry.content {
            query = query.bind(content);
        }
        if let Some(is_archived) = entry.is_archived {
            query = query.bind(is_archived);
        }
        if let Some(archived_at) = entry.archived_at {
            query = query.bind(archived_at);
        }
        if let Some(is_starred) = entry.is_starred {
            query = query.bind(is_starred);
        }
        if let Some(starred_at) = entry.starred_at {
            query = query.bind(starred_at);
        }
        if let Some(ref mimetype) = entry.mimetype {
            query = query.bind(mimetype);
        }
        if let Some(ref language) = entry.language {
            query = query.bind(language);
        }
        if let Some(reading_time) = entry.reading_time {
            query = query.bind(reading_time);
        }
        if let Some(ref preview_picture) = entry.preview_picture {
            query = query.bind(preview_picture);
        }
        if let Some(is_public) = entry.is_public {
            query = query.bind(is_public);
        }

        query.bind(id).execute(self.pool.as_ref()).await?;

        self.find_by_id(id).await?.ok_or(SqlxError::RowNotFound)
    }

    async fn delete(&self, id: i32) -> Result<bool, SqlxError> {
        let result = sqlx::query("DELETE FROM entries WHERE id = ?")
            .bind(id)
            .execute(self.pool.as_ref())
            .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn count(&self) -> Result<i64, SqlxError> {
        let row = sqlx::query("SELECT COUNT(*) as count FROM entries")
            .fetch_one(self.pool.as_ref())
            .await?;

        Ok(row.try_get("count")?)
    }

    async fn add_tag(&self, entry_id: i32, tag_id: i32) -> Result<(), SqlxError> {
        sqlx::query("INSERT OR IGNORE INTO entry_tags (entry_id, tag_id) VALUES (?, ?)")
            .bind(entry_id)
            .bind(tag_id)
            .execute(self.pool.as_ref())
            .await?;

        Ok(())
    }

    async fn remove_tag(&self, entry_id: i32, tag_id: i32) -> Result<(), SqlxError> {
        sqlx::query("DELETE FROM entry_tags WHERE entry_id = ? AND tag_id = ?")
            .bind(entry_id)
            .bind(tag_id)
            .execute(self.pool.as_ref())
            .await?;

        Ok(())
    }

    async fn archive(&self, id: i32) -> Result<(), SqlxError> {
        let now = Self::get_current_timestamp();

        sqlx::query(
            "UPDATE entries SET is_archived = 1, archived_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    async fn unarchive(&self, id: i32) -> Result<(), SqlxError> {
        let now = Self::get_current_timestamp();

        sqlx::query(
            "UPDATE entries SET is_archived = 0, archived_at = NULL, updated_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    async fn star(&self, id: i32) -> Result<(), SqlxError> {
        let now = Self::get_current_timestamp();

        sqlx::query(
            "UPDATE entries SET is_starred = 1, starred_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }

    async fn unstar(&self, id: i32) -> Result<(), SqlxError> {
        let now = Self::get_current_timestamp();

        sqlx::query(
            "UPDATE entries SET is_starred = 0, starred_at = NULL, updated_at = ? WHERE id = ?",
        )
        .bind(now)
        .bind(id)
        .execute(self.pool.as_ref())
        .await?;

        Ok(())
    }
}
