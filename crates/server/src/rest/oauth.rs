use actix_cors::Cors;
use actix_web::{
    Either, Error, HttpMessage,
    dev::ServiceRequest,
    error::{self, ErrorBadRequest, ErrorInternalServerError},
    web::{self, Json, ServiceConfig, post},
};
use actix_web_httpauth::extractors::bearer::BearerAuth;

use crate::{app::AppState, auth::find_user, middleware::TransactionContext, rest::UserInfo};
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

// TODO refactor this ugle deeeep ifs and the method itself
#[expect(clippy::too_many_lines)]
async fn post_token(
    tctx: web::ReqData<TransactionContext<'_>>,
    data: web::Data<AppState>,
    request: Either<web::Form<GetToken>, web::Json<GetToken>>,
) -> actix_web::Result<Json<Token>> {
    let mut tx = tctx.tx()?;
    let request = request.into_inner();
    match request.grant_type {
        Some(gt) if gt == "password" => {
            if let Some(username) = request.username
                && let Some(password) = request.password
            {
                if let Some(client_id) = request.client_id {
                    if let Some(client_secret) = request.client_secret {
                        if let Some(user_row) = find_user(&mut tx, &username, &password).await? {
                            if let Some(client_row) = clients::find_by_user_id_client_id_and_secret(
                                &mut tx,
                                user_row.id,
                                &client_id,
                                &client_secret,
                            )
                            .await?
                            {
                                let new_token = data
                                    .token_storage
                                    .new_token(&mut tx, user_row.id, client_row.id)
                                    .await?;

                                Ok(Json(Token {
                                    access_token: new_token.access_token,
                                    expires_in: new_token.expires_in,
                                    token_type: BEARER.to_owned(),
                                    scope: None,
                                    refresh_token: new_token.refresh_token,
                                }))
                            } else {
                                Err(ErrorBadRequest(oauth_error(
                                    "invalid_client",
                                    "The client credentials are invalid",
                                )))
                            }
                        } else {
                            Err(ErrorBadRequest(oauth_error(
                                "invalid_grant",
                                "Invalid username and password combination",
                            )))
                        }
                    } else {
                        Err(ErrorBadRequest(oauth_error(
                            "invalid_client",
                            "The client credentials are invalid",
                        )))
                    }
                } else {
                    Err(ErrorBadRequest(oauth_error(
                        "invalid_client",
                        "Client id was not found in the headers or body",
                    )))
                }
            } else {
                Err(ErrorBadRequest(oauth_error(
                    "invalid_request",
                    "Missing parameters. \"username\" and \"password\" required",
                )))
            }
        }
        Some(gt) if gt == "refresh_token" => {
            if let Some(client_id) = request.client_id {
                if let Some(client_secret) = request.client_secret {
                    if clients::find_by_client_id_and_secret(&mut tx, &client_id, &client_secret)
                        .await?
                        .is_some()
                    {
                        if let Some(refresh_token) = request.refresh_token {
                            if let Some(new_token) =
                                data.token_storage.refresh(&mut tx, &refresh_token).await?
                            {
                                Ok(Json(Token {
                                    access_token: new_token.access_token,
                                    expires_in: new_token.expires_in,
                                    token_type: "bearer".to_owned(),
                                    scope: None,
                                    refresh_token: new_token.refresh_token,
                                }))
                            } else {
                                Err(ErrorBadRequest(oauth_error(
                                    "invalid_grant",
                                    "Invalid refresh token",
                                )))
                            }
                        } else {
                            Err(ErrorBadRequest(oauth_error(
                                "invalid_request",
                                "No \"refresh_token\" parameter found",
                            )))
                        }
                    } else {
                        Err(ErrorBadRequest(oauth_error(
                            "invalid_client",
                            "The client credentials are invalid",
                        )))
                    }
                } else {
                    Err(ErrorBadRequest(oauth_error(
                        "invalid_client",
                        "The client credentials are invalid",
                    )))
                }
            } else {
                Err(ErrorBadRequest(oauth_error(
                    "invalid_client",
                    "Client id was not found in the headers or body",
                )))
            }
        }
        _ => Err(ErrorBadRequest(oauth_error(
            "invalid_request",
            "Invalid grant_type parameter or parameter missing",
        ))),
    }
}

pub(in crate::rest) async fn auth_extractor(
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
