use actix_session::Session;
use actix_web::{
    HttpRequest, HttpResponse, Responder,
    error::{ErrorForbidden, ErrorInternalServerError, ErrorNotFound},
    http::header,
    mime,
    web::{self, Redirect, ServiceConfig, get, post},
};
use chrono::Utc;
use db::repository::{
    Id, clients,
    entries::{self, FindParams, SortOrder, UpdateEntry},
};
use helpers::{generate_client_id, generate_client_secret, hash_url};
use url::Url;

use crate::{
    app::AppState,
    auth::find_user,
    middleware::TransactionContext,
    scraper::extract_title,
    web::dto::{Client, LoginForm},
};

use dto::{
    AddArticleForm, ArchiveForm, ArticleCounters, ArticleMetadata, ArticlePageData,
    ArticlesContext, Category, ClientDeleteForm, Clients, CreateClientForm, DeleteForm,
    FavouriteForm, Page,
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
        .route("/add", post().to(do_add));
}

async fn login(_session: Session, app: web::Data<AppState>) -> impl Responder {
    match app.handlebars.render(
        "index",
        &Page {
            nav_partial: None,
            main_partial: "login".to_owned(),
        },
    ) {
        Ok(rendered) => HttpResponse::Ok()
            .append_header((header::CONTENT_TYPE, mime::TEXT_HTML))
            .body(rendered),
        Err(e) => HttpResponse::from_error(ErrorInternalServerError(e)),
    }
}

