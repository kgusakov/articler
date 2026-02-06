use actix_web::{
    error::{ErrorInternalServerError, ErrorNotFound},
    web::{self, Json, Query},
};
use chrono::{DateTime, Utc};
use log::error;
use serde::{Deserialize, Serialize};
use serde_with::{BoolFromInt, StringWithSeparator};
use serde_with::{formats::CommaSeparator, serde_as};
use slug::slugify;
use thiserror::Error;
use url::Url;

use crate::{
    api::{oauth::UserInfo, wallabag::Id},
    app::AppState,
    middleware::TransactionContext,
    models::{Entry, Tag},
    scraper::extract_title,
};
use db::repository::{entries, tags};
use helpers::{generate_uid, hash_str};
use result::{ArticlerError, ArticlerResult};

// TODO current implementation needed only for mobile app healthchecks. Needed full implementation
pub async fn exists() -> actix_web::Result<Json<Exists>> {
    Ok(Json(Exists { exists: false }))
}

pub async fn post_entries(
    tctx: web::ReqData<TransactionContext<'_>>,
    data: web::Data<AppState>,
    request: web::Form<AddEntry>,
    user_info: UserInfo,
) -> actix_web::Result<Json<AddEntryResponse>> {
    do_post_entries(tctx, data, request.into_inner(), user_info).await
}

pub async fn post_entries_json(
    tctx: web::ReqData<TransactionContext<'_>>,
    data: web::Data<AppState>,
    request: web::Json<AddEntry>,
    user_info: UserInfo,
) -> actix_web::Result<Json<AddEntryResponse>> {
    do_post_entries(tctx, data, request.into_inner(), user_info).await
}

