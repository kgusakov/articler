use crate::error::{
    NotFoundSnafu, NotImpementedSnafu, Result, UnexpectedStateSnafu, UrlFormatSnafu,
};
use actix_web::{
    Either,
    web::{self, Json, Query},
};
use app_state::AppState;
use chrono::Utc;
use slug::slugify;
use snafu::ResultExt;
use url::Url;

use crate::{
    UserInfo,
    models::{Entry, Tag},
    wallabag::Id,
};
use db::repository::{entries, tags};
use dto::{
    AddEntry, AddEntryResponse, DeleteEntryRequest, DeleteEntryResponse, Embedded, Entries,
    EntriesRequest, EntryTags, Exists, Expect, Link, Links, UpdateEntry,
};
use helpers::{generate_uid, hash_url};

// TODO current implementation needed only for mobile app healthchecks. Needed full implementation
pub(crate) async fn exists() -> Result<Json<Exists>> {
    Ok(Json(Exists { exists: false }))
}

pub(crate) async fn post_entries(
    data: web::Data<AppState>,
    request: Either<web::Json<AddEntry>, web::Form<AddEntry>>,
    user_info: UserInfo,
) -> Result<Json<AddEntryResponse>> {
    // TODO
    // Check if url already exist:
    //    - if exist update the current entry
    //    - if not - create new one
    //
    // In both case if title and/or content is not set - both will be retrieved from the internet again
    let add_entry = request.into_inner();

    if let (Some(_), Some(_)) = (add_entry.title, add_entry.content) {
        // TODO add support of receiving title and html to skip data crawling outside step
        return NotImpementedSnafu {
            msg: "Title and content fields is not supported yet",
        }
        .fail();
    }

    let document = data.scraper.extract_or_fallback(&add_entry.url).await;

    let domain_name = add_entry.url.domain().or(add_entry.url.host_str());

    let now = Utc::now().timestamp();

    let archived = add_entry.archive.unwrap_or(false);
    let starred = add_entry.starred.unwrap_or(false);

    let preview_picture = document.image_url.map(|u| u.to_string());

    let published_at = document.published_at.map(|v| v.timestamp());

    // TODO can we remove all these ugly to_string?
    let create_entry = entries::CreateEntry {
        user_id: user_info.user_id,
        // TODO actually here we must have url without redirects already
        url: add_entry.url.to_string(),
        hashed_url: hash_url(&add_entry.url),
        given_url: add_entry.url.to_string(),
        hashed_given_url: hash_url(&add_entry.url),
        title: document.title,
        content: document.content_html,
        content_text: document.content_text,
        is_archived: archived,
        archived_at: if archived { Some(now) } else { None },
        is_starred: starred,
        starred_at: if starred { Some(now) } else { None },
        created_at: now,
        updated_at: now,
        mimetype: document.mime_type,
        language: document.language,
        reading_time: document.reading_time,
        domain_name: domain_name.unwrap_or("").to_owned(),
        preview_picture,
        origin_url: add_entry.origin_url,
        published_at,
        published_by: add_entry.authors.map(|a| a.join(",")),
        is_public: add_entry.public,
        uid: add_entry.public.filter(|p| *p).map(|_b| generate_uid()),
    };

    let tag_to_create_tag = |label: String| -> tags::CreateTag {
        tags::CreateTag {
            user_id: user_info.user_id,
            slug: slugify(&label),
            label,
        }
    };

    let create_tags = add_entry
        .tags
        .map(|tags| {
            tags.into_iter()
                .map(tag_to_create_tag)
                .collect::<Vec<tags::CreateTag>>()
        })
        // TODO if it is not new entry - we will force empty tags. It should be fixed when this method will support not only entry creations
        .unwrap_or(vec![]);

    let (entry_row, tag_rows) = entries::create(&data.pool, create_entry, &create_tags).await?;

    let tags = tag_rows.into_iter().map(Tag::from).collect();

    // TODO replace by real url
    let self_url = Url::parse("https://example.com").context(UrlFormatSnafu)?;

    Ok(web::Json(AddEntryResponse {
        entry: Entry::try_from((entry_row, tags))?,
        _links: Links {
            _self: Link { href: self_url },
            first: None,
            last: None,
            next: None,
        },
    }))
}

