use actix_session::Session;
use actix_web::{
    HttpRequest, HttpResponse, Responder,
    error::{ErrorForbidden, ErrorInternalServerError},
    http::header,
    mime,
    web::{self, Redirect, ServiceConfig, get, post},
};
use chrono::Utc;
use db::{
    ArticlerResult,
    repository::{
        self, Db, Id,
        entries::{self, EntriesCriteria, EntryRow, SortOrder, UpdateEntry},
    },
};
use serde::{Deserialize, Serialize};

use crate::{app::AppState, auth::find_user, middleware::TransactionContext};

pub fn routes(cfg: &mut ServiceConfig) {
    cfg.route("/login", get().to(login))
        .route("/", get().to(index))
        .route("/do_login", post().to(do_login))
        .route("/do_archive", post().to(do_archive));
}

async fn login(_session: Session, app: web::Data<AppState>) -> impl Responder {
    match app
        .handlebars
        .render("index", &Page { nav_partial: None, main_partial: "login".to_string() })
    {
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
            archive: Some(false),
            sort: Some(entries::SortColumn::Updated),
            order: Some(SortOrder::Desc),
            ..Default::default()
        };

        // TODO must load only metadata
        let articles_metadata: Vec<ArticleMetadata> = entries::find_all(&mut tx, &params)
            .await?
            .into_iter()
            .map(|e| e.0.into())
            .collect();

        let context = ArticlesContext {
            page: Page { nav_partial: Some("navigation".to_string()), main_partial: "main".to_string() },
            articles: articles_metadata,
            counters: ArticleCounters::load(&mut tx, user_id).await?,
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
struct ArchiveForm {
    article_id: repository::Id,
}

async fn do_archive(
    session: Session,
    req: HttpRequest,
    form: web::Form<ArchiveForm>,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> actix_web::Result<impl Responder> {
    let mut tx = tctx.tx()?;

    // TODO error messages must be reworked
    let user_id = session
        .get("user_id")
        .map_err(ErrorInternalServerError)?
        .ok_or(ErrorForbidden(""))?;

    let now = Utc::now().timestamp();

    let update = UpdateEntry {
        is_archived: Some(Some(true)),
        archived_at: Some(Some(now)),
        ..Default::default()
    };

    entries::update_by_id(&mut tx, user_id, form.into_inner().article_id, update).await?;

    let referer = req
        .headers()
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("/")
        .to_string();

    Ok(Redirect::to(referer).see_other())
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
    id: repository::Id,
    title: String,
    image_url: Option<String>,
    domain: String,
    reading_time: i32,
}

impl From<EntryRow> for ArticleMetadata {
    fn from(entry: EntryRow) -> Self {
        Self {
            id: entry.id,
            title: entry.title,
            image_url: entry.preview_picture,
            domain: entry.domain_name,
            reading_time: entry.reading_time,
        }
    }
}

#[derive(Serialize)]
struct ArticlesContext {
    #[serde(flatten)]
    page: Page,
    articles: Vec<ArticleMetadata>,
    #[serde(flatten)]
    counters: ArticleCounters,
}

#[derive(Serialize)]
struct ArticleCounters {
    unread_counter: i64,
    all_counter: i64,
    starred_counter: i64,
    archived_counter: i64,
}

impl ArticleCounters {
    async fn load(tx: &mut sqlx::Transaction<'_, Db>, user_id: Id) -> ArticlerResult<Self> {
        Ok(Self {
            unread_counter: entries::count(
                tx,
                &EntriesCriteria {
                    user_id,
                    archive: Some(false),
                    ..Default::default()
                },
            )
            .await?,
            all_counter: entries::count(
                tx,
                &EntriesCriteria {
                    user_id,
                    ..Default::default()
                },
            )
            .await?,
            starred_counter: entries::count(
                tx,
                &EntriesCriteria {
                    user_id,
                    starred: Some(true),
                    ..Default::default()
                },
            )
            .await?,
            archived_counter: entries::count(
                tx,
                &EntriesCriteria {
                    user_id,
                    archive: Some(true),
                    ..Default::default()
                },
            )
            .await?,
        })
    }
}