async fn do_post_entries(
    tctx: web::ReqData<TransactionContext<'_>>,
    data: web::Data<AppState>,
    request: AddEntry,
    user_info: UserInfo,
) -> actix_web::Result<Json<AddEntryResponse>> {
    // TODO
    // Check if url already exist:
    //    - if exist update the current entry
    //    - if not - create new one
    //
    // In both case if title and/or content is not set - both will be retrieved from the internet again
    let (title, content, mime_type, published_at, language, preview_picture) =
        if let (Some(t), Some(c)) = (request.title, request.content) {
            (
                t,
                c,
                "".to_string(),
                request.published_at,
                request.language,
                request.preview_picture,
            )
        } else {
            match data.scraper.extract(&request.url).await {
                Ok(document) => (
                    document.title,
                    document.content_html,
                    document.mime_type.unwrap_or("".to_string()),
                    document.published_at,
                    document.language,
                    document.image_url,
                ),
                Err(err) => {
                    error!("Error while parsing url {}: {:?}", request.url, err);
                    (
                        // TODO abstraction is leaking here - we need to generalize handling of parsing errors
                        extract_title(&request.url).to_string(),
                        "".to_string(),
                        "".to_string(),
                        None,
                        None,
                        None,
                    )
                }
            }
        };

    // TODO must be calculated
    let reading_time = 0;

    let domain_name = request.url.domain().or(request.url.host_str());

    let now = Utc::now().timestamp();

    let archived = request.archive.unwrap_or(false);
    let starred = request.starred.unwrap_or(false);

    // TODO can we remove all these ugly to_string?
    let create_entry = entries::CreateEntry {
        user_id: user_info.user_id,
        // TODO actually here we must have url without redirects already
        url: request.url.to_string(),
        hashed_url: hash_str(request.url.as_str()),
        given_url: request.url.to_string(),
        hashed_given_url: hash_str(request.url.as_str()),
        title,
        content,
        is_archived: archived,
        archived_at: if archived { Some(now) } else { None },
        is_starred: starred,
        starred_at: if starred { Some(now) } else { None },
        created_at: now,
        updated_at: now,
        mimetype: Some(mime_type),
        language,
        reading_time,
        domain_name: domain_name.unwrap_or("").to_string(),
        preview_picture: preview_picture.map(|u| u.to_string()),
        origin_url: request.origin_url.map(|u| u.to_string()),
        published_at: published_at.map(|v| v.timestamp()),
        published_by: request.authors.map(|a| a.join(",")),
        is_public: request.public,
        uid: request.public.filter(|p| *p).map(|_b| generate_uid()),
    };

    let tag_to_create_tag = |label: String| -> tags::CreateTag {
        tags::CreateTag {
            user_id: user_info.user_id,
            slug: slugify(&label),
            label,
        }
    };

    let create_tags = request
        .tags
        .map(|_tags| {
            _tags
                .into_iter()
                .map(tag_to_create_tag)
                .collect::<Vec<tags::CreateTag>>()
        })
        // TODO if it is not new entry - we will force empty tags. It should be fixed when this method will support not only entry creations
        .unwrap_or(vec![]);

    let mut tx = tctx.tx()?;
    let (entry_row, tag_rows) = entries::create(&mut tx, create_entry, &create_tags).await?;

    let tags = tag_rows.into_iter().map(Tag::from).collect();

    // TODO replace by real url
    #[allow(clippy::redundant_closure)] // Or Location of error will be not correct
    let self_url = Url::parse("https://example.com").map_err(|e| Into::<ArticlerError>::into(e))?;

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

pub async fn entries(
    tctx: web::ReqData<TransactionContext<'_>>,
    request: Query<EntriesRequest>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Entries>> {
    let request = request.into_inner();

    let params = entries::EntriesCriteria {
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

    let mut tx = tctx.tx()?;

    let count_without_paging = entries::count(&mut tx, &params).await?;

    let pages = (count_without_paging as f64 / request.per_page as f64).ceil() as i64;

    if request.page > pages {
        return Err(ErrorNotFound("Not found"));
    }

    // TODO implement all needed request filters and etc
    let entries = entries::find_all(&mut tx, &params).await?;

    let mut ents = vec![];

    for (e, tags) in entries {
        let mapped_tags: Vec<Tag> = tags.into_iter().map(|tr| tr.into()).collect();
        ents.push(Entry::try_from((e, mapped_tags))?);
    }

    // TODO implement actual urls generating
    #[allow(clippy::redundant_closure)] // Or Location of error will be not correct
    let url = Url::parse("http://example.com").map_err(|e| Into::<ArticlerError>::into(e))?;

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

pub async fn get_tags_by_entry(
    tctx: web::ReqData<TransactionContext<'_>>,
    entry_id: web::Path<Id>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Vec<Tag>>> {
    let entry_id = entry_id.into_inner();

    let mut tx = tctx.tx()?;

    if entries::exists_by_id(&mut tx, user_info.user_id, entry_id).await? {
        let result = tags::find_by_entry_id(&mut tx, user_info.user_id, entry_id)
            .await?
            .into_iter()
            .map(|tr| tr.into())
            .collect();

        Ok(Json(result))
    } else {
        Err(ErrorNotFound("Entry not found"))
    }
}

pub async fn delete_tag_from_entry(
    tctx: web::ReqData<TransactionContext<'_>>,
    ids: web::Path<(Id, Id)>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Entry>> {
    let (entry_id, tag_id) = ids.into_inner();

    let mut tx = tctx.tx()?;

    if entries::exists_by_id(&mut tx, user_info.user_id, entry_id).await? {
        entries::delete_tag_by_tag_id(&mut tx, user_info.user_id, entry_id, tag_id).await?;

        if let Some((entry_row, tag_rows)) =
            entries::find_by_id(&mut tx, user_info.user_id, entry_id).await?
        {
            let tags = tag_rows.into_iter().map(|tr| tr.into()).collect();

            Ok(Json(Entry::try_from((entry_row, tags))?))
        } else {
            // Due to transactions - entry couldn't be deleted here
            Err(ErrorInternalServerError("Unknown error"))
        }
    } else {
        Err(ErrorNotFound("Entry not found"))
    }
}

pub async fn delete_entry(
    tctx: web::ReqData<TransactionContext<'_>>,
    entry_id: web::Path<i64>,
    request: Query<DeleteEntryRequest>,
    user_info: UserInfo,
) -> actix_web::Result<Json<DeleteEntryResponse>> {
    let request = request.into_inner();
    let entry_id = entry_id.into_inner();

    let mut tx = tctx.tx()?;

    match request.expect {
        Expect::Id => {
            let deleted = entries::delete_by_id(&mut tx, user_info.user_id, entry_id).await?;

            if !deleted {
                return Err(ErrorNotFound("Entry not found"));
            }

            Ok(Json(DeleteEntryResponse::Id { id: entry_id }))
        }
        Expect::Full => {
            let full_entry = entries::find_by_id(&mut tx, user_info.user_id, entry_id).await?;

            let (entry_row, tag_rows) =
                full_entry.ok_or_else(|| ErrorNotFound("Entry not found"))?;

            let deleted = entries::delete_by_id(&mut tx, user_info.user_id, entry_id).await?;

            if !deleted {
                return Err(ErrorNotFound("Entry not found"));
            }

            let tags: Vec<Tag> = tag_rows.into_iter().map(|tr| tr.into()).collect();
            let entry = Entry::try_from((entry_row, tags))?;

            Ok(Json(DeleteEntryResponse::Full {
                entry: Box::new(entry),
            }))
        }
    }
}

pub async fn post_entry_tags(
    tctx: web::ReqData<TransactionContext<'_>>,
    entry_id: web::Path<Id>,
    request: web::Form<EntryTags>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Entry>> {
    let entry_id = entry_id.into_inner();

    let mut tx = tctx.tx()?;

    // TODO dirty design - looks like we need entry repository method for it
    if entries::find_by_id(&mut tx, user_info.user_id, entry_id)
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

        tags::update_tags_by_entry_id(&mut tx, user_info.user_id, entry_id, &full_tags).await?;

        let (entry_row, tag_rows) = entries::find_by_id(&mut tx, user_info.user_id, entry_id)
            .await?
            .ok_or(ErrorNotFound("Entry not found"))?;

        let entry_tags = tag_rows.into_iter().map(Tag::from).collect();

        Ok(Json(Entry::try_from((entry_row, entry_tags))?))
    } else {
        Err(ErrorNotFound("Entry not found"))
    }
}

pub async fn patch_entry_json(
    tctx: web::ReqData<TransactionContext<'_>>,
    entry_id: web::Path<i64>,
    request: web::Json<UpdateEntry>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Entry>> {
    do_patch_entry(tctx, entry_id, request.into_inner(), user_info).await
}

pub async fn patch_entry_form(
    tctx: web::ReqData<TransactionContext<'_>>,
    entry_id: web::Path<i64>,
    request: web::Form<UpdateEntry>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Entry>> {
    do_patch_entry(tctx, entry_id, request.into_inner(), user_info).await
}

async fn do_patch_entry(
    tctx: web::ReqData<TransactionContext<'_>>,
    entry_id: web::Path<i64>,
    request: UpdateEntry,
    user_info: UserInfo,
) -> actix_web::Result<Json<Entry>> {
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

    let mut tx = tctx.tx()?;

    let updated = entries::update_by_id(&mut tx, user_info.user_id, entry_id, repo_update).await?;

    if !updated {
        return Err(ErrorNotFound("Entry not found"));
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

        tags::update_tags_by_entry_id(&mut tx, user_info.user_id, entry_id, &full_tags).await?;
    };

    let (entry_row, tag_rows) = entries::find_by_id(&mut tx, user_info.user_id, entry_id)
        .await?
        .ok_or_else(|| ErrorNotFound("Entry not found"))?;

    let entry_tags = tag_rows.into_iter().map(|t| t.into()).collect();

    let entry = Entry::try_from((entry_row, entry_tags))?;

    Ok(Json(entry))
}

#[derive(Default, Deserialize, Debug, PartialEq, Clone, Copy)]
enum Expect {
    #[default]
    #[serde(rename(deserialize = "id"))]
    Id,
    #[serde(rename(deserialize = "full"))]
    Full,
}

#[derive(Deserialize, Debug)]
pub struct DeleteEntryRequest {
    #[serde(default)]
    expect: Expect,
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
    labels: Vec<String>,
}

#[derive(Serialize)]
pub struct AddEntryResponse {
    #[serde(flatten)]
    entry: Entry,
    _links: Links,
}

#[derive(Serialize)]
pub struct Embedded {
    items: Vec<Entry>,
}

#[derive(Serialize)]
pub struct Entries {
    page: i64,
    limit: i64,
    pages: i64,
    total: i64,
    #[serde(rename(serialize = "_embedded"))]
    embedded: Embedded,
    _links: Links,
}

fn try_parse_url(s: Option<String>) -> ArticlerResult<Option<Url>> {
    Ok(s.map(|u| Url::parse(&u)).transpose()?)
}

// TODO ugly place for errors - api module is overwhelmed with logic already
#[derive(Error, Debug)]
enum HandlerError {
    #[error("Date from seconds convert error")]
    DateFromError,
}

fn try_parse_timestamp_opt(s: Option<i64>) -> ArticlerResult<Option<DateTime<Utc>>> {
    match s {
        Some(t) => match DateTime::from_timestamp_secs(t) {
            Some(r) => Ok(Some(r)),
            None => Err(HandlerError::DateFromError.into()),
        },
        None => Ok(None),
    }
}

fn try_parse_timestamp(s: i64) -> ArticlerResult<DateTime<Utc>> {
    match DateTime::from_timestamp_secs(s) {
        Some(r) => Ok(r),
        None => Err(HandlerError::DateFromError.into()),
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
    type Error = ArticlerError;

    fn try_from((e, tags): (entries::EntryRow, Vec<Tag>)) -> Result<Self, Self::Error> {
        Ok(Entry {
            id: e.id,
            url: Url::parse(&e.url)?,
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
            mimetype: e.mimetype,
            language: e.language,
            reading_time: e.reading_time,
            domain_name: e.domain_name,
            preview_picture: try_parse_url(e.preview_picture)?,
            origin_url: try_parse_url(e.origin_url)?,
            published_at: try_parse_timestamp_opt(e.published_at)?,
            // TODO this .map(to_string) look ugly
            published_by: e
                .published_by
                .map(|s| s.split(",").map(|s| s.to_string()).collect()),
            is_public: e.is_public,
            uid: e.uid,
        })
    }
}

#[derive(Default, Deserialize, Debug, PartialEq, Clone, Copy)]
enum FindSortEnum {
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
enum FindSortOrder {
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
enum Detail {
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
    // If not set - title will be retrieved by scraping
    title: Option<String>,
    // If not set - content will be retrieved by scraping
    content: Option<String>,
    #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
    tags: Option<Vec<String>>,
    #[serde_as(as = "Option<BoolFromInt>")]
    archive: Option<bool>,
    #[serde_as(as = "Option<BoolFromInt>")]
    starred: Option<bool>,
    // Will be set as given url for the entry
    // If there will be some redirects, result url will be set as entry url
    url: Url,
    language: Option<String>,
    published_at: Option<DateTime<Utc>>,
    preview_picture: Option<Url>,
    #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
    authors: Option<Vec<String>>,
    // Generate public link for the url or not
    #[serde_as(as = "Option<BoolFromInt>")]
    public: Option<bool>,
    // Origin url for the entry (from where user found it).
    origin_url: Option<String>,
}

#[serde_as]
#[derive(Deserialize, PartialEq, Debug)]
pub struct UpdateEntry {
    title: Option<String>,
    content: Option<String>,
    #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
    tags: Option<Vec<String>>,
    #[serde_as(as = "Option<BoolFromInt>")]
    archive: Option<bool>,
    #[serde_as(as = "Option<BoolFromInt>")]
    starred: Option<bool>,
    language: Option<String>,
    published_at: Option<DateTime<Utc>>,
    preview_picture: Option<Url>,
    #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
    authors: Option<Vec<String>>,
    #[serde_as(as = "Option<BoolFromInt>")]
    public: Option<bool>,
    origin_url: Option<String>,
}

#[serde_as]
#[derive(Deserialize, Debug, PartialEq)]
pub struct EntriesRequest {
    #[serde_as(as = "Option<BoolFromInt>")]
    archive: Option<bool>,
    #[serde_as(as = "Option<BoolFromInt>")]
    starred: Option<bool>,
    #[serde(default)]
    sort: FindSortEnum,
    #[serde(default)]
    order: FindSortOrder,
    #[serde(default = "default_page")]
    page: i64,
    #[serde(rename(deserialize = "perPage"))]
    #[serde(default = "default_per_page")]
    per_page: i64,
    #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
    tags: Option<Vec<String>>,
    #[serde(default)]
    since: i64,
    #[serde_as(as = "Option<BoolFromInt>")]
    public: Option<bool>,
    #[serde(default)]
    detail: Detail,
    domain_name: Option<String>,
}

#[derive(Serialize)]
pub struct Links {
    #[serde(rename(serialize = "self"))]
    _self: Link,
    #[serde(skip_serializing_if = "Option::is_none")]
    first: Option<Link>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last: Option<Link>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next: Option<Link>,
}

#[derive(Serialize)]
struct Link {
    href: Url,
}

#[derive(Debug, Serialize)]
pub struct Exists {
    exists: bool,
}
