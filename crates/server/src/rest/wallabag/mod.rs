mod entries;
mod tags;

use super::oauth::UserInfo;
use crate::rest::wallabag::entries::exists;
use actix_cors::Cors;
use actix_utils::future::{Ready, ready};
use actix_web::FromRequest;
use actix_web::web::{ServiceConfig, delete, get, patch, post};
use actix_web::{
    Error, HttpMessage,
    web::{self, Json},
};
use actix_web_httpauth::middleware::HttpAuthentication;
use entries::*;
use tags::*;

type Id = i64;

const VERSION: &str = "2.6.12";

pub fn routes(cfg: &mut ServiceConfig) {
    let oauth = HttpAuthentication::with_fn(super::oauth::auth_extractor);

    // TODO permissive cors is a security issue - must be fixed
    let cors = Cors::permissive();

    // TODO Tooooo long already - refactoring needed
    cfg.service(
        web::scope("/api")
            .wrap(cors)
            .service(web::resource(["version", "version.json"]).route(get().to(version)))
            .service(
                web::scope("")
                    .wrap(oauth)
                    .service(
                        web::resource(["/entries", "/entries.json"])
                            .route(get().to(entries))
                            .route(post().to(post_entries)),
                    )
                    .service(
                        web::scope("/entries")
                            .service(
                                web::resource(["/exists.json", "/exists"]).route(get().to(exists)),
                            )
                            .service(
                                web::resource(["/{entry_id}.json", "/{entry_id}"])
                                    .route(delete().to(delete_entry))
                                    .route(patch().to(patch_entry)),
                            )
                            .service(
                                web::resource(["/{entry_id}/tags.json", "/{entry_id}/tags"])
                                    .route(get().to(get_tags_by_entry))
                                    .route(post().to(post_entry_tags)),
                            )
                            .service(
                                web::resource([
                                    "/{entry_id}/tags/{tag_id}.json",
                                    "/{entry_id}/tags/{tag_id}",
                                ])
                                .route(delete().to(delete_tag_from_entry)),
                            ),
                    )
                    .service(web::resource(["/tags.json", "/tags"]).route(get().to(get_tags)))
                    .service(
                        web::scope("/tags")
                            .service(
                                web::resource(["/label.json", "label"])
                                    .route(delete().to(delete_tags_by_label)),
                            )
                            .service(
                                web::resource(["/{tag_id}.json", "{tag_id}"])
                                    .route(delete().to(delete_tag_by_id)),
                            ),
                    )
                    .service(
                        web::scope("/tag").service(
                            web::resource(["label.json", "label"])
                                .route(delete().to(delete_tag_by_label)),
                        ),
                    ),
            ),
    );
}

async fn version() -> actix_web::Result<Json<String>> {
    Ok(Json(VERSION.to_string()))
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
