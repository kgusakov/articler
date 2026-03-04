use actix_session::Session;
use actix_web::{
    HttpRequest, HttpResponse, Responder,
    error::{ErrorForbidden, ErrorInternalServerError, ErrorNotFound},
    http::header,
    mime,
    web::{self, Redirect, ServiceConfig, get, post},
};
use app_state::AppState;
use chrono::Utc;
use db::repository::{
    Db, clients,
    entries::{self, FindParams, SortOrder, UpdateEntry},
};
use helpers::{generate_client_id, generate_client_secret, hash_url};
use sqlx::{Acquire, SqlitePool};
use types::Id;
use url::Url;

use wallabag_api::auth::find_user;

use crate::web::{
    dto::{Client, LoginForm},
    ui::dto::{
        EditArticleTitleForm, HxSource, PartialArticleContext, PartialArticlesContext,
        PartialCategoriesContext,
    },
};

use dto::{
    AddArticleForm, ArchiveForm, ArticleContext, ArticleCounters, ArticleMetadata, ArticlesContext,
    Category, ClientDeleteForm, Clients, CreateClientForm, DeleteForm, FavouriteForm, Page,
};

pub fn routes(cfg: &mut ServiceConfig) {
    cfg.route("/login", get().to(login))
        .route("/", get().to(index))
        .route("/all", get().to(all))
        .route("/favourite", get().to(favourite))
        .route("/archive", get().to(archive))
        .route("/article/{id}", get().to(article))
        .route("/clients", get().to(clients))
        .route("/do_create_client", post().to(do_create_client))
        .route("/do_client_delete", post().to(do_client_delete))
        .route("/logout", get().to(logout))
        .route("/do_login", post().to(do_login))
        .route("/do_archive", post().to(do_archive))
        .route("/do_favourite", post().to(do_favourite))
        .route("/do_delete", post().to(do_delete))
        .route("/do_edit_title", post().to(do_edit_title))
        .route("/add", post().to(do_add))
        .route("/partial/categories", get().to(partial_categories))
        .route("/partial/articles/{category}", get().to(partial_articles));
}

async fn login(_session: Session, app: web::Data<AppState>) -> impl Responder {
    match app.handlebars.render(
        "index",
        &Page {
            nav_partial: None,
            main_partial: "login".to_owned(),
            back_location: None,
        },
    ) {
        Ok(rendered) => HttpResponse::Ok()
            .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
            .body(rendered),
        Err(e) => HttpResponse::from_error(ErrorInternalServerError(e)),
    }
}

async fn clients(session: Session, app: web::Data<AppState>) -> actix_web::Result<HttpResponse> {
    if let Some(user_id) = session.get("user_id").map_err(ErrorInternalServerError)? {
        let clients = clients::find_by_user_id(&app.pool, user_id)
            .await?
            .into_iter()
            .map(Client::from)
            .collect();

        match app.handlebars.render(
            "index",
            &Clients {
                clients,
                page: Page {
                    nav_partial: Some("navigation".to_owned()),
                    main_partial: "clients".to_owned(),
                    back_location: None,
                },
            },
        ) {
            Ok(rendered) => Ok(HttpResponse::Ok()
                .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
                .body(rendered)),
            Err(e) => Err(ErrorInternalServerError(e)),
        }
    } else {
        Ok(HttpResponse::Found()
            .append_header(("Location", "/login"))
            .finish())
    }
}

async fn logout(session: Session) -> actix_web::Result<impl Responder> {
    // TODO this approach purge cookie with CookieSessionStore only if the client correctly process the received answer
    session.purge();
    Ok(Redirect::to("/login").see_other())
}

