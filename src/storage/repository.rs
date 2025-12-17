use std::{collections::HashMap, fmt::Display, sync::Arc};

use crate::{
    api::entries,
    models::{Annotation, Entry, Range, Tag},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use indexmap::{
    IndexMap,
    map::{OccupiedEntry, VacantEntry},
};
use sqlx::{
    Decode, Error as SqlxError, QueryBuilder, Row, Sqlite, SqlitePool,
    prelude::*,
    sqlite::{SqliteRow, SqliteTypeInfo, SqliteValueRef},
};
use url::Url;

const ENTRIES_TABLE: &str = "entries";
const TAGS_TABLE: &str = "tags";
const ENTRIES_TAG_TABLE: &str = "entry_tags";

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

impl<'r> FromRow<'r, SqliteRow> for EntryRow {
    fn from_row(row: &'r sqlx::sqlite::SqliteRow) -> Result<Self, SqlxError> {
        Ok(EntryRow {
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
            preview_picture: row
                .try_get::<Option<DbUrl>, _>("preview_picture")
                .map(|u| u.map(|_u| Into::<Url>::into(_u)))?,
            origin_url: row
                .try_get::<Option<DbUrl>, _>("origin_url")
                .map(|u| u.map(|_u| Into::<Url>::into(_u)))?,
            published_at: row.try_get("published_at")?,
            published_by: row.try_get("published_by")?,
            is_public: row.try_get("is_public")?,
            uid: row.try_get("uid")?,
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
pub struct EntryRow {
    pub id: i32,
    pub url: Url,
    pub hashed_url: Option<String>,
    pub given_url: Option<Url>,
    pub hashed_given_url: Option<String>,
    pub title: String,
    pub content: String,
    pub is_archived: bool,
    pub archived_at: Option<DateTime<Utc>>,
    pub is_starred: bool,
    pub starred_at: Option<DateTime<Utc>>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub mimetype: Option<String>,
    pub language: Option<String>,
    pub reading_time: i32,
    pub domain_name: String,
    pub preview_picture: Option<Url>,
    pub origin_url: Option<Url>,
    pub published_at: Option<DateTime<Utc>>,
    pub published_by: Option<String>,
    pub is_public: Option<bool>,
    pub uid: Option<String>,
}

pub struct EntryRowWithRelations {
    pub entry: EntryRow,
    pub tags: Vec<TagRow>,
    pub annotations: Vec<Annotation>,
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

enum SortColumn {
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

enum SortOrder {
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

enum Detail {
    Metadata,
    Full,
}

#[derive(Default)]
pub struct AllEntriesParams {
    pub archive: Option<bool>,
    pub starred: Option<bool>,
    pub sort: Option<SortColumn>,
    pub order: Option<SortOrder>,
    pub page: Option<i32>,
    pub tags: Option<Vec<String>>,
    pub since: Option<DateTime<Utc>>,
    pub public: Option<bool>,
    pub detail: Option<Detail>,
    pub domain_name: Option<String>,
}

#[async_trait]
pub trait EntryRepository: Send + Sync {
    async fn find_all(
        &self,
        params: AllEntriesParams,
    ) -> Result<Vec<EntryRowWithRelations>, SqlxError>;
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
        params: AllEntriesParams,
    ) -> Result<Vec<EntryRowWithRelations>, SqlxError> {
        let mut q_builder = QueryBuilder::new(format!(
            r#"SELECT e.*, t.id as tag_id, t.label as tag_label, t.slug as tag_slug FROM {} as e
            LEFT JOIN {} et on et.entry_id = e.id
            LEFT JOIN {} t on t.id = et.tag_id"#,
            ENTRIES_TABLE, ENTRIES_TAG_TABLE, TAGS_TABLE
        ));
        q_builder.push(" WHERE 1=1");

        let mut w_separated = q_builder.separated(",");

        if let Some(a) = params.archive {
            w_separated.push(" AND is_archived = ?");
            w_separated.push_bind(a);
        }

        if let Some(s) = params.starred {
            w_separated.push(" AND is_starred = ?");
            w_separated.push_bind(s);
        }

        if let Some(p) = params.public {
            w_separated.push(" AND is_public = ?");
            w_separated.push_bind(p);
        }

        if let Some(d) = params.since {
            w_separated.push(" AND update_at = ?");
            w_separated.push_bind(d.timestamp());
        }

        if let Some(column) = params.sort {
            q_builder.push(" ORDER BY ?");
            q_builder.push_bind(column.to_string());

            if let Some(order) = params.order {
                q_builder.push(" ?");
                q_builder.push_bind(order.to_string());
            }
        }

        if let Some(_) = params.page {
            todo!("Paging is not supported yet");
        }

        if let Some(_) = params.detail {
            todo!("Detail is not supported yet");
        }

        if let Some(_) = params.domain_name {
            todo!("Domain name is not supported yet");
        }

        if let Some(_) = params.tags {
            todo!("Tags is not supported yet");
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

            entrs_with_relations.push(EntryRowWithRelations {
                entry: EntryRow::from_row(e.1[0])?,
                tags: tags,
                annotations: vec![],
            });
        }

        Ok(entrs_with_relations)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sqlx::SqlitePool;
    use url::Url;

    use crate::storage::repository::{
        AllEntriesParams, EntryRepository, SqliteEntryRepository, SqliteTagRepository,
    };

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../tests/fixtures/entries.sql")
    )]
    async fn get_entries(pool: SqlitePool) {
        let a_pool = Arc::new(pool);
        let tag_repo = Arc::new(SqliteTagRepository::new(a_pool.clone()));

        let entry_repository = SqliteEntryRepository::new(a_pool.clone(), tag_repo.clone());

        let entries = entry_repository
            .find_all(AllEntriesParams {
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(1, entries.len());
        assert_eq!(
            Url::parse("https://example.com/article/rust-web-backend/url").unwrap(),
            entries[0].entry.url
        );
        assert_eq!(
            vec!["rust", "web-development", "backend", "tutorial"],
            entries[0]
                .tags
                .iter()
                .map(|v| v.slug.clone())
                .collect::<Vec<String>>()
        );
    }
}
