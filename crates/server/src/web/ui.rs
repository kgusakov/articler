use actix_session::Session;
use actix_web::{
    HttpRequest, HttpResponse, Responder,
    error::{ErrorForbidden, ErrorInternalServerError, ErrorNotFound},
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
        .route("/all", get().to(all))
        .route("/favourite", get().to(favourite))
        .route("/archive", get().to(archive))
        .route("/article/{id}", get().to(article))
        .route("/do_login", post().to(do_login))
        .route("/do_archive", post().to(do_archive))
        .route("/do_favourite", post().to(do_favourite))
        .route("/do_delete", post().to(do_delete));
}

async fn login(_session: Session, app: web::Data<AppState>) -> impl Responder {
    match app.handlebars.render(
        "index",
        &Page {
            nav_partial: None,
            main_partial: "login".to_string(),
        },
    ) {
        Ok(rendered) => HttpResponse::Ok()
            .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
            .body(rendered),
        Err(e) => HttpResponse::from_error(ErrorInternalServerError(e)),
    }
}
async fn article(
    session: Session,
    app: web::Data<AppState>,
    tctx: web::ReqData<TransactionContext<'_>>,
    id: web::Path<Id>,
) -> actix_web::Result<HttpResponse> {
    if let Some(user_id) = session.get("user_id").map_err(ErrorInternalServerError)? {
        let mut tx = tctx.tx()?;
        if let Some((article, _)) = entries::find_by_id(&mut tx, user_id, id.into_inner()).await? {
            let article_page = ArticlePageData {
                id: article.id,
                title: article.title,
                content: article.content,
                domain: article.domain_name,
                url: article.url,
                reading_time: article.reading_time,
                is_archived: article.is_archived,
                is_starred: article.is_starred,
                page: Page {
                    nav_partial: Some("navigation".to_string()),
                    main_partial: "article".to_string(),
                },
            };
            match app.handlebars.render("index", &article_page) {
                Ok(rendered) => Ok(HttpResponse::Ok()
                    .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
                    .body(rendered)),
                Err(e) => Err(ErrorInternalServerError(e)),
            }
        } else {
            // TODO make normal 404 screen
            Err(ErrorNotFound("Article not found"))
        }
    } else {
        Ok(HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish())
    }
}