async fn article(
    session: Session,
    app: web::Data<AppState>,
    id: web::Path<Id>,
    req: HttpRequest,
) -> actix_web::Result<HttpResponse> {
    if let Some(user_id) = session.get("user_id").map_err(ErrorInternalServerError)? {
        if let Some((article, _)) = entries::find_by_id(&app.pool, user_id, id.into_inner()).await?
        {
            let article_page = ArticleContext {
                article: PartialArticleContext {
                    id: article.id,
                    title: article.title,
                    content: article.content,
                    domain: article.domain_name,
                    url: article.url,
                    reading_time: article.reading_time,
                    is_archived: article.is_archived,
                    is_starred: article.is_starred,
                    source: HxSource::Article,
                },
                page: Page {
                    nav_partial: Some("navigation".to_owned()),
                    main_partial: "article".to_owned(),
                    back_location: Some(referer_or_root(&req)),
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

async fn index(session: Session, app: web::Data<AppState>) -> actix_web::Result<HttpResponse> {
    // TODO check if user still exsists
    if let Some(user_id) = session.get("user_id").map_err(ErrorInternalServerError)? {
        main(
            app,
            user_id,
            FindParams {
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

async fn all(session: Session, app: web::Data<AppState>) -> actix_web::Result<HttpResponse> {
    if let Some(user_id) = session.get("user_id").map_err(ErrorInternalServerError)? {
        main(
            app,
            user_id,
            FindParams {
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

async fn favourite(session: Session, app: web::Data<AppState>) -> actix_web::Result<HttpResponse> {
    if let Some(user_id) = session.get("user_id").map_err(ErrorInternalServerError)? {
        main(
            app,
            user_id,
            FindParams {
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

async fn archive(session: Session, app: web::Data<AppState>) -> actix_web::Result<HttpResponse> {
    if let Some(user_id) = session.get("user_id").map_err(ErrorInternalServerError)? {
        main(
            app,
            user_id,
            FindParams {
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
    user_id: Id,
    entries_filter: FindParams,
    active_category: Category,
) -> actix_web::Result<HttpResponse> {
    let mut tx = app.pool.begin().await.map_err(ErrorInternalServerError)?;

    // TODO must load only metadata
    let articles_metadata: Vec<ArticleMetadata> = entries::find_all(&mut *tx, &entries_filter)
        .await?
        .into_iter()
        .map(|e| e.0.into())
        .collect();

    let context = ArticlesContext {
        page: Page {
            nav_partial: Some("navigation".to_owned()),
            main_partial: "main".to_owned(),
            back_location: None,
        },
        articles: articles_metadata,
        counters: ArticleCounters::load(&mut *tx, user_id).await?,
        active_category,
    };

    tx.commit().await.map_err(ErrorInternalServerError)?;

    let rendered = app
        .handlebars
        .render("index", &context)
        .map_err(ErrorInternalServerError)?;
    Ok(HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
        .body(rendered))
}

async fn partial_articles(
    session: Session,
    app: web::Data<AppState>,
    category: web::Path<Category>,
) -> actix_web::Result<HttpResponse> {
    let mut tx = app.pool.begin().await.map_err(ErrorInternalServerError)?;

    let user_id = check_user_id(&session)?;
    let params = find_params_for_category(user_id, &category);

    // TODO must load only metadata
    let articles_metadata: Vec<ArticleMetadata> = entries::find_all(&mut *tx, &params)
        .await?
        .into_iter()
        .map(|e| e.0.into())
        .collect();

    let context = PartialArticlesContext {
        articles: articles_metadata,
        counters: ArticleCounters::load(&mut *tx, user_id).await?,
        active_category: category.into_inner(),
    };

    tx.commit().await.map_err(ErrorInternalServerError)?;

    let rendered = app
        .handlebars
        .render("articles_and_categories", &context)
        .map_err(ErrorInternalServerError)?;
    Ok(HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
        .body(rendered))
}

async fn partial_categories(
    session: Session,
    req: HttpRequest,
    app: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    let Some(user_id) = session.get("user_id").map_err(ErrorInternalServerError)? else {
        return Ok(HttpResponse::Forbidden().finish());
    };

    let active_category = Category::from(&req);

    let context = PartialCategoriesContext {
        counters: ArticleCounters::load(&app.pool, user_id).await?,
        active_category,
    };

    let rendered = app
        .handlebars
        .render("categories", &context)
        .map_err(ErrorInternalServerError)?;
    Ok(HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
        .body(rendered))
}

async fn do_archive(
    session: Session,
    req: HttpRequest,
    form: web::Form<ArchiveForm>,
    app: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    let user_id = check_user_id(&session)?;

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

    entries::update_by_id(&app.pool, user_id, form.article_id, update).await?;

    if is_htmx_request(&req) {
        if let Some(HxSource::Article) = form.source {
            render_article(&app, &app.pool, user_id, form.article_id).await
        } else {
            render_article_cards(&app, &app.pool, user_id, &Category::from(&req)).await
        }
    } else {
        Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, referer_or_root(&req)))
            .finish())
    }
}

async fn do_favourite(
    session: Session,
    req: HttpRequest,
    form: web::Form<FavouriteForm>,
    app: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    let user_id = check_user_id(&session)?;

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

    entries::update_by_id(&app.pool, user_id, form.article_id, update).await?;

    if is_htmx_request(&req) {
        if let Some(HxSource::Article) = form.source {
            render_article(&app, &app.pool, user_id, form.article_id).await
        } else {
            render_article_cards(&app, &app.pool, user_id, &Category::from(&req)).await
        }
    } else {
        Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, referer_or_root(&req)))
            .finish())
    }
}

async fn do_delete(
    session: Session,
    req: HttpRequest,
    form: web::Form<DeleteForm>,
    app: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    let user_id = check_user_id(&session)?;

    let form = form.into_inner();

    entries::delete_by_id(&app.pool, user_id, form.article_id).await?;

    if is_htmx_request(&req) {
        if let Some(HxSource::Article) = form.source {
            let referer = form.back_location.unwrap_or(String::from("/"));
            Ok(HttpResponse::Ok()
                .append_header(("HX-Redirect", referer))
                .finish())
        } else {
            render_article_cards(&app, &app.pool, user_id, &Category::from(&req)).await
        }
    } else {
        let referer = form.back_location.unwrap_or(referer_or_root(&req));
        Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, referer))
            .finish())
    }
}

async fn do_client_delete(
    session: Session,
    req: HttpRequest,
    form: web::Form<ClientDeleteForm>,
    app: web::Data<AppState>,
) -> actix_web::Result<impl Responder> {
    let user_id = check_user_id(&session)?;

    let form = form.into_inner();

    clients::delete_by_id(&app.pool, user_id, form.id).await?;

    Ok(Redirect::to(referer_or_root(&req)).see_other())
}

async fn do_create_client(
    session: Session,
    req: HttpRequest,
    form: web::Form<CreateClientForm>,
    app: web::Data<AppState>,
) -> actix_web::Result<impl Responder> {
    let user_id = check_user_id(&session)?;

    let now = chrono::Utc::now().timestamp();
    let _ = clients::create(
        &app.pool,
        user_id,
        &form.client_name,
        &generate_client_id(),
        &generate_client_secret(),
        now,
    )
    .await?;

    Ok(Redirect::to(referer_or_root(&req)).see_other())
}

async fn do_add(
    session: Session,
    req: HttpRequest,
    app: web::Data<AppState>,
    form: web::Form<AddArticleForm>,
) -> actix_web::Result<impl Responder> {
    let user_id = check_user_id(&session)?;

    let url: Url = form
        .into_inner()
        .url
        .parse()
        .map_err(ErrorInternalServerError)?;

    let document = app.scraper.extract_or_fallback(&url).await;

    let now = Utc::now().timestamp();
    let domain_name = url.domain().or(url.host_str()).unwrap_or("").to_owned();

    let create_entry = entries::CreateEntry {
        user_id,
        url: url.to_string(),
        hashed_url: hash_url(&url),
        given_url: url.to_string(),
        hashed_given_url: hash_url(&url),
        title: document.title,
        content: document.content_html,
        content_text: document.content_text,
        is_archived: false,
        archived_at: None,
        is_starred: false,
        starred_at: None,
        created_at: now,
        updated_at: now,
        mimetype: document.mime_type,
        language: document.language,
        reading_time: document.reading_time,
        domain_name,
        preview_picture: document.image_url.map(|u| u.to_string()),
        origin_url: None,
        published_at: document.published_at.map(|v| v.timestamp()),
        published_by: None,
        is_public: None,
        uid: None,
    };

    entries::create(&app.pool, create_entry, &[]).await?;

    Ok(Redirect::to(referer_or_root(&req)).see_other())
}

pub(in crate::web) async fn do_login(
    app: web::Data<AppState>,
    form: web::Form<LoginForm>,
    req: HttpRequest,
    session: Session,
) -> actix_web::Result<impl Responder> {
    if let Some(user) = find_user(&app.pool, &form.username, &form.password).await? {
        session
            .insert("user_id", user.id)
            .map_err(ErrorInternalServerError)?;

        Ok(HttpResponse::Found()
            .append_header(("Location", "/"))
            .finish())
    } else {
        Ok(HttpResponse::Found()
            .append_header(("Location", referer_or_root(&req)))
            .finish())
    }
}

async fn do_edit_title(
    session: Session,
    req: HttpRequest,
    app: web::Data<AppState>,
    form: web::Form<EditArticleTitleForm>,
) -> actix_web::Result<impl Responder> {
    let user_id = check_user_id(&session)?;

    let form = form.into_inner();

    let update = UpdateEntry {
        title: Some(Some(form.title)),
        ..Default::default()
    };

    entries::update_by_id(&app.pool, user_id, form.article_id, update).await?;

    if is_htmx_request(&req) {
        if let Some(HxSource::Article) = form.source {
            render_article(&app, &app.pool, user_id, form.article_id).await
        } else {
            render_article_cards(&app, &app.pool, user_id, &Category::from(&req)).await
        }
    } else {
        Ok(HttpResponse::SeeOther()
            .append_header((header::LOCATION, referer_or_root(&req)))
            .finish())
    }
}

fn check_user_id(session: &Session) -> Result<i64, actix_web::Error> {
    session
        .get("user_id")
        .map_err(ErrorInternalServerError)?
        .ok_or(ErrorForbidden(""))
}

fn referer_or_root(req: &HttpRequest) -> String {
    req.headers()
        .get(header::REFERER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("/")
        .to_owned()
}

fn is_htmx_request(req: &HttpRequest) -> bool {
    req.headers().get("HX-Request").is_some()
}

fn find_params_for_category(user_id: Id, category: &Category) -> FindParams {
    let all = FindParams {
        user_id,
        sort: Some(entries::SortColumn::Created),
        order: Some(SortOrder::Desc),
        ..Default::default()
    };

    category_to_find_params(category, all)
}

fn category_to_find_params(category: &Category, all: FindParams) -> FindParams {
    match category {
        Category::All => all,
        Category::Favourite => FindParams {
            starred: Some(true),
            ..all
        },
        Category::Archived => FindParams {
            archive: Some(true),
            ..all
        },
        Category::Unread => FindParams {
            archive: Some(false),
            ..all
        },
    }
}

async fn render_article_cards<'c, C>(
    app: &AppState,
    conn: C,
    user_id: Id,
    category: &Category,
) -> actix_web::Result<HttpResponse>
where
    C: Acquire<'c, Database = Db>,
{
    let mut conn = conn.acquire().await.map_err(ErrorInternalServerError)?;

    let params = find_params_for_category(user_id, category);

    // TODO must load only metadata
    let articles: Vec<ArticleMetadata> = entries::find_all(&mut *conn, &params)
        .await
        .map_err(ErrorInternalServerError)?
        .into_iter()
        .map(|e| e.0.into())
        .collect();

    let rendered = app
        .handlebars
        .render(
            "article_cards",
            &serde_json::json!({ "articles": articles }),
        )
        .map_err(ErrorInternalServerError)?;

    Ok(HttpResponse::Ok()
        .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
        .body(rendered))
}

async fn render_article(
    app: &AppState,
    pool: &SqlitePool,
    user_id: Id,
    article_id: Id,
) -> actix_web::Result<HttpResponse> {
    if let Some((article, _)) = entries::find_by_id(pool, user_id, article_id).await? {
        let article_contenxt = PartialArticleContext {
            id: article.id,
            title: article.title,
            content: article.content,
            domain: article.domain_name,
            url: article.url,
            reading_time: article.reading_time,
            is_archived: article.is_archived,
            is_starred: article.is_starred,
            source: HxSource::Article,
        };

        let rendered = app
            .handlebars
            .render("article", &article_contenxt)
            .map_err(ErrorInternalServerError)?;

        Ok(HttpResponse::Ok()
            .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
            .body(rendered))
    } else {
        Err(ErrorNotFound("Article not found"))
    }
}

mod dto {
    use actix_http::header;
    use actix_web::HttpRequest;
    use db::{
        ArticlerResult,
        repository::{
            Db,
            entries::{self, EntryRow, FindParams},
        },
    };
    use serde::{Deserialize, Serialize};
    use types::{Id, ReadingTime};
    use url::Url;

    use crate::web::dto::Client;

    #[derive(Deserialize)]
    pub struct DeleteForm {
        pub article_id: Id,
        pub back_location: Option<String>,
        pub source: Option<HxSource>,
    }

    #[derive(Deserialize)]
    pub struct ArchiveForm {
        pub article_id: Id,
        pub archived: bool,
        pub source: Option<HxSource>,
    }

    #[derive(Deserialize)]
    pub struct FavouriteForm {
        pub article_id: Id,
        pub starred: bool,
        pub source: Option<HxSource>,
    }

    #[derive(Serialize)]
    pub struct Clients {
        #[serde(flatten)]
        pub page: Page,
        pub clients: Vec<Client>,
    }

    #[derive(Deserialize)]
    pub struct CreateClientForm {
        pub client_name: String,
    }

    #[derive(Deserialize)]
    pub struct AddArticleForm {
        pub url: String,
    }

    #[derive(Deserialize)]
    pub struct ClientDeleteForm {
        pub id: Id,
    }

    #[derive(Serialize)]
    pub struct PartialArticleContext {
        pub id: Id,
        pub title: String,
        pub content: String,
        pub domain: String,
        pub url: String,
        pub reading_time: ReadingTime,
        pub is_archived: bool,
        pub is_starred: bool,
        pub source: HxSource,
    }

    #[derive(Serialize)]
    pub struct ArticleContext {
        #[serde(flatten)]
        pub article: PartialArticleContext,
        #[serde(flatten)]
        pub page: Page,
    }

    #[derive(Serialize)]
    pub struct Page {
        pub nav_partial: Option<String>,
        pub main_partial: String,
        pub back_location: Option<String>,
    }

    #[derive(Serialize)]
    pub struct ArticleMetadata {
        pub id: Id,
        pub title: String,
        pub image_url: Option<String>,
        pub domain: String,
        pub reading_time: ReadingTime,
        pub is_archived: bool,
        pub is_starred: bool,
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
    pub struct ArticlesContext {
        #[serde(flatten)]
        pub page: Page,
        pub articles: Vec<ArticleMetadata>,
        #[serde(flatten)]
        pub counters: ArticleCounters,
        pub active_category: Category,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename_all = "lowercase")]
    pub enum Category {
        All,
        Unread,
        Favourite,
        Archived,
    }

    impl Category {
        pub fn from(req: &HttpRequest) -> Self {
            let path = req
                .headers()
                .get(header::REFERER)
                .and_then(|v| v.to_str().ok())
                .and_then(|referer| Url::parse(referer).ok())
                .map_or_else(|| "/".to_owned(), |u| u.path().to_owned());

            match path.as_str() {
                "/all" => Category::All,
                "/favourite" => Category::Favourite,
                "/archive" => Category::Archived,
                _ => Category::Unread,
            }
        }
    }

    #[derive(Serialize)]
    pub struct ArticleCounters {
        pub unread_counter: i64,
        pub all_counter: i64,
        pub starred_counter: i64,
        pub archived_counter: i64,
    }

    impl ArticleCounters {
        pub async fn load<'c, C>(conn: C, user_id: Id) -> ArticlerResult<Self>
        where
            C: sqlx::Acquire<'c, Database = Db>,
        {
            let mut tx = conn.acquire().await?;
            Ok(Self {
                unread_counter: entries::count(
                    &mut *tx,
                    &FindParams {
                        user_id,
                        archive: Some(false),
                        ..Default::default()
                    },
                )
                .await?,
                all_counter: entries::count(
                    &mut *tx,
                    &FindParams {
                        user_id,
                        ..Default::default()
                    },
                )
                .await?,
                starred_counter: entries::count(
                    &mut *tx,
                    &FindParams {
                        user_id,
                        starred: Some(true),
                        ..Default::default()
                    },
                )
                .await?,
                archived_counter: entries::count(
                    &mut *tx,
                    &FindParams {
                        user_id,
                        archive: Some(true),
                        ..Default::default()
                    },
                )
                .await?,
            })
        }
    }

    #[derive(Serialize)]
    pub struct PartialCategoriesContext {
        #[serde(flatten)]
        pub counters: ArticleCounters,
        pub active_category: Category,
    }

    #[derive(Serialize)]
    pub struct PartialArticlesContext {
        pub articles: Vec<ArticleMetadata>,
        #[serde(flatten)]
        pub counters: ArticleCounters,
        pub active_category: Category,
    }

    #[derive(Serialize, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "lowercase")]
    pub enum HxSource {
        Article,
    }

    #[derive(Deserialize)]
    pub struct EditArticleTitleForm {
        pub article_id: Id,
        pub title: String,
        pub source: Option<HxSource>,
    }
}
