use actix_session::Session;
use actix_web::{
    HttpResponse, Responder,
    error::ErrorInternalServerError,
    http::header,
    mime,
    web::{self, ServiceConfig, get, post},
};
use db::repository::entries::{self, EntriesCriteria, SortOrder};
use serde::{Deserialize, Serialize};

use crate::{app::AppState, auth::find_user, middleware::TransactionContext};

pub fn routes(cfg: &mut ServiceConfig) {
    cfg.route("/login", get().to(login))
        .route("/", get().to(index))
        .route("/do_login", post().to(do_login));
}

async fn login(_session: Session, app: web::Data<AppState>) -> impl Responder {
    let page = Page {
        nav_partial: None,
        main_partial: "login".to_string(),
    };
    match app.handlebars.render("index", &page) {
        Ok(rendered) => HttpResponse::Ok()
            .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
            .body(rendered),
        Err(e) => HttpResponse::from_error(ErrorInternalServerError(e)),
    }
}

async fn index(
    session: Session,
    app: web::Data<AppState>,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> actix_web::Result<impl Responder> {
    if let Some(user_id) = session.get("user_id").map_err(ErrorInternalServerError)? {
        let mut tx = tctx.tx()?;

        let params = EntriesCriteria {
            user_id,
            sort: Some(entries::SortColumn::Updated),
            order: Some(SortOrder::Desc),
            ..Default::default()
        };
        let unread_articles = entries::find_all(&mut tx, &params).await?;

        let articles_metadata: Vec<ArticleMetadata> = unread_articles
            .into_iter()
            .map(|e| ArticleMetadata {
                title: e.0.title,
                image_url: e.0.preview_picture,
                domain: e.0.domain_name,
                reading_time: e.0.reading_time,
            })
            .collect();
        let page = Page {
            nav_partial: Some("navigation".to_string()),
            main_partial: "main".to_string(),
        };

        let unread_counter = entries::count(
            &mut tx,
            &EntriesCriteria {
                user_id,
                archive: Some(false),
                ..Default::default()
            },
        )
        .await?;

        let all_counter = entries::count(
            &mut tx,
            &EntriesCriteria {
                user_id,
                ..Default::default()
            },
        )
        .await?;

        let starred_counter = entries::count(
            &mut tx,
            &EntriesCriteria {
                user_id,
                starred: Some(true),
                ..Default::default()
            },
        )
        .await?;

        let archived_counter = entries::count(
            &mut tx,
            &EntriesCriteria {
                user_id,
                archive: Some(true),
                ..Default::default()
            },
        )
        .await?;

        let context = ArticlesContext {
            page,
            articles: articles_metadata,
            unread_counter,
            all_counter,
            starred_counter,
            archived_counter,
        };

        let rendered = app
            .handlebars
            .render("index", &context)
            .map_err(ErrorInternalServerError)?;
        Ok(HttpResponse::Ok()
            .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
            .body(rendered))
    } else {
        Ok(HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish())
    }
}

#[derive(Deserialize)]
pub(in crate::web) struct LoginForm {
    #[serde(rename(deserialize = "_username"))]
    username: String,
    #[serde(rename(deserialize = "_password"))]
    password: String,
}

pub(in crate::web) async fn do_login(
    tctx: web::ReqData<TransactionContext<'_>>,
    form: web::Form<LoginForm>,
    session: Session,
) -> impl Responder {
    let mut tx = match tctx.tx() {
        Ok(tx) => tx,
        Err(e) => return HttpResponse::from_error(ErrorInternalServerError(e)),
    };

    if let Ok(Some(user)) = find_user(&mut tx, &form.username, &form.password).await
        && let Err(err) = session.insert("user_id", user.id)
    {
        return HttpResponse::from_error(ErrorInternalServerError(err));
    }

    HttpResponse::Found()
        .append_header(("Location", "/"))
        .finish()
}

#[derive(Serialize)]
struct Page {
    nav_partial: Option<String>,
    main_partial: String,
}

#[derive(Serialize)]
struct ArticleMetadata {
    title: String,
    image_url: Option<String>,
    domain: String,
    reading_time: i32,
}

#[derive(Serialize)]
struct ArticlesContext {
    #[serde(flatten)]
    page: Page,
    articles: Vec<ArticleMetadata>,
    unread_counter: i64,
    all_counter: i64,
    starred_counter: i64,
    archived_counter: i64,
}
