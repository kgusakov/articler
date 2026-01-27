use super::oauth::UserInfo;
use crate::{
    app::AppState,
    helpers::{generate_uid, hash_str},
    models::{Entry, Tag},
    repository::{entries, tags},
};
use actix_utils::future::{Ready, ready};
use actix_web::web::{ServiceConfig, delete, get, patch, post};
use actix_web::{
    Error, HttpMessage,
    error::{ErrorInternalServerError, ErrorNotFound},
    web::{self, Json, Query},
};
use actix_web::{FromRequest, guard};
use actix_web_httpauth::middleware::HttpAuthentication;
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::BoolFromInt;
use serde_with::StringWithSeparator;
use serde_with::formats::CommaSeparator;
use serde_with::serde_as;
use slug::slugify;
use std::str::FromStr;
use url::{ParseError, Url};

type Id = i64;

const VERSION: &str = "2.6.12";

pub fn routes(cfg: &mut ServiceConfig) {
    let oauth = HttpAuthentication::with_fn(super::oauth::auth_extractor);

    cfg.route("/api/version.json", get().to(version))
        .route("/api/version", get().to(version));

    cfg.service(
        web::scope("/api")
            .wrap(oauth)
            .route(
                "/entries.json",
                web::route()
                    .guard(guard::Post())
                    .guard(guard::Header(
                        "content-type",
                        "application/x-www-form-urlencoded",
                    ))
                    .to(post_entries),
            )
            .route(
                "/entries.json",
                web::route()
                    .guard(guard::Post())
                    .guard(guard::Header("content-type", "application/json"))
                    .to(post_entries_json),
            )
            .route(
                "/entries",
                web::route()
                    .guard(guard::Post())
                    .guard(guard::Header(
                        "content-type",
                        "application/x-www-form-urlencoded",
                    ))
                    .to(post_entries),
            )
            .route(
                "/entries",
                web::route()
                    .guard(guard::Post())
                    .guard(guard::Header("content-type", "application/json"))
                    .to(post_entries_json),
            )
            .route("/entries.json", get().to(entries))
            .route("/entries", get().to(entries))
            .service(
                web::scope("/entries")
                    .route("/exists.json", get().to(exists))
                    .route("/exists", get().to(exists))
                    .route("/{entry_id}.json", delete().to(delete_entry))
                    .route("/{entry_id}", delete().to(delete_entry))
                    .route("/{entry_id}.json", patch().to(patch_entry))
                    .route("/{entry_id}", patch().to(patch_entry))
                    .route("/{entry_id}/tags", get().to(get_tags_by_entry))
                    .route("/{entry_id}/tags.json", post().to(post_entry_tags))
                    .route("/{entry_id}/tags", post().to(post_entry_tags))
                    .route(
                        "/{entry_id}/tags/{tag_id}.json",
                        delete().to(delete_tag_from_entry),
                    )
                    .route(
                        "/{entry_id}/tags/{tag_id}",
                        delete().to(delete_tag_from_entry),
                    ),
            )
            .route("/tags.json", get().to(get_tags))
            .route("/tags", get().to(get_tags))
            .service(
                web::scope("/tags")
                    .route("/label.json", delete().to(delete_tags_by_label))
                    .route("/label", delete().to(delete_tags_by_label))
                    .route("/{tag_id}.json", delete().to(delete_tag_by_id))
                    .route("/{tag_id}", delete().to(delete_tag_by_id)),
            )
            .service(
                web::scope("/tag")
                    .route("/label.json", delete().to(delete_tag_by_label))
                    .route("/label", delete().to(delete_tag_by_label)),
            ),
    );
}

#[derive(Debug, Serialize)]
struct Exists {
    exists: bool,
}

// TODO current implementation needed only for mobile app healthchecks. Needed full implementation
async fn exists() -> actix_web::Result<Json<Exists>> {
    Ok(Json(Exists { exists: false }))
}

async fn version() -> actix_web::Result<Json<String>> {
    Ok(Json(VERSION.to_string()))
}

async fn post_entries(
    data: web::Data<AppState>,
    request: web::Form<AddEntry>,
    user_info: UserInfo,
) -> actix_web::Result<Json<AddEntryResponse>> {
    do_post_entries(data, request.into_inner(), user_info).await
}

async fn post_entries_json(
    data: web::Data<AppState>,
    request: web::Json<AddEntry>,
    user_info: UserInfo,
) -> actix_web::Result<Json<AddEntryResponse>> {
    do_post_entries(data, request.into_inner(), user_info).await
}

