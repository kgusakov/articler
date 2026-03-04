use actix_cors::Cors;
use actix_web::{
    Either, Error, HttpMessage,
    dev::ServiceRequest,
    error::{self, ErrorBadRequest, ErrorInternalServerError},
    web::{self, Json, ServiceConfig, post},
};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use app_state::AppState;

use crate::{UserInfo, auth::find_user};
use db::repository::clients;
use dto::{GetToken, OauthError, Token};

static BEARER: &str = "bearer";

pub fn routes(cfg: &mut ServiceConfig) {
    // TODO permissive cors is a security issue - must be fixed
    let cors = Cors::permissive();

    cfg.service(
        web::scope("/oauth/v2/token")
            .wrap(cors)
            .route("", post().to(post_token)),
    );
}

async fn post_token(
    data: web::Data<AppState>,
    request: Either<web::Form<GetToken>, web::Json<GetToken>>,
) -> actix_web::Result<Json<Token>> {
    let request = request.into_inner();
    match &request.grant_type {
        Some(gt) if gt == "password" => new_token(data, request).await,
        Some(gt) if gt == "refresh_token" => refresh_token(data, request).await,
        _ => Err(ErrorBadRequest(oauth_error(
            "invalid_request",
            "Invalid grant_type parameter or parameter missing",
        ))),
    }
}

async fn refresh_token(data: web::Data<AppState>, request: GetToken) -> Result<Json<Token>, Error> {
    let Some(client_id) = request.client_id else {
        return Err(ErrorBadRequest(oauth_error(
            "invalid_client",
            "Client id was not found in the headers or body",
        )));
    };

    let Some(client_secret) = request.client_secret else {
        return Err(ErrorBadRequest(oauth_error(
            "invalid_client",
            "The client credentials are invalid",
        )));
    };

    if clients::find_by_client_id_and_secret(&data.pool, &client_id, &client_secret)
        .await?
        .is_none()
    {
        return Err(ErrorBadRequest(oauth_error(
            "invalid_client",
            "The client credentials are invalid",
        )));
    }

    let Some(refresh_token) = request.refresh_token else {
        return Err(ErrorBadRequest(oauth_error(
            "invalid_request",
            "No \"refresh_token\" parameter found",
        )));
    };

    let Some(new_token) = data
        .token_storage
        .refresh(&data.pool, &refresh_token)
        .await?
    else {
        return Err(ErrorBadRequest(oauth_error(
            "invalid_grant",
            "Invalid refresh token",
        )));
    };

    Ok(Json(Token {
        access_token: new_token.access_token,
        expires_in: new_token.expires_in,
        token_type: BEARER.to_owned(),
        scope: None,
        refresh_token: new_token.refresh_token,
    }))
}

async fn new_token(data: web::Data<AppState>, request: GetToken) -> Result<Json<Token>, Error> {
    let Some(username) = request.username else {
        return Err(ErrorBadRequest(oauth_error(
            "invalid_request",
            "Missing parameters. \"username\" and \"password\" required",
        )));
    };

    let Some(password) = request.password else {
        return Err(ErrorBadRequest(oauth_error(
            "invalid_request",
            "Missing parameters. \"username\" and \"password\" required",
        )));
    };

    let Some(client_id) = request.client_id else {
        return Err(ErrorBadRequest(oauth_error(
            "invalid_client",
            "Client id was not found in the headers or body",
        )));
    };

    let Some(client_secret) = request.client_secret else {
        return Err(ErrorBadRequest(oauth_error(
            "invalid_client",
            "The client credentials are invalid",
        )));
    };

    let mut tx = data.pool.begin().await.map_err(ErrorInternalServerError)?;

    let Some(user_row) = find_user(&mut *tx, &username, &password).await? else {
        return Err(ErrorBadRequest(oauth_error(
            "invalid_grant",
            "Invalid username and password combination",
        )));
    };

    let Some(client_row) = clients::find_by_user_id_client_id_and_secret(
        &mut *tx,
        user_row.id,
        &client_id,
        &client_secret,
    )
    .await?
    else {
        return Err(ErrorBadRequest(oauth_error(
            "invalid_client",
            "The client credentials are invalid",
        )));
    };

    tx.commit().await.map_err(ErrorInternalServerError)?;

    let new_token = data
        .token_storage
        .new_token(&data.pool, user_row.id, client_row.id)
        .await?;

    Ok(Json(Token {
        access_token: new_token.access_token,
        expires_in: new_token.expires_in,
        token_type: BEARER.to_owned(),
        scope: None,
        refresh_token: new_token.refresh_token,
    }))
}

pub(crate) async fn auth_extractor(
    req: ServiceRequest,
    credentials: Option<BearerAuth>,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    let Some(credentials) = credentials else {
        return Err((
            error::ErrorUnauthorized(oauth_error(
                "access_denied",
                "OAuth2 authentication required",
            )),
            req,
        ));
    };
    let token_storage = &req
        .app_data::<web::Data<AppState>>()
        .expect("App data for the request is not configured properly")
        .token_storage;

    match token_storage.validate(credentials.token()).await {
        Ok(Some(claim)) => {
            req.extensions_mut().insert(UserInfo {
                user_id: claim.user_id,
                client_id: claim.client_id,
            });

            Ok(req)
        }
        Ok(None) => Err((
            error::ErrorUnauthorized(oauth_error(
                "invalid_grant",
                "The access token provided is invalid.",
            )),
            req,
        )),
        Err(e) => Err((ErrorInternalServerError(e), req)),
    }
}

fn oauth_error(error: &str, description: &str) -> OauthError {
    OauthError {
        error: error.to_owned(),
        error_description: description.to_owned(),
    }
}

mod dto {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Debug)]
    pub struct GetToken {
        pub grant_type: Option<String>,
        pub client_id: Option<String>,
        pub client_secret: Option<String>,
        pub username: Option<String>,
        pub password: Option<String>,
        pub refresh_token: Option<String>,
    }

    #[derive(Serialize, Debug)]
    pub struct Token {
        pub access_token: String,
        pub refresh_token: String,
        pub expires_in: i64,
        pub token_type: String,
        pub scope: Option<String>,
    }

    #[derive(Debug, Serialize)]
    pub struct OauthError {
        pub error: String,
        pub error_description: String,
    }

    impl std::fmt::Display for OauthError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let json = serde_json::to_string(self).map_err(|_| std::fmt::Error)?;
            write!(f, "{json}")
        }
    }
}
