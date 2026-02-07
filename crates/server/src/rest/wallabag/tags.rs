use actix_web::{
    error::ErrorNotFound,
    web::{self, Json},
};
use serde::Deserialize;
use serde_with::{StringWithSeparator, formats::CommaSeparator, serde_as};

use crate::{
    rest::{oauth::UserInfo, wallabag::Id},
    middleware::TransactionContext,
    models::Tag,
};
use db::repository::tags;

pub async fn get_tags(
    tctx: web::ReqData<TransactionContext<'_>>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Vec<Tag>>> {
    let mut tx = tctx.tx()?;

    let result = tags::get_all(&mut tx, user_info.user_id)
        .await?
        .into_iter()
        .map(|tr| tr.into())
        .collect();

    Ok(Json(result))
}

pub async fn delete_tags_by_label(
    tctx: web::ReqData<TransactionContext<'_>>,
    label: web::Query<TagsLabel>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Vec<Tag>>> {
    let mut tx = tctx.tx()?;

    let result = tags::delete_all_by_label(&mut tx, user_info.user_id, &label.labels)
        .await?
        .into_iter()
        .map(|tr| tr.into())
        .collect();

    Ok(Json(result))
}

pub async fn delete_tag_by_id(
    tctx: web::ReqData<TransactionContext<'_>>,
    tag_id: web::Path<Id>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Tag>> {
    let mut tx = tctx.tx()?;

    let result = tags::delete_by_id(&mut tx, user_info.user_id, tag_id.into_inner())
        .await?
        .map(|tr| tr.into());

    if let Some(delete_tag) = result {
        Ok(Json(delete_tag))
    } else {
        Err(ErrorNotFound("Tag not found"))
    }
}

pub async fn delete_tag_by_label(
    tctx: web::ReqData<TransactionContext<'_>>,
    label: web::Query<TagLabel>,
    user_info: UserInfo,
) -> actix_web::Result<Json<Tag>> {
    let mut tx = tctx.tx()?;

    let result = tags::delete_by_label(&mut tx, user_info.user_id, &label.label)
        .await?
        .map(|tr| tr.into());

    if let Some(delete_tag) = result {
        Ok(Json(delete_tag))
    } else {
        Err(ErrorNotFound("Tag not found"))
    }
}

#[derive(Deserialize)]
pub struct TagLabel {
    #[serde(rename(deserialize = "tag"))]
    label: String,
}

#[serde_as]
#[derive(Deserialize)]
pub struct TagsLabel {
    #[serde(rename(deserialize = "tags"))]
    #[serde_as(as = "StringWithSeparator::<CommaSeparator, String>")]
    labels: Vec<String>,
}
