use sqlx::{FromRow, QueryBuilder, Row, sqlite::SqliteRow};

use result::ArticlerResult;

use super::{
    Db, DbErrorType, ENTRIES_TABLE, ENTRIES_TAG_TABLE, Id, SQLITE_LIMIT_VARIABLE_NUMBER, TAGS_TABLE,
};

/* Return Vec of tags, which was linked to entry_id. Vec consists of ALL tags, even tags, which was already linked before and included in tags argument. */
pub async fn create_and_link<'c, C>(
    conn: C,
    entry_id: Id,
    tags: &[CreateTag],
) -> ArticlerResult<Vec<TagRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    if tags.is_empty() {
        return Ok(vec![]);
    }

    if tags.len() > SQLITE_LIMIT_VARIABLE_NUMBER / 2 {
        return Err(DbErrorType::RepositoryError(format!(
            "Too many tags: {} exceeds limit of {}",
            tags.len(),
            SQLITE_LIMIT_VARIABLE_NUMBER / 2
        ))
        .into());
    }

    let mut conn = conn.acquire().await?;

    let mut tag_builder = QueryBuilder::new("INSERT INTO tags (user_id, label, slug) ");
    tag_builder.push_values(tags.iter(), |mut b, tag| {
        b.push_bind(tag.user_id)
            .push_bind(&tag.label)
            .push_bind(&tag.slug);
    });
    tag_builder.push(" ON CONFLICT DO NOTHING");
    tag_builder.build().execute(&mut *conn).await?;

    let mut insert_query = QueryBuilder::new(format!(r"INSERT INTO {ENTRIES_TAG_TABLE} SELECT "));
    insert_query.push(entry_id);
    insert_query.push(format!(
        " as entry_id, id as tag_id FROM {TAGS_TABLE} WHERE label IN ("
    ));
    let mut separated = insert_query.separated(", ");
    for tag in tags {
        separated.push_bind(&tag.label);
    }
    separated.push_unseparated(") ON CONFLICT DO NOTHING");

    insert_query.build().execute(&mut *conn).await?;

    let mut get_tags = QueryBuilder::new(format!("SELECT * from {TAGS_TABLE} WHERE label IN ("));

    let mut tags_separated = get_tags.separated(", ");
    for tag in tags {
        tags_separated.push_bind(&tag.label);
    }
    tags_separated.push_unseparated(")");

    Ok(get_tags
        .build_query_as::<TagRow>()
        .fetch_all(&mut *conn)
        .await?)
}

pub async fn update_tags_by_entry_id<'c, C>(
    conn: C,
    user_id: Id,
    entry_id: Id,
    tags: &[CreateTag],
) -> ArticlerResult<Vec<TagRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;
    let result_tags = create_and_link(&mut *conn, entry_id, tags).await?;

    let mut builder = QueryBuilder::new(format!(
        "DELETE FROM {ENTRIES_TAG_TABLE} WHERE entry_id IN (SELECT id FROM {ENTRIES_TABLE} WHERE entry_id =",
    ));

    builder.push_bind(entry_id);

    builder.push(" AND user_id = ");
    builder.push_bind(user_id);
    builder.push(") ");

    builder.push(format!(
        r"
         AND tag_id NOT IN (
            SELECT id FROM {TAGS_TABLE} t WHERE t.label IN (
    ",
    ));

    let mut separated = builder.separated(", ");
    for t in tags {
        separated.push_bind(&t.label);
    }

    separated.push_unseparated("))");

    builder.build().execute(&mut *conn).await?;

    Ok(result_tags)
}

pub async fn find_by_entry_id<'c, C>(
    conn: C,
    user_id: Id,
    entry_id: Id,
) -> ArticlerResult<Vec<TagRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;
    Ok(sqlx::query_as::<_, TagRow>(&format!(
        r"
        SELECT t.* FROM {TAGS_TABLE} t
        INNER JOIN {ENTRIES_TAG_TABLE} et ON et.entry_id = ? AND et.tag_id = t.id
        WHERE t.user_id = ?
    ",
    ))
    .bind(entry_id)
    .bind(user_id)
    .fetch_all(&mut *conn)
    .await?)
}

