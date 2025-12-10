use actix_web::{
    get,
    web::{Json, Path},
};
use serde::{Deserialize, Serialize};

// TODO /api pref should be moved as a base prefix
#[get("/api/entries")]
pub async fn entries(request: Path<EntriesRequest>) -> Json<Entries> {
    todo!();
}

// TODO all urls must be appropriate url type instead of String

#[derive(Deserialize)]
struct EntriesRequest {
    archive: Option<i32>,
    starred: Option<i32>,
    // TODO: must be a enum of created, updated, archived
    sort: Option<String>,
    // TODO: must be a enum of asc, desc
    order: Option<String>,
    page: Option<i32>,
    per_page: Option<i32>,
    // TODO: must be an array of comma separated strings
    tags: Option<String>,
    since: Option<i32>,
    public: Option<i32>,
    //TODO: must be an enum of metadata, full
    detail: Option<String>,
    domain_name: Option<String>,
}

#[derive(Serialize)]
struct Entries {
    page: i32,
    limit: i32,
    pages: i32,
    total: i32,
    embedded: Embedded,
    links: Links,
}

#[derive(Serialize)]
struct Embedded {
    items: Vec<Entry>,
}

#[derive(Serialize)]
struct Entry {
    id: i32,
    url: String,
    hashed_url: Option<String>,
    given_url: Option<String>,
    hashed_given_url: Option<String>,
    title: String,
    content: String,
    // TODO must be 0,1 in json
    is_archived: bool,
    // TODO must be valid date
    archived_at: Option<String>,
    // TODO must be 0,1 in json
    is_starred: bool,
    // TODO must be valid date
    starred_at: Option<String>,
    tags: Vec<Tag>,
    // TODO must be a valid date
    created_at: String,
    // TODO must be a valid date
    update_at: String,
    annotations: Option<Vec<Annotation>>,
    mimetype: Option<String>,
    language: Option<String>,
    reading_time: i32,
    domain_name: String,
    preview_picture: Option<String>,
    origin_url: Option<String>,
    published_at: Option<String>,
    published_by: Option<String>,
    is_public: Option<bool>,
    uid: Option<String>,
}

#[derive(Serialize)]
struct Annotation {
    id: i32,
    annotator_schema_version: String,
    text: String,
    // TODO date type must be used
    created_at: String,
    // TODO date type must be used
    updated_at: String,
    quote: String,
    ranges: Vec<Range>,
}

#[derive(Serialize)]
struct Range {
    start: String,
    end: String,
    #[serde(rename(serialize = "startOffset"))]
    start_offset: i64,
    #[serde(rename(serialize = "endOffset"))]
    end_offset: i64,
}

#[derive(Serialize)]
struct Tag {
    id: i32,
    label: String,
    slug: String,
}

#[derive(Serialize)]
struct Links {
    _self: Link,
    first: Link,
    last: Link,
    next: Link,
}

#[derive(Serialize)]
struct Link {
    // TODO must be just self in json
    href: String,
}

// TODO:
// #[post("oauth/v2/token")]
// pub async fn token(
//     data: web::Data<AppState>,
//     web::Form(tokenRequest): web::Form<TokenRequest>,
// ) -> std::result::Result<HttpResponse, actix_web::error::Error> {
//     todo!("Implement token API request");
//     // assert_eq!(tokenRequest.grant_type, "password");

//     // match data.stroage
//     //     .find_user_by_username_and_password(&tokenRequest.username, &tokenRequest.password).await {
//     //         Some(user) => {
//     //             match data.stroage.find_client_by_user_id(user.user_id).await? {
//     //                 Some(client) => client
//     //             }
//     //         }
//     //     }
//     // data.stroage.get_client_id(tokenRequest.client_id);

//     // Ok(HttpResponse::Ok())
// }

// fn generate_access_token() -> String {
//     Alphanumeric.sample_string(&mut rand::rng(), 16)
// }

// #[derive(Deserialize)]
// struct TokenRequest {
//     grant_type: String,
//     client_id: String,
//     client_secret: String,
//     username: String,
//     password: String,
// }

// struct TokenResponse {
//     access_token: String,
//     expires_in: u32,
//     refresh_token: String,
//     token_type: String,
// }

// struct Client {
//     id: u64,
//     client_id: String,
//     client_secret: String,
//     user_id: u64,
// }

// struct User {
//     id: u64,
//     username: String,
//     password: String,
// }

// struct Token {
//     id: u64,
//     access_token: String,
//     refresh_token: Option<String>,
//     user_id: u64,
// }

// struct AppState {
//     pub stroage: Storage,
// }