pub(crate) async fn entries(
    data: web::Data<AppState>,
    request: Query<EntriesRequest>,
    user_info: UserInfo,
) -> Result<Json<Entries>> {
    let request = request.into_inner();

    let params = entries::FindParams {
        user_id: user_info.user_id,
        archive: request.archive,
        starred: request.starred,
        sort: Some(request.sort.into()),
        order: Some(request.order.into()),
        page: Some(request.page),
        per_page: Some(request.per_page),
        tags: request.tags,
        since: Some(request.since),
        public: request.public,
        detail: Some(request.detail.into()),
        domain_name: request.domain_name,
    };

    let mut tx = data.pool.begin().await?;

    let count_without_paging = entries::count(&mut *tx, &params).await?;

    // TODO implement all needed request filters and etc
    let entries = entries::find_all(&mut *tx, &params).await?;

    tx.commit().await?;

    // TODO fix clippy
    #[expect(clippy::cast_precision_loss)]
    #[expect(clippy::cast_possible_truncation)]
    let pages = (count_without_paging as f64 / request.per_page as f64).ceil() as i64;

    if request.page > pages {
        return NotFoundSnafu {
            msg: "Page not found",
        }
        .fail();
    }

    let mut ents = vec![];

    for (e, tags) in entries {
        let mapped_tags: Vec<Tag> = tags.into_iter().map(std::convert::Into::into).collect();
        ents.push(Entry::try_from((e, mapped_tags))?);
    }

    // TODO implement actual urls generating
    let url = Url::parse("http://example.com").context(UrlFormatSnafu)?;

    Ok(web::Json(Entries {
        page: request.page,
        limit: request.per_page,
        pages,
        total: count_without_paging as i64,
        embedded: Embedded { items: ents },
        _links: Links {
            _self: Link { href: url.clone() },
            first: Some(Link { href: url.clone() }),
            last: Some(Link { href: url.clone() }),
            next: Some(Link { href: url.clone() }),
        },
    }))
}

pub(crate) async fn get_tags_by_entry(
    data: web::Data<AppState>,
    entry_id: web::Path<Id>,
    user_info: UserInfo,
) -> Result<Json<Vec<Tag>>> {
    let entry_id = entry_id.into_inner();

    let mut tx = data.pool.begin().await?;

    let result = if entries::exists_by_id(&mut *tx, user_info.user_id, entry_id).await? {
        let result = tags::find_by_entry_id(&mut *tx, user_info.user_id, entry_id)
            .await?
            .into_iter()
            .map(std::convert::Into::into)
            .collect();

        Ok(Json(result))
    } else {
        NotFoundSnafu {
            msg: "Entry not found",
        }
        .fail()
    };

    tx.commit().await?;

    result
}

pub(crate) async fn delete_tag_from_entry(
    data: web::Data<AppState>,
    ids: web::Path<(Id, Id)>,
    user_info: UserInfo,
) -> Result<Json<Entry>> {
    let (entry_id, tag_id) = ids.into_inner();

    let mut tx = data.pool.begin().await?;

    let result = if entries::exists_by_id(&mut *tx, user_info.user_id, entry_id).await? {
        entries::delete_tag_by_tag_id(&mut *tx, user_info.user_id, entry_id, tag_id).await?;

        if let Some((entry_row, tag_rows)) =
            entries::find_by_id(&mut *tx, user_info.user_id, entry_id).await?
        {
            let tags = tag_rows.into_iter().map(std::convert::Into::into).collect();

            Ok(Json(Entry::try_from((entry_row, tags))?))
        } else {
            UnexpectedStateSnafu {
                msg: "Can't find entry by id",
            }
            .fail()
        }
    } else {
        NotFoundSnafu {
            msg: "Entry not found",
        }
        .fail()
    };

    tx.commit().await?;

    result
}

pub(crate) async fn delete_entry(
    data: web::Data<AppState>,
    entry_id: web::Path<i64>,
    request: Query<DeleteEntryRequest>,
    user_info: UserInfo,
) -> Result<Json<DeleteEntryResponse>> {
    let request = request.into_inner();
    let entry_id = entry_id.into_inner();

    let mut tx = data.pool.begin().await?;

    let result = match request.expect {
        Expect::Id => {
            let deleted = entries::delete_by_id(&mut *tx, user_info.user_id, entry_id).await?;

            if !deleted {
                return NotFoundSnafu {
                    msg: "Entry not found",
                }
                .fail();
            }

            Ok(Json(DeleteEntryResponse::Id { id: entry_id }))
        }
        Expect::Full => {
            let full_entry = entries::find_by_id(&mut *tx, user_info.user_id, entry_id).await?;

            let (entry_row, tag_rows) = full_entry.ok_or_else(|| {
                NotFoundSnafu {
                    msg: "Entry not found",
                }
                .build()
            })?;

            let deleted = entries::delete_by_id(&mut *tx, user_info.user_id, entry_id).await?;

            if !deleted {
                return NotFoundSnafu {
                    msg: "Entry not found",
                }
                .fail();
            }

            let tags: Vec<Tag> = tag_rows.into_iter().map(std::convert::Into::into).collect();
            let entry = Entry::try_from((entry_row, tags))?;

            Ok(Json(DeleteEntryResponse::Full {
                entry: Box::new(entry),
            }))
        }
    };

    tx.commit().await?;

    result
}

