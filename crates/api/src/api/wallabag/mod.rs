mod entries;
mod tags;

use super::oauth::UserInfo;
use crate::api::wallabag::entries::exists;
use actix_utils::future::{Ready, ready};
use actix_web::web::{ServiceConfig, delete, get, patch, post};
use actix_web::{
    Error, HttpMessage,
    web::{self, Json},
};
use actix_web::{FromRequest, guard};
use actix_web_httpauth::middleware::HttpAuthentication;
use entries::*;
use tags::*;

type Id = i64;

const VERSION: &str = "2.6.12";

pub fn routes(cfg: &mut ServiceConfig) {
    let oauth = HttpAuthentication::with_fn(super::oauth::auth_extractor);

    cfg.route("/api/version.json", get().to(version))
        .route("/api/version", get().to(version));

    cfg.route(
        "/api/version.json",
        web::route().guard(guard::Options()).to(version),
    )
    .route(
        "/api/version",
        web::route().guard(guard::Options()).to(version),
    );

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