pub async fn get_all<'c, C>(conn: C, user_id: Id) -> ArticlerResult<Vec<TagRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;
    Ok(
        sqlx::query_as::<_, TagRow>(&format!("SELECT * FROM {TAGS_TABLE} t WHERE user_id = ?",))
            .bind(user_id)
            .fetch_all(&mut *conn)
            .await?,
    )
}

pub async fn delete_by_label<'c, C>(
    conn: C,
    user_id: Id,
    label: &str,
) -> ArticlerResult<Option<TagRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;
    Ok(sqlx::query_as::<_, TagRow>(&format!(
        "DELETE FROM {TAGS_TABLE} WHERE user_id = ? AND label = ? RETURNING *",
    ))
    .bind(user_id)
    .bind(label)
    .fetch_optional(&mut *conn)
    .await?)
}

pub async fn delete_all_by_label<'c, C>(
    conn: C,
    user_id: Id,
    labels: &[String],
) -> ArticlerResult<Vec<TagRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;
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
        .fetch_all(&mut *conn)
        .await?)
}

pub async fn delete_by_id<'c, C>(conn: C, user_id: Id, id: Id) -> ArticlerResult<Option<TagRow>>
where
    C: sqlx::Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await?;
    Ok(sqlx::query_as::<_, TagRow>(&format!(
        "DELETE FROM {TAGS_TABLE} WHERE user_id = ? AND id = ? RETURNING *",
    ))
    .bind(user_id)
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?)
}

#[derive(Debug, PartialEq)]
pub struct TagRow {
    pub id: Id,
    pub user_id: Id,
    pub label: String,
    pub slug: String,
}