// TODO implement Either based version for Json input data also
pub(crate) async fn post_entry_tags(
    data: web::Data<AppState>,
    entry_id: web::Path<Id>,
    request: web::Form<EntryTags>,
    user_info: UserInfo,
) -> Result<Json<Entry>> {
    let entry_id = entry_id.into_inner();

    let mut tx = data.pool.begin().await?;

    // TODO dirty design - looks like we need entry repository method for it
    let result: Result<Json<Entry>> = if entries::find_by_id(&mut *tx, user_info.user_id, entry_id)
        .await?
        .is_some()
    {
        let full_tags: Vec<tags::CreateTag> = request
            .into_inner()
            .labels
            .into_iter()
            .map(|l| tags::CreateTag {
                user_id: user_info.user_id,
                slug: slugify(&l),
                label: l,
            })
            .collect();

        tags::update_tags_by_entry_id(&mut *tx, user_info.user_id, entry_id, &full_tags).await?;

        let (entry_row, tag_rows) = entries::find_by_id(&mut *tx, user_info.user_id, entry_id)
            .await?
            .ok_or(
                NotFoundSnafu {
                    msg: "Entry not found",
                }
                .build(),
            )?;

        let entry_tags = tag_rows.into_iter().map(Tag::from).collect();

        Ok(Json(Entry::try_from((entry_row, entry_tags))?))
    } else {
        NotFoundSnafu {
            msg: "Entry not found",
        }
        .fail()
    };

    tx.commit().await?;

    result
}

pub(crate) async fn patch_entry(
    data: web::Data<AppState>,
    entry_id: web::Path<i64>,
    request: Either<web::Json<UpdateEntry>, web::Form<UpdateEntry>>,
    user_info: UserInfo,
) -> Result<Json<Entry>> {
    let request = request.into_inner();
    let entry_id = entry_id.into_inner();

    let now = Utc::now().timestamp();

    let repo_update = entries::UpdateEntry {
        title: request.title.map(Some),
        content: request.content.map(Some),
        is_archived: request.archive.map(Some),
        archived_at: match request.archive {
            Some(true) => Some(Some(now)),
            Some(false) => Some(None),
            None => None,
        },
        is_starred: request.starred.map(Some),
        starred_at: match request.starred {
            Some(true) => Some(Some(now)),
            Some(false) => Some(None),
            None => None,
        },
        updated_at: now,
        language: request.language.map(Some),
        reading_time: None,
        preview_picture: request.preview_picture.map(|u| Some(u.to_string())),
        origin_url: request.origin_url.map(Some),
        published_at: request.published_at.map(|dt| Some(dt.timestamp())),
        published_by: request.authors.map(|authors| Some(authors.join(","))),
        is_public: request.public.map(Some),
        // TODO must be not regenerated if already was public?
        uid: match request.public {
            Some(true) => Some(Some(generate_uid())),
            // TODO must be setted to null, but will be ignored by update
            Some(false) => Some(None),
            None => None,
        },
    };

    let mut tx = data.pool.begin().await?;

    let updated = entries::update_by_id(&mut *tx, user_info.user_id, entry_id, repo_update).await?;

    if !updated {
        tx.rollback().await?;

        return NotFoundSnafu {
            msg: "Entry not found",
        }
        .fail();
    }

    if let Some(tags_labels) = request.tags {
        let full_tags: Vec<tags::CreateTag> = tags_labels
            .into_iter()
            .map(|l| tags::CreateTag {
                user_id: user_info.user_id,
                slug: slugify(&l),
                label: l,
            })
            .collect();

        tags::update_tags_by_entry_id(&mut *tx, user_info.user_id, entry_id, &full_tags).await?;
    }

    let (entry_row, tag_rows) = entries::find_by_id(&mut *tx, user_info.user_id, entry_id)
        .await?
        .ok_or_else(|| {
            NotFoundSnafu {
                msg: "Entry not found",
            }
            .build()
        })?;

    let entry_tags = tag_rows.into_iter().map(std::convert::Into::into).collect();

    let entry = Entry::try_from((entry_row, entry_tags))?;

    tx.commit().await?;

    Ok(Json(entry))
}