async fn do_post_entries(
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
            let document = data
                .scraper
                .extract(&request.url)
                .await
                .map_err(ErrorInternalServerError)?;

            (
                document.title,
                document.content_html,
                document.mime_type.unwrap_or("".to_string()),
                document.published_at,
                document.language,
                document.image_url,
            )
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

    let mut tx = data.pool.begin().await.map_err(ErrorInternalServerError)?;
    let (entry_row, tag_rows) = entries::create(&mut tx, create_entry, &create_tags)
        .await
        .map_err(ErrorInternalServerError)?;
    tx.commit().await.map_err(ErrorInternalServerError)?;

    let tags = tag_rows.into_iter().map(Tag::from).collect();

    // TODO replace by real url
    let self_url = Url::parse("https://example.com").map_err(ErrorInternalServerError)?;

    Ok(web::Json(AddEntryResponse {
        entry: Entry::try_from((entry_row, tags)).map_err(ErrorInternalServerError)?,
        _links: Links {
            _self: Link { href: self_url },
            first: None,
            last: None,
            next: None,
        },
    }))
}

async fn entries(
    data: web::Data<AppState>,
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

    let mut tx = data.pool.begin().await.map_err(ErrorInternalServerError)?;

    let count_without_paging = entries::count(&mut tx, &params)
        .await
        .map_err(ErrorInternalServerError)?;

    let pages = (count_without_paging as f64 / request.per_page as f64).ceil() as i64;

    if request.page > pages {
        return Err(ErrorNotFound("Not found"));
    }

    // TODO implement all needed request filters and etc
    let entries = entries::find_all(&mut tx, &params)
        .await
        .map_err(ErrorInternalServerError)?;

    tx.commit().await.map_err(ErrorInternalServerError)?;

    let mut ents = vec![];

    for (e, tags) in entries {
        let mapped_tags: Vec<Tag> = tags.into_iter().map(|tr| tr.into()).collect();
        ents.push(Entry::try_from((e, mapped_tags)).map_err(ErrorInternalServerError)?);
    }

    // TODO implement actual urls generating
    let url = Url::from_str("http://example.com").unwrap();

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

async fn get_tags_by_entry(
    data: web::Data<AppState>,
    entry_id: web::Path<Id>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Vec<Tag>>> {
    let entry_id = entry_id.into_inner();

    let mut tx = data.pool.begin().await.map_err(ErrorInternalServerError)?;

    if entries::exists_by_id(&mut tx, user_info.user_id, entry_id)
        .await
        .map_err(ErrorInternalServerError)?
    {
        let result = tags::find_by_entry_id(&mut tx, user_info.user_id, entry_id)
            .await
            .map_err(ErrorInternalServerError)?
            .into_iter()
            .map(|tr| tr.into())
            .collect();

        tx.commit().await.map_err(ErrorInternalServerError)?;

        Ok(Json(result))
    } else {
        Err(ErrorNotFound("Entry not found"))
    }
}

async fn get_tags(
    data: web::Data<AppState>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Vec<Tag>>> {
    let mut tx = data.pool.begin().await.map_err(ErrorInternalServerError)?;

    let result = tags::get_all(&mut tx, user_info.user_id)
        .await
        .map_err(ErrorInternalServerError)?
        .into_iter()
        .map(|tr| tr.into())
        .collect();

    tx.commit().await.map_err(ErrorInternalServerError)?;

    Ok(Json(result))
}

#[derive(Deserialize)]
struct TagLabel {
    #[serde(rename(deserialize = "tag"))]
    label: String,
}

async fn delete_tag_by_id(
    data: web::Data<AppState>,
    tag_id: web::Path<Id>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Tag>> {
    let mut tx = data.pool.begin().await.map_err(ErrorInternalServerError)?;

    let result = tags::delete_by_id(&mut tx, user_info.user_id, tag_id.into_inner())
        .await
        .map_err(ErrorInternalServerError)?
        .map(|tr| tr.into());

    tx.commit().await.map_err(ErrorInternalServerError)?;

    if let Some(delete_tag) = result {
        Ok(Json(delete_tag))
    } else {
        Err(ErrorNotFound("Tag not found"))
    }
}

async fn delete_tag_by_label(
    data: web::Data<AppState>,
    label: web::Query<TagLabel>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Tag>> {
    let mut tx = data.pool.begin().await.map_err(ErrorInternalServerError)?;

    let result = tags::delete_by_label(&mut tx, user_info.user_id, &label.label)
        .await
        .map_err(ErrorInternalServerError)?
        .map(|tr| tr.into());

    tx.commit().await.map_err(ErrorInternalServerError)?;

    if let Some(delete_tag) = result {
        Ok(Json(delete_tag))
    } else {
        Err(ErrorNotFound("Tag not found"))
    }
}

#[serde_as]
#[derive(Deserialize)]
struct TagsLabel {
    #[serde(rename(deserialize = "tags"))]
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, String>")]
    labels: Vec<String>,
}

async fn delete_tags_by_label(
    data: web::Data<AppState>,
    label: web::Query<TagsLabel>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Vec<Tag>>> {
    let mut tx = data.pool.begin().await.map_err(ErrorInternalServerError)?;

    let result = tags::delete_all_by_label(&mut tx, user_info.user_id, &label.labels)
        .await
        .map_err(ErrorInternalServerError)?
        .into_iter()
        .map(|tr| tr.into())
        .collect();

    tx.commit().await.map_err(ErrorInternalServerError)?;

    Ok(Json(result))
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
struct DeleteEntryRequest {
    #[serde(default)]
    expect: Expect,
}

#[derive(Serialize)]
#[serde(untagged)]
enum DeleteEntryResponse {
    Id {
        id: i64,
    },
    Full {
        #[serde(flatten)]
        entry: Box<Entry>,
    },
}

async fn delete_tag_from_entry(
    data: web::Data<AppState>,
    ids: web::Path<(Id, Id)>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Entry>> {
    let (entry_id, tag_id) = ids.into_inner();

    let mut tx = data.pool.begin().await.map_err(ErrorInternalServerError)?;

    if entries::exists_by_id(&mut tx, user_info.user_id, entry_id)
        .await
        .map_err(ErrorInternalServerError)?
    {
        entries::delete_tag_by_tag_id(&mut tx, user_info.user_id, entry_id, tag_id)
            .await
            .map_err(ErrorInternalServerError)?;

        if let Some((entry_row, tag_rows)) =
            entries::find_by_id(&mut tx, user_info.user_id, entry_id)
                .await
                .map_err(ErrorInternalServerError)?
        {
            tx.commit().await.map_err(ErrorInternalServerError)?;

            let tags = tag_rows.into_iter().map(|tr| tr.into()).collect();

            Ok(Json(
                Entry::try_from((entry_row, tags)).map_err(ErrorInternalServerError)?,
            ))
        } else {
            // TODO needed while transactions is not implemented
            Err(ErrorNotFound("Entry not found"))
        }
    } else {
        Err(ErrorNotFound("Entry not found"))
    }
}

async fn delete_entry(
    data: web::Data<AppState>,
    entry_id: web::Path<i64>,
    request: Query<DeleteEntryRequest>,
    user_info: UserInfo,
) -> actix_web::Result<Json<DeleteEntryResponse>> {
    let request = request.into_inner();
    let entry_id = entry_id.into_inner();

    match request.expect {
        Expect::Id => {
            let mut tx = data.pool.begin().await.map_err(ErrorInternalServerError)?;

            let deleted = entries::delete_by_id(&mut tx, user_info.user_id, entry_id)
                .await
                .map_err(ErrorInternalServerError)?;

            if !deleted {
                return Err(ErrorNotFound("Entry not found"));
            }

            tx.commit().await.map_err(ErrorInternalServerError)?;

            Ok(Json(DeleteEntryResponse::Id { id: entry_id }))
        }
        Expect::Full => {
            let mut tx = data.pool.begin().await.map_err(ErrorInternalServerError)?;

            let full_entry = entries::find_by_id(&mut tx, user_info.user_id, entry_id)
                .await
                .map_err(ErrorInternalServerError)?;

            let (entry_row, tag_rows) =
                full_entry.ok_or_else(|| ErrorNotFound("Entry not found"))?;

            let deleted = entries::delete_by_id(&mut tx, user_info.user_id, entry_id)
                .await
                .map_err(ErrorInternalServerError)?;

            if !deleted {
                return Err(ErrorNotFound("Entry not found"));
            }

            tx.commit().await.map_err(ErrorInternalServerError)?;

            let tags: Vec<Tag> = tag_rows.into_iter().map(|tr| tr.into()).collect();
            let entry = Entry::try_from((entry_row, tags)).map_err(ErrorInternalServerError)?;

            Ok(Json(DeleteEntryResponse::Full {
                entry: Box::new(entry),
            }))
        }
    }
}

#[serde_as]
#[derive(Deserialize)]
struct EntryTags {
    #[serde(rename(deserialize = "tags"))]
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, String>")]
    labels: Vec<String>,
}

async fn post_entry_tags(
    data: web::Data<AppState>,
    entry_id: web::Path<Id>,
    request: web::Form<EntryTags>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Entry>> {
    let entry_id = entry_id.into_inner();

    let mut tx = data.pool.begin().await.map_err(ErrorInternalServerError)?;

    // TODO dirty design - looks like we need entry repository method for it
    if entries::find_by_id(&mut tx, user_info.user_id, entry_id)
        .await
        .map_err(ErrorInternalServerError)?
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

        tags::update_tags_by_entry_id(&mut tx, user_info.user_id, entry_id, &full_tags)
            .await
            .map_err(ErrorInternalServerError)?;

        let (entry_row, tag_rows) = entries::find_by_id(&mut tx, user_info.user_id, entry_id)
            .await
            .map_err(ErrorInternalServerError)?
            .ok_or(ErrorNotFound("Entry not found"))?;

        tx.commit().await.map_err(ErrorInternalServerError)?;

        let entry_tags = tag_rows.into_iter().map(Tag::from).collect();

        Ok(Json(
            Entry::try_from((entry_row, entry_tags)).map_err(ErrorInternalServerError)?,
        ))
    } else {
        Err(ErrorNotFound("Entry not found"))
    }
}

async fn patch_entry(
    data: web::Data<AppState>,
    entry_id: web::Path<i64>,
    request: web::Form<UpdateEntry>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Entry>> {
    let entry_id = entry_id.into_inner();
    let request = request.into_inner();

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

    let mut tx = data.pool.begin().await.map_err(ErrorInternalServerError)?;

    let updated = entries::update_by_id(&mut tx, user_info.user_id, entry_id, repo_update)
        .await
        .map_err(ErrorInternalServerError)?;

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

        tags::update_tags_by_entry_id(&mut tx, user_info.user_id, entry_id, &full_tags)
            .await
            .map_err(ErrorInternalServerError)?;
    };

    let (entry_row, tag_rows) = entries::find_by_id(&mut tx, user_info.user_id, entry_id)
        .await
        .map_err(ErrorInternalServerError)?
        .ok_or_else(|| ErrorNotFound("Entry not found"))?;

    tx.commit().await.map_err(ErrorInternalServerError)?;

    let entry_tags = tag_rows.into_iter().map(|t| t.into()).collect();

    let entry = Entry::try_from((entry_row, entry_tags)).map_err(ErrorInternalServerError)?;

    Ok(Json(entry))
}

#[derive(Serialize)]
struct AddEntryResponse {
    #[serde(flatten)]
    entry: Entry,
    _links: Links,
}

#[derive(Serialize)]
struct Embedded {
    items: Vec<Entry>,
}

#[derive(Serialize)]
struct Entries {
    page: i64,
    limit: i64,
    pages: i64,
    total: i64,
    #[serde(rename(serialize = "_embedded"))]
    embedded: Embedded,
    _links: Links,
}

fn try_parse_url(s: Option<String>) -> Result<Option<Url>, ParseError> {
    s.map(|u| Url::parse(&u)).transpose()
}

fn try_parse_timestamp_opt(s: Option<i64>) -> anyhow::Result<Option<DateTime<Utc>>> {
    match s {
        Some(t) => match DateTime::from_timestamp_secs(t) {
            Some(r) => Ok(Some(r)),
            None => Err(anyhow!("Can't parse timestamp")),
        },
        None => Ok(None),
    }
}

fn try_parse_timestamp(s: i64) -> anyhow::Result<DateTime<Utc>> {
    match DateTime::from_timestamp_secs(s) {
        Some(r) => Ok(r),
        None => Err(anyhow!("Can't parse timestamp")),
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
    type Error = anyhow::Error;

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
struct AddEntry {
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
struct UpdateEntry {
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
struct EntriesRequest {
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
struct Links {
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

impl FromRequest for UserInfo {
    type Error = Error;

    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(
        req: &actix_web::HttpRequest,
        _payload: &mut actix_http::Payload,
    ) -> Self::Future {
        if let Some(user_info) = req.extensions().get::<UserInfo>() {
            ready(Ok(user_info.clone()))
        } else {
            ready(Err(actix_web::error::ErrorUnauthorized("No user info")))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::http::api::{Detail, EntriesRequest, FindSortEnum, FindSortOrder};

    #[test]
    fn test() {
        assert_eq!(
            EntriesRequest {
                archive: Some(true),
                starred: Some(false),
                sort: FindSortEnum::Created,
                order: FindSortOrder::Asc,
                page: 0,
                per_page: 10,
                tags: Some(vec!["api".to_string(), "rest".to_string()]),
                since: 0,
                public: Some(true),
                detail: Detail::Full,
                domain_name: Some("example.com".to_string())
            },
            serde_json::from_str::<EntriesRequest>(
                r#"{
                    "archive":1,
                    "starred":0,
                    "sort":"created",
                    "order":"asc",
                    "page":0,
                    "perPage":10,
                    "tags":"api,rest",
                    "since":0,
                    "public":1,
                    "detail":"full",
                    "domain_name":"example.com"
                    }"#,
            )
            .unwrap()
        );
    }
}
