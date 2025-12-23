use std::{fmt::Display, sync::Arc};

use crate::models::Range;
use async_trait::async_trait;
use indexmap::IndexMap;
use sqlx::{
    Error as SqlxError, QueryBuilder, Row, SqlitePool, prelude::*, sqlite::SqliteRow,
};

const ENTRIES_TABLE: &str = "entries";
const TAGS_TABLE: &str = "tags";
const ENTRIES_TAG_TABLE: &str = "entry_tags";
const ANNOTATIONS_TABLE: &str = "annotations";
const ANNOTATION_RANGES_TABLE: &str = "annotation_ranges";

pub struct TagRow {
    pub id: i32,
    pub label: String,
    pub slug: String,
}

impl TagRow {
    pub fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, SqlxError> {
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

#[async_trait]
pub trait TagRepository: Send + Sync {}

#[async_trait]
impl TagRepository for SqliteTagRepository {}

impl<'r> FromRow<'r, SqliteRow> for EntryRow {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, SqlxError> {
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
    pub id: i32,
    pub annotator_schema_version: String,
    pub text: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub quote: String,
}

impl AnnotationRow {
    pub fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, SqlxError> {
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
pub struct EntryRow {
    pub id: i64,
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
    pub created_at: i64,
    pub updated_at: i64,
    pub mimetype: Option<String>,
    pub language: Option<String>,
    pub reading_time: i64,
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
pub struct AllEntriesParams {
    pub archive: Option<bool>,
    pub starred: Option<bool>,
    pub sort: Option<SortColumn>,
    pub order: Option<SortOrder>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub since: Option<i64>,
    pub public: Option<bool>,
    pub detail: Option<Detail>,
    pub domain_name: Option<String>,
}

#[async_trait]
pub trait EntryRepository: Send + Sync {
    async fn find_all(
        &self,
        params: &AllEntriesParams,
    ) -> Result<Vec<(EntryRow, Vec<TagRow>)>, SqlxError>;

    async fn count(&self, params: &AllEntriesParams) -> Result<i64, SqlxError>;
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
    async fn find_all(
        &self,
        params: &AllEntriesParams,
    ) -> Result<Vec<(EntryRow, Vec<TagRow>)>, SqlxError> {
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
            return Err(SqlxError::Decode(
                "Detail metadata mode is not supported yet".into(),
            ));
        }

        // TODO implement domain_name filtering
        if let Some(_) = params.domain_name {
            return Err(SqlxError::Decode(
                "Domain filtering is not supported yet".into(),
            ));
        }

        // TODO implement tags filtering
        if let Some(_) = params.tags {
            return Err(SqlxError::Decode(
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

    async fn count(&self, params: &AllEntriesParams) -> Result<i64, SqlxError> {
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
            return Err(SqlxError::Decode(
                "Detail metadata mode is not supported yet".into(),
            ));
        }

        // TODO implement domain_name filtering
        if let Some(_) = params.domain_name {
            return Err(SqlxError::Decode(
                "Domain filtering is not supported yet".into(),
            ));
        }

        // TODO implement tags filtering
        if let Some(_) = params.tags {
            return Err(SqlxError::Decode(
                "Tags filtering is not supported yet".into(),
            ));
        }

        Ok(q_builder
            .build()
            .fetch_one(self.pool.as_ref())
            .await?
            .get(0))
    }
}
