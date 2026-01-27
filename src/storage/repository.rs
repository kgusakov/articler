
use async_trait::async_trait;
use sqlx::{
    Error as SqlxError, QueryBuilder, Row, SqlitePool, prelude::*, sqlite::SqliteRow,
};
use thiserror::Error;

const ENTRIES_TABLE: &str = "entries";
const TAGS_TABLE: &str = "tags";
const ENTRIES_TAG_TABLE: &str = "entry_tags";
const SQLITE_LIMIT_VARIABLE_NUMBER: usize = 999;

type Result<T> = std::result::Result<T, DbError>;
type Id = i64;

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
    pub user_id: Id,
    pub label: String,
    pub slug: String,
}

impl<'r> FromRow<'r, SqliteRow> for TagRow {
    fn from_row(row: &'r SqliteRow) -> std::result::Result<TagRow, SqlxError> {
        Ok(TagRow {
            id: row.try_get("id")?,
            user_id: row.try_get("user_id")?,
            label: row.try_get("label")?,
            slug: row.try_get("slug")?,
        })
    }
}

#[derive(Debug)]
pub struct CreateTag {
    pub user_id: Id,
    pub label: String,
    pub slug: String,
}

#[derive(Clone)]
pub struct SqliteTagRepository {
    pool: SqlitePool,
}

impl SqliteTagRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

// TODO implement transactions per web request

#[async_trait]
pub trait TagRepository: Send + Sync {
    async fn create_and_link_tags(&self, entry_id: Id, tags: &[CreateTag]) -> Result<Vec<TagRow>>;

    async fn update_tags_by_entry_id(
        &self,
        user_id: Id,
        entry_id: Id,
        tags: &[CreateTag],
    ) -> Result<Vec<TagRow>>;

    async fn find_by_entry_id(&self, user_id: Id, entry_id: Id) -> Result<Vec<TagRow>>;

    async fn get_all(&self, user_id: Id) -> Result<Vec<TagRow>>;

    async fn delete_by_label(&self, user_id: Id, label: &str) -> Result<Option<TagRow>>;

    async fn delete_by_id(&self, user_id: Id, id: Id) -> Result<Option<TagRow>>;

    async fn delete_all_by_label(&self, user_id: Id, labels: &[String]) -> Result<Vec<TagRow>>;
}

pub struct SqliteUserRepository;