impl<'r> FromRow<'r, SqliteRow> for TagRow {
    fn from_row(row: &'r SqliteRow) -> std::result::Result<TagRow, sqlx::Error> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_delete_by_label(pool: SqlitePool) {
        let initial_tags = get_all(&pool, 1).await.unwrap();
        assert_eq!(initial_tags.len(), 6, "Should have 6 tags initially");

        let deleted_tag = delete_by_label(&pool, 1, "label1").await.unwrap();
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

        let tags_after = get_all(&pool, 1).await.unwrap();
        assert_eq!(tags_after.len(), 5, "Should have 5 tags after deletion");

        let entry_tags = find_by_entry_id(&pool, 1, 2).await.unwrap();
        assert_eq!(
            entry_tags.len(),
            1,
            "Entry 2 should have 1 tag after cascade"
        );
        assert_eq!(
            entry_tags[0].label, "label2",
            "Entry 2 should only have label2"
        );

        let not_deleted = delete_by_label(&pool, 1, "nonexistent").await.unwrap();
        assert!(
            not_deleted.is_none(),
            "Should return None for non-existent label"
        );

        let final_tags = get_all(&pool, 1).await.unwrap();
        assert_eq!(
            final_tags.len(),
            5,
            "Should still have 5 tags after failed deletion"
        );
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_delete_all_by_label(pool: SqlitePool) {
        let initial_tags = get_all(&pool, 1).await.unwrap();
        assert_eq!(initial_tags.len(), 6, "Should have 6 tags initially");

        let labels_to_delete = vec![
            "label1".to_owned(),
            "label2".to_owned(),
            "label3".to_owned(),
        ];
        let deleted_tags = delete_all_by_label(&pool, 1, &labels_to_delete)
            .await
            .unwrap();

        assert_eq!(deleted_tags.len(), 3, "Should return 3 deleted tags");

        let deleted_labels: Vec<String> = deleted_tags.iter().map(|t| t.label.clone()).collect();
        assert!(deleted_labels.contains(&"label1".to_owned()));
        assert!(deleted_labels.contains(&"label2".to_owned()));
        assert!(deleted_labels.contains(&"label3".to_owned()));

        let remaining_tags = get_all(&pool, 1).await.unwrap();
        assert_eq!(remaining_tags.len(), 3, "Should have 3 tags after deletion");

        let remaining_labels: Vec<String> =
            remaining_tags.iter().map(|t| t.label.clone()).collect();
        assert!(remaining_labels.contains(&"label4".to_owned()));
        assert!(remaining_labels.contains(&"label5".to_owned()));
        assert!(remaining_labels.contains(&"label6".to_owned()));

        let entry_tags = find_by_entry_id(&pool, 1, 2).await.unwrap();
        assert_eq!(
            entry_tags.len(),
            0,
            "Entry 2 should have no tags after cascade"
        );

        let empty_result = delete_all_by_label(&pool, 1, &[]).await.unwrap();
        assert_eq!(
            empty_result.len(),
            0,
            "Should return empty vector for empty input"
        );

        let mixed_labels = vec![
            "label4".to_owned(),
            "nonexistent".to_owned(),
            "label5".to_owned(),
        ];
        let mixed_deleted = delete_all_by_label(&pool, 1, &mixed_labels).await.unwrap();
        assert_eq!(mixed_deleted.len(), 2, "Should only delete existing tags");

        let mixed_deleted_labels: Vec<String> =
            mixed_deleted.iter().map(|t| t.label.clone()).collect();
        assert!(mixed_deleted_labels.contains(&"label4".to_owned()));
        assert!(mixed_deleted_labels.contains(&"label5".to_owned()));
        assert!(!mixed_deleted_labels.contains(&"nonexistent".to_owned()));

        let final_tags = get_all(&pool, 1).await.unwrap();
        assert_eq!(final_tags.len(), 1, "Should have 1 tag remaining");
        assert_eq!(final_tags[0].label, "label6");

        let nonexistent_labels = vec!["fake1".to_owned(), "fake2".to_owned()];
        let none_deleted = delete_all_by_label(&pool, 1, &nonexistent_labels)
            .await
            .unwrap();
        assert_eq!(
            none_deleted.len(),
            0,
            "Should return empty vector for non-existent labels"
        );

        let unchanged_tags = get_all(&pool, 1).await.unwrap();
        assert_eq!(unchanged_tags.len(), 1, "Should still have 1 tag");
    }

    #[sqlx::test(
        migrations = "../../migrations",
        fixtures("../../tests/fixtures/users.sql", "../../tests/fixtures/entries.sql")
    )]
    async fn test_tag_delete_by_id(pool: SqlitePool) {
        let initial_tags = get_all(&pool, 1).await.unwrap();
        assert_eq!(initial_tags.len(), 6, "Should have 6 tags initially");

        let tag_to_delete = initial_tags
            .iter()
            .find(|t| t.label == "label1")
            .expect("label1 should exist in fixtures");
        let tag_id = tag_to_delete.id;

        let deleted_tag = delete_by_id(&pool, 1, tag_id).await.unwrap();
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

        let remaining_tags = get_all(&pool, 1).await.unwrap();
        assert_eq!(remaining_tags.len(), 5, "Should have 5 tags after deletion");

        assert!(
            !remaining_tags.iter().any(|t| t.id == tag_id),
            "Deleted tag should not be in remaining tags"
        );

        let entry_tags = find_by_entry_id(&pool, 1, 2).await.unwrap();
        assert_eq!(
            entry_tags.len(),
            1,
            "Entry 2 should have 1 tag after cascade"
        );
        assert_eq!(
            entry_tags[0].label, "label2",
            "Entry 2 should only have label2"
        );

        let not_deleted = delete_by_id(&pool, 1, 999).await.unwrap();
        assert!(
            not_deleted.is_none(),
            "Should return None for non-existent ID"
        );

        let final_tags = get_all(&pool, 1).await.unwrap();
        assert_eq!(
            final_tags.len(),
            5,
            "Should still have 5 tags after failed deletion"
        );
    }
}
