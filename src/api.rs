use crate::{
    helpers::{generate_uid, hash_str},
    models::{Entry, Tag},
    storage::repository::{
        self, CreateEntry, CreateTag, EntriesCriteria, EntryRepository, EntryRow, SortColumn,
        SortOrder, SqliteEntryRepository, SqliteTagRepository, TagRepository, TagRow,
        UpdateEntry as RepositoryUpdateEntry,
    },
};
use actix_web::{
    App, HttpServer, delete,
    dev::Server,
    error::{ErrorInternalServerError, ErrorNotFound},
    get, patch, post, routes,
    web::{self, Json, Query},
};
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_with::BoolFromInt;
use serde_with::StringWithSeparator;
use serde_with::formats::CommaSeparator;
use serde_with::serde_as;
use slug::slugify;
use sqlx::{Pool, Sqlite};
use std::{str::FromStr, sync::Arc};
use url::{ParseError, Url};

type Id = i64;

// TODO post with the same url is not supported
#[post("/api/entries.json")]
pub async fn post_entries(
    data: web::Data<AppState>,
    request: web::Form<AddEntry>,
) -> actix_web::Result<Json<AddEntryResponse>> {
    //
    // Check if url already exist:
    //    - if exist update the current entry
    //    - if not - create new one
    //
    // In both case if title and/or content is not set - both will be retrieved from the internet again
    let request = request.into_inner();

    // TODO must be received by scrapper
    let title = "Title";
    // TODO must be received by scrapper
    let content = "Content";
    // TODO must be received by scrapper
    let mimetype = "text/html";
    // TODO must be calculated
    let reading_time = 0;
    // TODO url without domain must be handled in an appropriate way
    let domain_name = request.url.domain().unwrap_or("");

    let now = Utc::now().timestamp();

    let archived = request.archive.unwrap_or(false);
    let starred = request.starred.unwrap_or(false);

    // TODO can we remove all these ugly to_string?
    let create_entry = CreateEntry {
        // TODO actually here we must have url without redirects already
        url: request.url.to_string(),
        hashed_url: hash_str(request.url.as_str()),
        given_url: request.url.to_string(),
        hashed_given_url: hash_str(request.url.as_str()),
        title: request.title.unwrap_or(title.to_string()),
        content: request.content.unwrap_or(content.to_string()),
        is_archived: archived,
        archived_at: if archived { Some(now) } else { None },
        is_starred: starred,
        starred_at: if starred { Some(now) } else { None },
        created_at: now,
        updated_at: now,
        mimetype: Some(mimetype.to_string()),
        language: request.language,
        reading_time: reading_time,
        domain_name: domain_name.to_string(),
        preview_picture: request.preview_picture.map(|u| u.to_string()),
        origin_url: request.origin_url.map(|u| u.to_string()),
        published_at: request.published_at.map(|v| v.timestamp()),
        published_by: request.authors.map(|a| a.join(",")),
        is_public: request.public,
        uid: request.public.filter(|p| *p).map(|_b| generate_uid()),
    };

    let tag_to_create_tag = |label: String| -> CreateTag {
        CreateTag {
            label: label.clone(),
            slug: slugify(label),
        }
    };

    let create_tags = request
        .tags
        .map(|_tags| {
            _tags
                .into_iter()
                .map(tag_to_create_tag)
                .collect::<Vec<CreateTag>>()
        })
        // TODO if it is not new entry - we will force empty tags. It should be fixed when this method will support not only entry creations
        .unwrap_or(vec![]);

    // TODO for create
    let (entry_row, tag_rows) = data
        .entry_repository
        .create(create_entry, &create_tags)
        .await
        .map_err(ErrorInternalServerError)?;

    let tags = tag_rows.into_iter().map(|tr| Tag::from(tr)).collect();

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

// TODO /api pref should be moved as a base prefix
#[routes]
#[get("/api/entries")]
#[get("/api/entries.json")]
pub async fn entries(
    data: web::Data<AppState>,
    request: Query<EntriesRequest>,
) -> actix_web::Result<Json<Entries>> {
    let request = request.into_inner();

    let params = EntriesCriteria {
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
    // TODO implement all needed request filters and etc
    let entries = data
        .entry_repository
        .find_all(&params)
        .await
        .map_err(ErrorInternalServerError)?;

    let mut ents = vec![];

    for (e, tags) in entries {
        let mapped_tags: Vec<Tag> = tags.into_iter().map(|tr| tr.into()).collect();
        ents.push(Entry::try_from((e, mapped_tags)).map_err(ErrorInternalServerError)?);
    }

    // TODO implement actual urls generating
    let url = Url::from_str("http://example.com").unwrap();

    let count_without_paging = data
        .entry_repository
        .count(&params)
        .await
        .map_err(ErrorInternalServerError)?;

    Ok(web::Json(Entries {
        page: request.page,
        limit: request.per_page,
        pages: (count_without_paging as f64 / request.per_page as f64).ceil() as i64,
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

#[get("/api/entries/{entry_id}/tags")]
pub async fn get_tags_by_entry(
    data: web::Data<AppState>,
    entry_id: web::Path<Id>,
) -> actix_web::Result<Json<Vec<Tag>>> {
    let entry_id = entry_id.into_inner();

    if data
        .entry_repository
        .exists_by_id(entry_id)
        .await
        .map_err(ErrorInternalServerError)?
    {
        Ok(Json(
            data.tag_repository
                .find_by_entry_id(entry_id)
                .await
                .map_err(ErrorInternalServerError)?
                .into_iter()
                .map(|tr| tr.into())
                .collect(),
        ))
    } else {
        Err(ErrorNotFound("Entry not found"))
    }
}

#[routes]
#[get("/api/tags.json")]
#[get("/api/tags")]
pub async fn get_tags(data: web::Data<AppState>) -> actix_web::Result<Json<Vec<Tag>>> {
    Ok(Json(
        data.tag_repository
            .get_all()
            .await
            .map_err(ErrorInternalServerError)?
            .into_iter()
            .map(|tr| tr.into())
            .collect(),
    ))
}

#[derive(Deserialize)]
struct TagLabel {
    #[serde(rename(deserialize = "tag"))]
    label: String,
}

#[routes]
#[delete("/api/tags/label.json")]
#[delete("/api/tags/label")]
pub async fn delete_tag_by_label(
    data: web::Data<AppState>,
    label: web::Query<TagLabel>,
) -> actix_web::Result<Json<Tag>> {
    let result = data
        .tag_repository
        .delete_by_label(&label.label)
        .await
        .map_err(ErrorInternalServerError)?
        .map(|tr| tr.into());
    if let Some(delete_tag) = result {
        Ok(Json(delete_tag))
    } else {
        Err(ErrorNotFound("Tag not found"))
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone, Copy)]
enum Expect {
    #[serde(rename(deserialize = "id"))]
    Id,
    #[serde(rename(deserialize = "full"))]
    Full,
}

impl Default for Expect {
    fn default() -> Self {
        Expect::Id
    }
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
        entry: Entry,
    },
}

#[routes]
#[delete("/api/entries/{entry_id}/tags/{tag_id}.json")]
#[delete("/api/entries/{entry_id}/tags/{tag_id}")]
pub async fn delete_tag_from_entry(
    data: web::Data<AppState>,
    ids: web::Path<(Id, Id)>,
) -> actix_web::Result<Json<Entry>> {
    let (entry_id, tag_id) = ids.into_inner();

    if data
        .entry_repository
        .exists_by_id(entry_id)
        .await
        .map_err(ErrorInternalServerError)?
    {
        data.entry_repository
            .delete_tag_by_tag_id(entry_id, tag_id)
            .await
            .map_err(ErrorInternalServerError)?;

        if let Some((entry_row, tag_rows)) = data
            .entry_repository
            .find_by_id(entry_id)
            .await
            .map_err(ErrorInternalServerError)?
        {
            let tags = tag_rows.into_iter().map(|tr| tr.into()).collect();

            Ok(Json(
                Entry::try_from((entry_row, tags)).map_err(ErrorInternalServerError)?,
            ))
        } else {
            // TODO needed while transactions is not implemented
            return Err(ErrorNotFound("Entry not found"));
        }
    } else {
        return Err(ErrorNotFound("Entry not found"));
    }
}

#[routes]
#[delete("/api/entries/{entry_id}.json")]
#[delete("/api/entries/{entry_id}")]
pub async fn delete_entry(
    data: web::Data<AppState>,
    entry_id: web::Path<i64>,
    request: Query<DeleteEntryRequest>,
) -> actix_web::Result<Json<DeleteEntryResponse>> {
    let request = request.into_inner();
    let entry_id = entry_id.into_inner();

    match request.expect {
        Expect::Id => {
            let deleted = data
                .entry_repository
                .delete_by_id(entry_id)
                .await
                .map_err(ErrorInternalServerError)?;

            if !deleted {
                return Err(ErrorNotFound("Entry not found"));
            }

            Ok(Json(DeleteEntryResponse::Id { id: entry_id }))
        }
        Expect::Full => {
            let full_entry = data
                .entry_repository
                .find_by_id(entry_id)
                .await
                .map_err(ErrorInternalServerError)?;

            let (entry_row, tag_rows) =
                full_entry.ok_or_else(|| ErrorNotFound("Entry not found"))?;

            let deleted = data
                .entry_repository
                .delete_by_id(entry_id)
                .await
                .map_err(ErrorInternalServerError)?;

            if !deleted {
                return Err(ErrorNotFound("Entry not found"));
            }

            let tags: Vec<Tag> = tag_rows.into_iter().map(|tr| tr.into()).collect();
            let entry = Entry::try_from((entry_row, tags)).map_err(ErrorInternalServerError)?;

            Ok(Json(DeleteEntryResponse::Full { entry }))
        }
    }
}

#[routes]
#[patch("/api/entries/{entry_id}.json")]
#[patch("/api/entries/{entry_id}")]
pub async fn patch_entry(
    data: web::Data<AppState>,
    entry_id: web::Path<i64>,
    request: Json<UpdateEntry>,
) -> actix_web::Result<Json<Entry>> {
    let entry_id = entry_id.into_inner();
    let request = request.into_inner();

    let now = Utc::now().timestamp();

    let repo_update = RepositoryUpdateEntry {
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

    let updated = data
        .entry_repository
        .update_by_id(entry_id, repo_update)
        .await
        .map_err(ErrorInternalServerError)?;

    if !updated {
        return Err(ErrorNotFound("Entry not found"));
    }

    if let Some(tags) = request.tags {
        let full_tags = tags
            .into_iter()
            .map(|l| CreateTag {
                // TODO remove clone
                label: l.clone(),
                slug: slugify(l),
            })
            .collect();

        data.tag_repository
            .update_tags_by_entry_id(entry_id, full_tags)
            .await
            .map_err(ErrorInternalServerError)?;
    };

    let (entry_row, tag_rows) = data
        .entry_repository
        .find_by_id(entry_id)
        .await
        .map_err(ErrorInternalServerError)?
        .ok_or_else(|| ErrorNotFound("Entry not found"))?;

    let entry_tags = tag_rows.into_iter().map(|t| t.into()).collect();

    let entry = Entry::try_from((entry_row, entry_tags)).map_err(ErrorInternalServerError)?;

    Ok(Json(entry))
}

pub struct AppState {
    pub tag_repository: Arc<dyn TagRepository>,
    pub entry_repository: Arc<dyn EntryRepository>,
}

pub fn app_state_init(pool: Arc<Pool<Sqlite>>) -> AppState {
    let tag_repo = Arc::new(SqliteTagRepository::new(pool.clone()));

    AppState {
        tag_repository: tag_repo.clone(),
        entry_repository: Arc::new(SqliteEntryRepository::new(pool.clone(), tag_repo.clone())),
    }
}

pub fn http_server(port: u16, app_state: AppState) -> std::io::Result<Server> {
    let app_data = web::Data::new(app_state);

    Ok(HttpServer::new(move || {
        App::new()
            .app_data(app_data.clone())
            .service(web::scope("/").service(entries))
            .service(web::scope("/").service(post_entries))
            .service(web::scope("/").service(patch_entry))
            .service(web::scope("/").service(delete_entry))
            .service(web::scope("/").service(get_tags_by_entry))
            .service(web::scope("/").service(get_tags))
            .service(web::scope("/").service(delete_tag_from_entry))
            .service(web::scope("/").service(delete_tag_by_label))
    })
    .bind(format!("0.0.0.0:{}", port))?
    .run())
}

#[derive(Serialize)]
pub struct AddEntryResponse {
    #[serde(flatten)]
    entry: Entry,
    _links: Links,
}

#[derive(Serialize)]
struct Embedded {
    items: Vec<Entry>,
}

#[derive(Serialize)]
pub struct Entries {
    page: i64,
    limit: i64,
    pages: i64,
    total: i64,
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

impl From<TagRow> for Tag {
    fn from(value: TagRow) -> Self {
        Self {
            id: value.id,
            label: value.label,
            slug: value.slug,
        }
    }
}

impl TryFrom<(EntryRow, Vec<Tag>)> for Entry {
    type Error = anyhow::Error;

    fn try_from((e, tags): (EntryRow, Vec<Tag>)) -> Result<Self, Self::Error> {
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
            tags: tags,
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

#[derive(Deserialize, Debug, PartialEq, Clone, Copy)]
enum FindSortEnum {
    #[serde(rename(deserialize = "created"))]
    Created,
    #[serde(rename(deserialize = "updated"))]
    Updated,
    #[serde(rename(deserialize = "archived"))]
    Archived,
}

impl Default for FindSortEnum {
    fn default() -> Self {
        FindSortEnum::Created
    }
}

impl Into<SortColumn> for FindSortEnum {
    fn into(self) -> SortColumn {
        match self {
            FindSortEnum::Created => SortColumn::Created,
            FindSortEnum::Updated => SortColumn::Updated,
            FindSortEnum::Archived => SortColumn::Archived,
        }
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone, Copy)]
enum FindSortOrder {
    #[serde(rename(deserialize = "asc"))]
    Asc,
    #[serde(rename(deserialize = "desc"))]
    Desc,
}

impl Default for FindSortOrder {
    fn default() -> Self {
        FindSortOrder::Asc
    }
}

impl Into<SortOrder> for FindSortOrder {
    fn into(self) -> SortOrder {
        match self {
            FindSortOrder::Asc => SortOrder::Asc,
            FindSortOrder::Desc => SortOrder::Desc,
        }
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone, Copy)]
enum Detail {
    #[serde(rename(deserialize = "metadata"))]
    Metadata,
    #[serde(rename(deserialize = "full"))]
    Full,
}

impl Default for Detail {
    fn default() -> Self {
        Detail::Full
    }
}

impl Into<repository::Detail> for Detail {
    fn into(self) -> repository::Detail {
        match self {
            Detail::Full => repository::Detail::Full,
            Detail::Metadata => repository::Detail::Metadata,
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
    // If not set - title will be retrieved by scrapping
    pub title: Option<String>,
    // If not set - content will be retrieved by scrapping
    pub content: Option<String>,
    #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
    pub tags: Option<Vec<String>>,
    #[serde_as(as = "Option<BoolFromInt>")]
    pub archive: Option<bool>,
    #[serde_as(as = "Option<BoolFromInt>")]
    pub starred: Option<bool>,
    // Will be set as given url for the entry
    // If there will be some redirects, result url will be set as entry url
    pub url: Url,
    pub language: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub preview_picture: Option<Url>,
    #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
    pub authors: Option<Vec<String>>,
    // Generate public link for the url or not
    #[serde_as(as = "Option<BoolFromInt>")]
    pub public: Option<bool>,
    // Origin url for the entry (from where user found it).
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

#[cfg(test)]
mod tests {
    use crate::api::{Detail, EntriesRequest, FindSortEnum, FindSortOrder};

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