async fn clients(
    session: Session,
    app: web::Data<AppState>,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> actix_web::Result<HttpResponse> {
    if let Some(user_id) = session.get("user_id").map_err(ErrorInternalServerError)? {
        let mut tx = tctx.tx()?;

        let clients = clients::find_by_user_id(&mut tx, user_id)
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
                    nav_partial: Some("navigation".to_owned()),
                    main_partial: "article".to_owned(),
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
    tctx: web::ReqData<TransactionContext<'_>>,
    user_id: Id,
    entries_filter: FindParams,
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
            nav_partial: Some("navigation".to_owned()),
            main_partial: "main".to_owned(),
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

async fn do_archive(
    session: Session,
    req: HttpRequest,
    form: web::Form<ArchiveForm>,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> actix_web::Result<impl Responder> {
    let mut tx = tctx.tx()?;

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

    entries::update_by_id(&mut tx, user_id, form.article_id, update).await?;

    Ok(Redirect::to(referer_or_root(&req)).see_other())
}

async fn do_favourite(
    session: Session,
    req: HttpRequest,
    form: web::Form<FavouriteForm>,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> actix_web::Result<impl Responder> {
    let mut tx = tctx.tx()?;

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

    entries::update_by_id(&mut tx, user_id, form.article_id, update).await?;

    Ok(Redirect::to(referer_or_root(&req)).see_other())
}

async fn do_delete(
    session: Session,
    req: HttpRequest,
    form: web::Form<DeleteForm>,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> actix_web::Result<impl Responder> {
    let mut tx = tctx.tx()?;

    let user_id = check_user_id(&session)?;

    let form = form.into_inner();

    entries::delete_by_id(&mut tx, user_id, form.article_id).await?;

    let referer = form.back_location.unwrap_or(referer_or_root(&req));

    Ok(Redirect::to(referer).see_other())
}

async fn do_client_delete(
    session: Session,
    req: HttpRequest,
    form: web::Form<ClientDeleteForm>,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> actix_web::Result<impl Responder> {
    let mut tx = tctx.tx()?;

    let user_id = check_user_id(&session)?;

    let form = form.into_inner();

    clients::delete_by_id(&mut tx, user_id, form.id).await?;

    Ok(Redirect::to(referer_or_root(&req)).see_other())
}

async fn do_create_client(
    session: Session,
    req: HttpRequest,
    form: web::Form<CreateClientForm>,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> actix_web::Result<impl Responder> {
    let mut tx = tctx.tx()?;

    let user_id = check_user_id(&session)?;

    let now = chrono::Utc::now().timestamp();
    let _ = clients::create(
        &mut tx,
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
    data: web::Data<AppState>,
    form: web::Form<AddArticleForm>,
    tctx: web::ReqData<TransactionContext<'_>>,
) -> actix_web::Result<impl Responder> {
    let mut tx = tctx.tx()?;

    let user_id = check_user_id(&session)?;

    let url: Url = form
        .into_inner()
        .url
        .parse()
        .map_err(ErrorInternalServerError)?;

    let (title, content, mime_type, published_at, language, preview_picture) =
        match data.scraper.extract(&url).await {
            Ok(document) => (
                document.title,
                document.content_html,
                document.mime_type.unwrap_or_default(),
                document.published_at,
                document.language,
                document.image_url,
            ),
            Err(err) => {
                log::error!("Error while parsing url {url}: {err:?}");
                (
                    extract_title(&url).to_owned(),
                    String::new(),
                    String::new(),
                    None,
                    None,
                    None,
                )
            }
        };

    let now = Utc::now().timestamp();
    let domain_name = url.domain().or(url.host_str()).unwrap_or("").to_owned();

    let create_entry = entries::CreateEntry {
        user_id,
        url: url.to_string(),
        hashed_url: hash_url(&url),
        given_url: url.to_string(),
        hashed_given_url: hash_url(&url),
        title,
        content,
        is_archived: false,
        archived_at: None,
        is_starred: false,
        starred_at: None,
        created_at: now,
        updated_at: now,
        mimetype: Some(mime_type),
        language,
        reading_time: 0,
        domain_name,
        preview_picture: preview_picture.map(|u| u.to_string()),
        origin_url: None,
        published_at: published_at.map(|v| v.timestamp()),
        published_by: None,
        is_public: None,
        uid: None,
    };

    entries::create(&mut tx, create_entry, &[]).await?;

    Ok(Redirect::to(referer_or_root(&req)).see_other())
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

mod dto {
    use db::{
        ArticlerResult,
        repository::{
            self, Db, Id,
            entries::{self, EntryRow, FindParams},
        },
    };
    use serde::{Deserialize, Serialize};

    use crate::web::dto::Client;

    #[derive(Deserialize)]
    pub struct DeleteForm {
        pub article_id: repository::Id,
        pub back_location: Option<String>,
    }

    #[derive(Deserialize)]
    pub struct ArchiveForm {
        pub article_id: repository::Id,
        pub archived: bool,
    }

    #[derive(Deserialize)]
    pub struct FavouriteForm {
        pub article_id: repository::Id,
        pub starred: bool,
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
    pub struct ArticlePageData {
        pub id: Id,
        pub title: String,
        pub content: String,
        pub domain: String,
        pub url: String,
        pub reading_time: i32,
        pub is_archived: bool,
        pub is_starred: bool,
        #[serde(flatten)]
        pub page: Page,
    }

    #[derive(Serialize)]
    pub struct Page {
        pub nav_partial: Option<String>,
        pub main_partial: String,
    }

    #[derive(Serialize)]
    pub struct ArticleMetadata {
        pub id: repository::Id,
        pub title: String,
        pub image_url: Option<String>,
        pub domain: String,
        pub reading_time: i32,
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

    #[derive(Serialize)]
    #[serde(rename_all = "lowercase")]
    pub enum Category {
        All,
        Unread,
        Favourite,
        Archived,
    }

    #[derive(Serialize)]
    pub struct ArticleCounters {
        pub unread_counter: i64,
        pub all_counter: i64,
        pub starred_counter: i64,
        pub archived_counter: i64,
    }

    impl ArticleCounters {
        pub async fn load(tx: &mut sqlx::Transaction<'_, Db>, user_id: Id) -> ArticlerResult<Self> {
            Ok(Self {
                unread_counter: entries::count(
                    tx,
                    &FindParams {
                        user_id,
                        archive: Some(false),
                        ..Default::default()
                    },
                )
                .await?,
                all_counter: entries::count(
                    tx,
                    &FindParams {
                        user_id,
                        ..Default::default()
                    },
                )
                .await?,
                starred_counter: entries::count(
                    tx,
                    &FindParams {
                        user_id,
                        starred: Some(true),
                        ..Default::default()
                    },
                )
                .await?,
                archived_counter: entries::count(
                    tx,
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
}