async fn index(
    session: Session,
    app: web::Data<AppState>,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> actix_web::Result<HttpResponse> {
    // TODO check if user still exsists
    if let Some(user_id) = session.get("user_id").map_err(ErrorInternalServerError)? {
        main(
            app,
            tctx,
            user_id,
            EntriesCriteria {
                user_id,
                archive: Some(false),
                sort: Some(entries::SortColumn::Created),
                order: Some(SortOrder::Desc),
                ..Default::default()
            },
            Category::Unread,
        )
        .await
    } else {
        Ok(HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish())
    }
}

async fn all(
    session: Session,
    app: web::Data<AppState>,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> actix_web::Result<HttpResponse> {
    if let Some(user_id) = session.get("user_id").map_err(ErrorInternalServerError)? {
        main(
            app,
            tctx,
            user_id,
            EntriesCriteria {
                user_id,
                sort: Some(entries::SortColumn::Created),
                order: Some(SortOrder::Desc),
                ..Default::default()
            },
            Category::All,
        )
        .await
    } else {
        Ok(HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish())
    }
}

async fn favourite(
    session: Session,
    app: web::Data<AppState>,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> actix_web::Result<HttpResponse> {
    if let Some(user_id) = session.get("user_id").map_err(ErrorInternalServerError)? {
        main(
            app,
            tctx,
            user_id,
            EntriesCriteria {
                user_id,
                starred: Some(true),
                sort: Some(entries::SortColumn::Created),
                order: Some(SortOrder::Desc),
                ..Default::default()
            },
            Category::Favourite,
        )
        .await
    } else {
        Ok(HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish())
    }
}

async fn archive(
    session: Session,
    app: web::Data<AppState>,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> actix_web::Result<HttpResponse> {
    if let Some(user_id) = session.get("user_id").map_err(ErrorInternalServerError)? {
        main(
            app,
            tctx,
            user_id,
            EntriesCriteria {
                user_id,
                archive: Some(true),
                sort: Some(entries::SortColumn::Created),
                order: Some(SortOrder::Desc),
                ..Default::default()
            },
            Category::Archived,
        )
        .await
    } else {
        Ok(HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish())
    }
}

async fn main(
    app: web::Data<AppState>,
    tctx: web::ReqData<TransactionContext<'_>>,
    user_id: Id,
    entries_filter: EntriesCriteria,
    active_category: Category,
) -> actix_web::Result<HttpResponse> {
    let mut tx = tctx.tx()?;

    // TODO must load only metadata
    let articles_metadata: Vec<ArticleMetadata> = entries::find_all(&mut tx, &entries_filter)
        .await?
        .into_iter()
        .map(|e| e.0.into())
        .collect();

    let context = ArticlesContext {
        page: Page {
            nav_partial: Some("navigation".to_string()),
            main_partial: "main".to_string(),
        },
        articles: articles_metadata,
        counters: ArticleCounters::load(&mut tx, user_id).await?,
        active_category,
    };

    let rendered = app
        .handlebars
        .render("index", &context)
        .map_err(ErrorInternalServerError)?;
    Ok(HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
        .body(rendered))
}

#[derive(Deserialize)]
struct DeleteForm {
    article_id: repository::Id,
    back_location: Option<String>,
}

#[derive(Deserialize)]
struct ArchiveForm {
    article_id: repository::Id,
    archived: bool,
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

    let form = form.into_inner();

    let update = UpdateEntry {
        is_archived: Some(Some(form.archived)),
        archived_at: Some(if form.archived {
            Some(Utc::now().timestamp())
        } else {
            None
        }),
        ..Default::default()
    };

    entries::update_by_id(&mut tx, user_id, form.article_id, update).await?;

    let referer = req
        .headers()
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("/")
        .to_string();

    Ok(Redirect::to(referer).see_other())
}

#[derive(Deserialize)]
struct FavouriteForm {
    article_id: repository::Id,
    starred: bool,
}

async fn do_favourite(
    session: Session,
    req: HttpRequest,
    form: web::Form<FavouriteForm>,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> actix_web::Result<impl Responder> {
    let mut tx = tctx.tx()?;

    let user_id = session
        .get("user_id")
        .map_err(ErrorInternalServerError)?
        .ok_or(ErrorForbidden(""))?;

    let form = form.into_inner();

    let update = UpdateEntry {
        is_starred: Some(Some(form.starred)),
        starred_at: Some(if form.starred {
            Some(Utc::now().timestamp())
        } else {
            None
        }),
        ..Default::default()
    };

    entries::update_by_id(&mut tx, user_id, form.article_id, update).await?;

    let referer = req
        .headers()
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("/")
        .to_string();

    Ok(Redirect::to(referer).see_other())
}

async fn do_delete(
    session: Session,
    req: HttpRequest,
    form: web::Form<DeleteForm>,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> actix_web::Result<impl Responder> {
    let mut tx = tctx.tx()?;

    let user_id = session
        .get("user_id")
        .map_err(ErrorInternalServerError)?
        .ok_or(ErrorForbidden(""))?;

    let form = form.into_inner();

    entries::delete_by_id(&mut tx, user_id, form.article_id).await?;

    let referer = form.back_location.unwrap_or(
        req.headers()
            .get(header::REFERER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("/")
            .to_string(),
    );

    Ok(Redirect::to(referer).see_other())
}

#[derive(Serialize, Deserialize)]
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
struct ArticlePageData {
    id: Id,
    title: String,
    content: String,
    domain: String,
    url: String,
    reading_time: i32,
    is_archived: bool,
    is_starred: bool,
    #[serde(flatten)]
    page: Page,
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
    is_archived: bool,
    is_starred: bool,
}

impl From<EntryRow> for ArticleMetadata {
    fn from(entry: EntryRow) -> Self {
        Self {
            id: entry.id,
            title: entry.title,
            image_url: entry.preview_picture,
            domain: entry.domain_name,
            reading_time: entry.reading_time,
            is_archived: entry.is_archived,
            is_starred: entry.is_starred,
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
    active_category: Category,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Category {
    All,
    Unread,
    Favourite,
    Archived,
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