mod dto {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    use serde_with::{BoolFromInt, StringWithSeparator};
    use serde_with::{formats::CommaSeparator, serde_as};
    use snafu::ResultExt;
    use url::Url;

    use crate::error::UrlFormatSnafu;
    use crate::error::{Result, TimestampToDateTimeSnafu};
    use crate::models::{Entry, Tag};
    use db::repository::{entries, tags};

    #[derive(Default, Deserialize, Debug, PartialEq, Clone, Copy)]
    pub enum Expect {
        #[default]
        #[serde(rename(deserialize = "id"))]
        Id,
        #[serde(rename(deserialize = "full"))]
        Full,
    }

    #[derive(Deserialize, Debug)]
    pub struct DeleteEntryRequest {
        #[serde(default)]
        pub expect: Expect,
    }

    #[derive(Serialize)]
    #[serde(untagged)]
    pub enum DeleteEntryResponse {
        Id {
            id: i64,
        },
        Full {
            #[serde(flatten)]
            entry: Box<Entry>,
        },
    }

    #[serde_as]
    #[derive(Deserialize)]
    pub struct EntryTags {
        #[serde(rename(deserialize = "tags"))]
        #[serde_as(as = "StringWithSeparator::<CommaSeparator, String>")]
        pub labels: Vec<String>,
    }

    #[derive(Serialize)]
    pub struct AddEntryResponse {
        #[serde(flatten)]
        pub entry: Entry,
        pub _links: Links,
    }

    #[derive(Serialize)]
    pub struct Embedded {
        pub items: Vec<Entry>,
    }

    #[derive(Serialize)]
    pub struct Entries {
        pub page: i64,
        pub limit: i64,
        pub pages: i64,
        pub total: i64,
        #[serde(rename(serialize = "_embedded"))]
        pub embedded: Embedded,
        pub _links: Links,
    }

    fn try_parse_url(s: Option<String>) -> Result<Option<Url>> {
        s.map(|u| Url::parse(&u))
            .transpose()
            .context(UrlFormatSnafu)
    }

    fn try_parse_timestamp_opt(s: Option<i64>) -> Result<Option<DateTime<Utc>>> {
        match s {
            Some(t) => match DateTime::from_timestamp_secs(t) {
                Some(r) => Ok(Some(r)),
                None => TimestampToDateTimeSnafu { timestamp: t }.fail(),
            },
            None => Ok(None),
        }
    }

    fn try_parse_timestamp(s: i64) -> Result<DateTime<Utc>> {
        match DateTime::from_timestamp_secs(s) {
            Some(r) => Ok(r),
            None => TimestampToDateTimeSnafu { timestamp: s }.fail(),
        }
    }

    impl From<tags::TagRow> for Tag {
        fn from(value: tags::TagRow) -> Self {
            Self {
                id: value.id,
                label: value.label,
                slug: value.slug,
            }
        }
    }

    impl TryFrom<(entries::EntryRow, Vec<Tag>)> for Entry {
        type Error = crate::error::Error;

        fn try_from(
            (e, tags): (entries::EntryRow, Vec<Tag>),
        ) -> std::result::Result<Self, Self::Error> {
            Ok(Entry {
                id: e.id,
                url: Url::parse(&e.url).context(UrlFormatSnafu)?,
                hashed_url: e.hashed_url,
                given_url: try_parse_url(e.given_url)?,
                hashed_given_url: e.hashed_given_url,
                title: e.title,
                content: e.content,
                is_archived: e.is_archived,
                archived_at: try_parse_timestamp_opt(e.archived_at)?,
                is_starred: e.is_starred,
                starred_at: try_parse_timestamp_opt(e.starred_at)?,
                tags,
                created_at: try_parse_timestamp(e.created_at)?,
                updated_at: try_parse_timestamp(e.updated_at)?,
                // TODO implement annotations support
                annotations: vec![],
                mimetype: e.mimetype.unwrap_or(String::new()),
                language: e.language,
                reading_time: e.reading_time,
                domain_name: e.domain_name,
                preview_picture: try_parse_url(e.preview_picture)?,
                origin_url: try_parse_url(e.origin_url)?,
                published_at: try_parse_timestamp_opt(e.published_at)?,
                // TODO this .map(to_string) look ugly
                published_by: e
                    .published_by
                    .map(|s| s.split(',').map(std::borrow::ToOwned::to_owned).collect()),
                is_public: e.is_public,
                uid: e.uid,
            })
        }
    }

    #[derive(Default, Deserialize, Debug, PartialEq, Clone, Copy)]
    pub enum FindSortEnum {
        #[default]
        #[serde(rename(deserialize = "created"))]
        Created,
        #[serde(rename(deserialize = "updated"))]
        Updated,
        #[serde(rename(deserialize = "archived"))]
        Archived,
    }

