use actix_web::{
    Error, HttpMessage,
    dev::ServiceRequest,
    error::{self, ErrorBadRequest, ErrorInternalServerError},
    guard,
    web::{self, Json, ServiceConfig},
};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use serde::{Deserialize, Serialize};

use crate::{
    app::AppState, helpers::find_user, middleware::TransactionContext, repository::clients,
};

static BEARER: &str = "bearer";

type Id = i64;

pub fn routes(cfg: &mut ServiceConfig) {
    cfg.service(
        web::scope("/oauth/v2/token")
            .route(
                "",
                web::route()
                    .guard(guard::Post())
                    .guard(guard::Header(
                        "content-type",
                        "application/x-www-form-urlencoded",
                    ))
                    .to(post_token),
            )
            .route(
                "",
                web::route()
                    .guard(guard::Post())
                    .guard(guard::Header("content-type", "application/json"))
                    .to(post_token_json),
            ),
    );
}

async fn post_token_json(
    tctx: web::ReqData<TransactionContext<'_>>,
    data: web::Data<AppState>,
    request: web::Json<GetToken>,
) -> actix_web::Result<Json<Token>> {
    create_token(tctx, data, request.into_inner()).await
}

async fn post_token(
    tctx: web::ReqData<TransactionContext<'_>>,
    data: web::Data<AppState>,
    request: web::Form<GetToken>,
) -> actix_web::Result<Json<Token>> {
    create_token(tctx, data, request.into_inner()).await
}

async fn create_token(
    tctx: web::ReqData<TransactionContext<'_>>,
    data: web::Data<AppState>,
    r: GetToken,
) -> actix_web::Result<Json<Token>> {
    let mut tx = tctx.tx()?;
    match r.grant_type {
        Some(gt) if gt == "password" => {
            if let Some(username) = r.username
                && let Some(password) = r.password
            {
                if let Some(client_id) = r.client_id {
                    if let Some(client_secret) = r.client_secret {
                        if let Some(user_row) = find_user(&mut tx, &username, &password).await? {
                            if let Some(client_row) = clients::find_by_user_id_client_id_and_secret(
                                &mut tx,
                                user_row.id,
                                &client_id,
                                &client_secret,
                            )
                            .await?
                            {
                                let new_token =
                                    data.token_storage.new_token(user_row.id, client_row.id)?;

                                Ok(Json(Token {
                                    access_token: new_token.access_token,
                                    expires_in: new_token.expires_in,
                                    token_type: BEARER.to_string(),
                                    scope: None,
                                    refresh_token: new_token.refresh_token,
                                }))
                            } else {
                                Err(ErrorBadRequest(o_error(
                                    "invalid_client",
                                    "The client credentials are invalid",
                                )))
                            }
                        } else {
                            Err(ErrorBadRequest(o_error(
                                "invalid_grant",
                                "Invalid username and password combination",
                            )))
                        }
                    } else {
                        Err(ErrorBadRequest(o_error(
                            "invalid_client",
                            "The client credentials are invalid",
                        )))
                    }
                } else {
                    Err(ErrorBadRequest(o_error(
                        "invalid_client",
                        "Client id was not found in the headers or body",
                    )))
                }
            } else {
                Err(ErrorBadRequest(o_error(
                    "invalid_request",
                    "Missing parameters. \"username\" and \"password\" required",
                )))
            }
        }
        Some(gt) if gt == "refresh_token" => {
            if let Some(client_id) = r.client_id {
                if let Some(client_secret) = r.client_secret {
                    if clients::find_by_client_id_and_secret(&mut tx, &client_id, &client_secret)
                        .await?
                        .is_some()
                    {
                        if let Some(refresh_token) = r.refresh_token {
                            if let Some(new_token) = data.token_storage.refresh(&refresh_token)? {
                                Ok(Json(Token {
                                    access_token: new_token.access_token,
                                    expires_in: new_token.expires_in,
                                    token_type: "bearer".to_string(),
                                    scope: None,
                                    refresh_token: new_token.refresh_token,
                                }))
                            } else {
                                Err(ErrorBadRequest(o_error(
                                    "invalid_grant",
                                    "Invalid refresh token",
                                )))
                            }
                        } else {
                            Err(ErrorBadRequest(o_error(
                                "invalid_request",
                                "No \"refresh_token\" parameter found",
                            )))
                        }
                    } else {
                        Err(ErrorBadRequest(o_error(
                            "invalid_client",
                            "The client credentials are invalid",
                        )))
                    }
                } else {
                    Err(ErrorBadRequest(o_error(
                        "invalid_client",
                        "The client credentials are invalid",
                    )))
                }
            } else {
                Err(ErrorBadRequest(o_error(
                    "invalid_client",
                    "Client id was not found in the headers or body",
                )))
            }
        }
        _ => Err(ErrorBadRequest(o_error(
            "invalid_request",
            "Invalid grant_type parameter or parameter missing",
        ))),
    }
}

pub async fn auth_extractor(
    req: ServiceRequest,
    credentials: Option<BearerAuth>,
) -> Result<ServiceRequest, (Error, ServiceRequest)> {
    let Some(credentials) = credentials else {
        return Err((
            error::ErrorUnauthorized(o_error("access_denied", "OAuth2 authentication required")),
            req,
        ));
    };
    let token_storage = &req.app_data::<web::Data<AppState>>().unwrap().token_storage;

    match token_storage.validate(credentials.token()) {
        Ok(Some(claim)) => {
            req.extensions_mut().insert(UserInfo {
                user_id: claim.user_id,
                client_id: claim.client_id,
            });

            Ok(req)
        }
        Ok(None) => Err((
            error::ErrorUnauthorized(o_error(
                "invalid_grant",
                "The access token provided is invalid.",
            )),
            req,
        )),
        Err(e) => Err((ErrorInternalServerError(e), req)),
    }
}

#[derive(Deserialize, Debug)]
struct GetToken {
    grant_type: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    username: Option<String>,
    password: Option<String>,
    refresh_token: Option<String>,
}

#[derive(Serialize, Debug)]
struct Token {
    access_token: String,
    expires_in: i64,
    token_type: String,
    scope: Option<String>,
    refresh_token: String,
}

#[derive(Debug, Serialize)]
struct OauthError {
    error: String,
    error_description: String,
}

impl std::fmt::Display for OauthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let json = serde_json::to_string(self).map_err(|_| std::fmt::Error)?;
        write!(f, "{}", json)
    }
}

fn o_error(error: &str, description: &str) -> OauthError {
    OauthError {
        error: error.to_string(),
        error_description: description.to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct UserInfo {
    pub user_id: Id,
    pub client_id: Id,
}