impl Default for SqliteUserRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl SqliteUserRepository {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl TagRepository for SqliteTagRepository {
    /* Return Vec of tags, which was linked to entry_id. Vec consists of ALL tags, even tags, which was already linked before and included in tags argument. */
    async fn create_and_link_tags(&self, entry_id: Id, tags: &[CreateTag]) -> Result<Vec<TagRow>> {
        if tags.is_empty() {
            return Ok(vec![]);
        }

        if tags.len() > SQLITE_LIMIT_VARIABLE_NUMBER / 2 {
            return Err(DbError::RepositoryError(format!(
                "Too many tags: {} exceeds limit of {}",
                tags.len(),
                SQLITE_LIMIT_VARIABLE_NUMBER / 2
            )));
        }

        let mut tag_builder = QueryBuilder::new("INSERT INTO tags (user_id, label, slug) ");
        tag_builder.push_values(tags.iter(), |mut b, tag| {
            b.push_bind(tag.user_id)
                .push_bind(&tag.label)
                .push_bind(&tag.slug);
        });
        tag_builder.push(" ON CONFLICT DO NOTHING");
        tag_builder.build().execute(&self.pool).await?;

        let mut insert_query =
            QueryBuilder::new(format!(r#"INSERT INTO {} SELECT "#, ENTRIES_TAG_TABLE));
        insert_query.push(entry_id);
        insert_query.push(format!(
            " as entry_id, id as tag_id FROM {} WHERE label IN (",
            TAGS_TABLE
        ));
        let mut separated = insert_query.separated(", ");
        for tag in tags {
            separated.push_bind(&tag.label);
        }
        separated.push_unseparated(") ON CONFLICT DO NOTHING");

        insert_query.build().execute(&self.pool).await?;

        let mut get_tags =
            QueryBuilder::new(format!("SELECT * from {} WHERE label IN (", TAGS_TABLE));

        let mut tags_separated = get_tags.separated(", ");
        for tag in tags {
            tags_separated.push_bind(&tag.label);
        }
        tags_separated.push_unseparated(")");

        Ok(get_tags
            .build_query_as::<TagRow>()
            .fetch_all(&self.pool)
            .await?)
    }

    async fn update_tags_by_entry_id(
        &self,
        user_id: Id,
        entry_id: Id,
        tags: &[CreateTag],
    ) -> Result<Vec<TagRow>> {
        let result_tags = self.create_and_link_tags(entry_id, tags).await?;

        let mut builder = QueryBuilder::new(format!(
            "DELETE FROM {ENTRIES_TAG_TABLE} WHERE entry_id IN (SELECT id FROM {ENTRIES_TABLE} WHERE entry_id =",
        ));

        builder.push_bind(entry_id);

        builder.push(" AND user_id = ");
        builder.push_bind(user_id);
        builder.push(") ");

        builder.push(format!(
            r#"
             AND tag_id NOT IN (
                SELECT id FROM {TAGS_TABLE} t WHERE t.label IN (
        "#,
        ));

        let mut separated = builder.separated(", ");
        for t in tags.iter() {
            separated.push_bind(&t.label);
        }

        separated.push_unseparated("))");

        builder.build().execute(&self.pool).await?;

        Ok(result_tags)
    }

    async fn find_by_entry_id(&self, user_id: Id, entry_id: Id) -> Result<Vec<TagRow>> {
        // TODO why manual ? + Ok() here needed for type inference?
        Ok(sqlx::query_as::<_, TagRow>(&format!(
            r#"
            SELECT t.* FROM {TAGS_TABLE} t
            INNER JOIN {ENTRIES_TAG_TABLE} et ON et.entry_id = ? AND et.tag_id = t.id
            WHERE t.user_id = ?
        "#,
        ))
        .bind(entry_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    async fn get_all(&self, user_id: Id) -> Result<Vec<TagRow>> {
        Ok(
            sqlx::query_as::<_, TagRow>(
                &format!("SELECT * FROM {TAGS_TABLE} t WHERE user_id = ?",),
            )
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?,
        )
    }

    async fn delete_by_label(&self, user_id: Id, label: &str) -> Result<Option<TagRow>> {
        Ok(sqlx::query_as::<_, TagRow>(&format!(
            "DELETE FROM {TAGS_TABLE} WHERE user_id = ? AND label = ? RETURNING *",
        ))
        .bind(user_id)
        .bind(label)
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn delete_all_by_label(&self, user_id: Id, labels: &[String]) -> Result<Vec<TagRow>> {
        let mut builder = QueryBuilder::new(&format!("DELETE FROM {TAGS_TABLE} WHERE user_id ="));

        builder.push_bind(user_id);

        builder.push(" AND label IN (");

        let mut labels_separated = builder.separated(", ");
        for label in labels {
            labels_separated.push_bind(label);
        }
        labels_separated.push_unseparated(") RETURNING *");

        Ok(builder
            .build_query_as::<TagRow>()
            .fetch_all(&self.pool)
            .await?)
    }

    async fn delete_by_id(&self, user_id: Id, id: Id) -> Result<Option<TagRow>> {
        Ok(sqlx::query_as::<_, TagRow>(&format!(
            "DELETE FROM {TAGS_TABLE} WHERE user_id = ? AND id = ? RETURNING *",
        ))
        .bind(user_id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }
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
    use std::sync::Arc;

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_delete_by_label(pool: SqlitePool) {
        let tag_repo = Arc::new(SqliteTagRepository::new(pool));

        // Verify initial 6 tags from fixtures
        let initial_tags = tag_repo.get_all(1).await.unwrap();
        assert_eq!(initial_tags.len(), 6, "Should have 6 tags initially");

        // Delete "label1" by label
        let deleted_tag = tag_repo.delete_by_label(1, "label1").await.unwrap();
        assert!(deleted_tag.is_some(), "Should return deleted tag");
        let deleted = deleted_tag.unwrap();
        assert_eq!(
            deleted.label, "label1",
            "Deleted tag should have label 'label1'"
        );
        assert_eq!(
            deleted.slug, "slug1",
            "Deleted tag should have slug 'slug1'"
        );

        // Verify only 5 tags remain
        let tags_after = tag_repo.get_all(1).await.unwrap();
        assert_eq!(tags_after.len(), 5, "Should have 5 tags after deletion");

        // Verify CASCADE behavior: entry 2 should lose label1 but keep label2
        let entry_tags = tag_repo.find_by_entry_id(1, 2).await.unwrap();
        assert_eq!(
            entry_tags.len(),
            1,
            "Entry 2 should have 1 tag after cascade"
        );
        assert_eq!(
            entry_tags[0].label, "label2",
            "Entry 2 should only have label2"
        );

        // Try deleting non-existent label
        let not_deleted = tag_repo.delete_by_label(1, "nonexistent").await.unwrap();
        assert!(
            not_deleted.is_none(),
            "Should return None for non-existent label"
        );

        // Verify count unchanged after failed deletion
        let final_tags = tag_repo.get_all(1).await.unwrap();
        assert_eq!(
            final_tags.len(),
            5,
            "Should still have 5 tags after failed deletion"
        );
    }

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_delete_all_by_label(pool: SqlitePool) {
        let tag_repo = Arc::new(SqliteTagRepository::new(pool));

        // Verify initial 6 tags from fixtures
        let initial_tags = tag_repo.get_all(1).await.unwrap();
        assert_eq!(initial_tags.len(), 6, "Should have 6 tags initially");

        // Delete multiple tags: label1, label2, label3
        let labels_to_delete = vec![
            "label1".to_string(),
            "label2".to_string(),
            "label3".to_string(),
        ];
        let deleted_tags = tag_repo
            .delete_all_by_label(1, &labels_to_delete)
            .await
            .unwrap();

        // Verify 3 tags were deleted and returned
        assert_eq!(deleted_tags.len(), 3, "Should return 3 deleted tags");

        // Verify the returned tags have correct labels
        let deleted_labels: Vec<String> = deleted_tags.iter().map(|t| t.label.clone()).collect();
        assert!(deleted_labels.contains(&"label1".to_string()));
        assert!(deleted_labels.contains(&"label2".to_string()));
        assert!(deleted_labels.contains(&"label3".to_string()));

        // Verify only 3 tags remain in database
        let remaining_tags = tag_repo.get_all(1).await.unwrap();
        assert_eq!(remaining_tags.len(), 3, "Should have 3 tags after deletion");

        // Verify remaining tags are label4, label5, label6
        let remaining_labels: Vec<String> =
            remaining_tags.iter().map(|t| t.label.clone()).collect();
        assert!(remaining_labels.contains(&"label4".to_string()));
        assert!(remaining_labels.contains(&"label5".to_string()));
        assert!(remaining_labels.contains(&"label6".to_string()));

        // Verify CASCADE behavior: entry 2 should have no tags (had label1 and label2)
        let entry_tags = tag_repo.find_by_entry_id(1, 2).await.unwrap();
        assert_eq!(
            entry_tags.len(),
            0,
            "Entry 2 should have no tags after cascade"
        );

        // Test deleting with empty vector
        let empty_result = tag_repo.delete_all_by_label(1, &[]).await.unwrap();
        assert_eq!(
            empty_result.len(),
            0,
            "Should return empty vector for empty input"
        );

        // Test deleting mix of existent and non-existent labels
        let mixed_labels = vec![
            "label4".to_string(),
            "nonexistent".to_string(),
            "label5".to_string(),
        ];
        let mixed_deleted = tag_repo
            .delete_all_by_label(1, &mixed_labels)
            .await
            .unwrap();
        assert_eq!(mixed_deleted.len(), 2, "Should only delete existing tags");

        let mixed_deleted_labels: Vec<String> =
            mixed_deleted.iter().map(|t| t.label.clone()).collect();
        assert!(mixed_deleted_labels.contains(&"label4".to_string()));
        assert!(mixed_deleted_labels.contains(&"label5".to_string()));
        assert!(!mixed_deleted_labels.contains(&"nonexistent".to_string()));

        // Verify only 1 tag remains (label6)
        let final_tags = tag_repo.get_all(1).await.unwrap();
        assert_eq!(final_tags.len(), 1, "Should have 1 tag remaining");
        assert_eq!(final_tags[0].label, "label6");

        // Test deleting all non-existent labels
        let nonexistent_labels = vec!["fake1".to_string(), "fake2".to_string()];
        let none_deleted = tag_repo
            .delete_all_by_label(1, &nonexistent_labels)
            .await
            .unwrap();
        assert_eq!(
            none_deleted.len(),
            0,
            "Should return empty vector for non-existent labels"
        );

        // Verify count unchanged
        let unchanged_tags = tag_repo.get_all(1).await.unwrap();
        assert_eq!(unchanged_tags.len(), 1, "Should still have 1 tag");
    }

    #[sqlx::test(
        migrations = "./migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_tag_delete_by_id(pool: SqlitePool) {
        let tag_repo = Arc::new(SqliteTagRepository::new(pool));

        // Verify initial 6 tags from fixtures
        let initial_tags = tag_repo.get_all(1).await.unwrap();
        assert_eq!(initial_tags.len(), 6, "Should have 6 tags initially");

        // Find tag with label "label1" to get its ID
        let tag_to_delete = initial_tags
            .iter()
            .find(|t| t.label == "label1")
            .expect("label1 should exist in fixtures");
        let tag_id = tag_to_delete.id;

        // Delete tag by ID
        let deleted_tag = tag_repo.delete_by_id(1, tag_id).await.unwrap();
        assert!(deleted_tag.is_some(), "Should return deleted tag");
        let deleted = deleted_tag.unwrap();
        assert_eq!(deleted.id, tag_id, "Deleted tag should have correct ID");
        assert_eq!(
            deleted.label, "label1",
            "Deleted tag should have label 'label1'"
        );
        assert_eq!(
            deleted.slug, "slug1",
            "Deleted tag should have slug 'slug1'"
        );

        // Verify only 5 tags remain
        let remaining_tags = tag_repo.get_all(1).await.unwrap();
        assert_eq!(remaining_tags.len(), 5, "Should have 5 tags after deletion");

        // Verify the deleted tag is not in remaining tags
        assert!(
            !remaining_tags.iter().any(|t| t.id == tag_id),
            "Deleted tag should not be in remaining tags"
        );

        // Verify CASCADE behavior: entry 2 should lose label1 but keep label2
        let entry_tags = tag_repo.find_by_entry_id(1, 2).await.unwrap();
        assert_eq!(
            entry_tags.len(),
            1,
            "Entry 2 should have 1 tag after cascade"
        );
        assert_eq!(
            entry_tags[0].label, "label2",
            "Entry 2 should only have label2"
        );

        // Try deleting non-existent tag by ID
        let not_deleted = tag_repo.delete_by_id(1, 999).await.unwrap();
        assert!(
            not_deleted.is_none(),
            "Should return None for non-existent ID"
        );

        // Verify count unchanged after failed deletion
        let final_tags = tag_repo.get_all(1).await.unwrap();
        assert_eq!(
            final_tags.len(),
            5,
            "Should still have 5 tags after failed deletion"
        );
    }
}