    impl From<FindSortEnum> for entries::SortColumn {
        fn from(val: FindSortEnum) -> Self {
            match val {
                FindSortEnum::Created => entries::SortColumn::Created,
                FindSortEnum::Updated => entries::SortColumn::Updated,
                FindSortEnum::Archived => entries::SortColumn::Archived,
            }
        }
    }

    #[derive(Default, Deserialize, Debug, PartialEq, Clone, Copy)]
    pub enum FindSortOrder {
        #[default]
        #[serde(rename(deserialize = "asc"))]
        Asc,
        #[serde(rename(deserialize = "desc"))]
        Desc,
    }

    impl From<FindSortOrder> for entries::SortOrder {
        fn from(val: FindSortOrder) -> Self {
            match val {
                FindSortOrder::Asc => entries::SortOrder::Asc,
                FindSortOrder::Desc => entries::SortOrder::Desc,
            }
        }
    }

    #[derive(Default, Deserialize, Debug, PartialEq, Clone, Copy)]
    pub enum Detail {
        #[serde(rename(deserialize = "metadata"))]
        Metadata,
        #[default]
        #[serde(rename(deserialize = "full"))]
        Full,
    }

    impl From<Detail> for entries::Detail {
        fn from(val: Detail) -> Self {
            match val {
                Detail::Full => entries::Detail::Full,
                Detail::Metadata => entries::Detail::Metadata,
            }
        }
    }

    fn default_page() -> i64 {
        1
    }

    fn default_per_page() -> i64 {
        30
    }

    #[serde_as]
    #[derive(Deserialize, PartialEq, Debug)]
    pub struct AddEntry {
        pub title: Option<String>,
        /// Raw html content, not processed by readability yet
        pub content: Option<String>,
        #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
        pub tags: Option<Vec<String>>,
        #[serde_as(as = "Option<BoolFromInt>")]
        pub archive: Option<bool>,
        #[serde_as(as = "Option<BoolFromInt>")]
        pub starred: Option<bool>,
        pub url: Url,
        pub language: Option<String>,
        pub published_at: Option<DateTime<Utc>>,
        pub preview_picture: Option<Url>,
        #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
        pub authors: Option<Vec<String>>,
        #[serde_as(as = "Option<BoolFromInt>")]
        pub public: Option<bool>,
        pub origin_url: Option<String>,
    }

    #[serde_as]
    #[derive(Deserialize, PartialEq, Debug)]
    pub struct UpdateEntry {
        pub title: Option<String>,
        pub content: Option<String>,
        #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
        pub tags: Option<Vec<String>>,
        #[serde_as(as = "Option<BoolFromInt>")]
        pub archive: Option<bool>,
        #[serde_as(as = "Option<BoolFromInt>")]
        pub starred: Option<bool>,
        pub language: Option<String>,
        pub published_at: Option<DateTime<Utc>>,
        pub preview_picture: Option<Url>,
        #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
        pub authors: Option<Vec<String>>,
        #[serde_as(as = "Option<BoolFromInt>")]
        pub public: Option<bool>,
        pub origin_url: Option<String>,
    }

    #[serde_as]
    #[derive(Deserialize, PartialEq, Debug)]
    pub struct EntriesRequest {
        #[serde_as(as = "Option<BoolFromInt>")]
        pub archive: Option<bool>,
        #[serde_as(as = "Option<BoolFromInt>")]
        pub starred: Option<bool>,
        #[serde(default)]
        pub sort: FindSortEnum,
        #[serde(default)]
        pub order: FindSortOrder,
        #[serde(default = "default_page")]
        pub page: i64,
        #[serde(rename(deserialize = "perPage"))]
        #[serde(default = "default_per_page")]
        pub per_page: i64,
        #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
        pub tags: Option<Vec<String>>,
        #[serde(default)]
        pub since: i64,
        #[serde_as(as = "Option<BoolFromInt>")]
        pub public: Option<bool>,
        #[serde(default)]
        pub detail: Detail,
        pub domain_name: Option<String>,
    }

    #[derive(Serialize)]
    pub struct Links {
        #[serde(rename(serialize = "self"))]
        pub _self: Link,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub first: Option<Link>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub last: Option<Link>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub next: Option<Link>,
    }

    #[derive(Serialize)]
    pub struct Link {
        pub href: Url,
    }

    #[derive(Debug, Serialize)]
    pub struct Exists {
        pub exists: bool,
    }
}
