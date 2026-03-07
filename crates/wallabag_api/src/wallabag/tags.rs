use actix_web::web::{self, Json};
use app_state::AppState;

use crate::{
    UserInfo,
    error::{NotFoundSnafu, Result},
    models::Tag,
    wallabag::Id,
};
use db::repository::tags;
use dto::{TagLabel, TagsLabel};

pub(crate) async fn get_tags(
    data: web::Data<AppState>,
    user_info: UserInfo,
) -> Result<Json<Vec<Tag>>> {
    let result = tags::get_all(&data.pool, user_info.user_id)
        .await?
        .into_iter()
        .map(std::convert::Into::into)
        .collect();

    Ok(Json(result))
}

pub(crate) async fn delete_tags_by_label(
    data: web::Data<AppState>,
    label: web::Query<TagsLabel>,
    user_info: UserInfo,
) -> Result<Json<Vec<Tag>>> {
    let result = tags::delete_all_by_label(&data.pool, user_info.user_id, &label.labels)
        .await?
        .into_iter()
        .map(std::convert::Into::into)
        .collect();

    Ok(Json(result))
}

pub(crate) async fn delete_tag_by_id(
    data: web::Data<AppState>,
    tag_id: web::Path<Id>,
    user_info: UserInfo,
) -> Result<Json<Tag>> {
    let result = tags::delete_by_id(&data.pool, user_info.user_id, tag_id.into_inner())
        .await?
        .map(std::convert::Into::into);

    if let Some(delete_tag) = result {
        Ok(Json(delete_tag))
    } else {
        NotFoundSnafu {
            msg: "Tag not found",
        }
        .fail()
    }
}

pub(crate) async fn delete_tag_by_label(
    data: web::Data<AppState>,
    label: web::Query<TagLabel>,
    user_info: UserInfo,
) -> Result<Json<Tag>> {
    let result = tags::delete_by_label(&data.pool, user_info.user_id, &label.label)
        .await?
        .map(std::convert::Into::into);

    if let Some(delete_tag) = result {
        Ok(Json(delete_tag))
    } else {
        NotFoundSnafu {
            msg: "Tag not found",
        }
        .fail()
    }
}

mod dto {
    use serde::Deserialize;
    use serde_with::{StringWithSeparator, formats::CommaSeparator, serde_as};

    #[derive(Deserialize)]
    pub struct TagLabel {
        #[serde(rename(deserialize = "tag"))]
        pub label: String,
    }

    #[serde_as]
    #[derive(Deserialize)]
    pub struct TagsLabel {
        #[serde(rename(deserialize = "tags"))]
        #[serde_as(as = "StringWithSeparator::<CommaSeparator, String>")]
        pub labels: Vec<String>,
    }
}
