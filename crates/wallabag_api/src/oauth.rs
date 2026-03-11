use actix_cors::Cors;
use actix_http::StatusCode;
use actix_web::{
    Either, HttpMessage,
    dev::ServiceRequest,
    web::{self, Json, ServiceConfig, post},
};
use actix_web_httpauth::extractors::bearer::BearerAuth;
use app_state::AppState;
use auth::find_user;
use snafu::ResultExt;

use crate::{
    UserInfo,
    error::{OauthSnafu, Result, TokenStorageSnafu},
};
use db::repository::clients;
use dto::{GetToken, Token};

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
) -> Result<Json<Token>> {
    let request = request.into_inner();
    match &request.grant_type {
        Some(gt) if gt == "password" => new_token(data, request).await,
        Some(gt) if gt == "refresh_token" => refresh_token(data, request).await,
        _ => OauthSnafu {
            error: "invalid_request",
            description: "Invalid grant_type parameter or parameter missing",
            status_code: StatusCode::BAD_REQUEST,
        }
        .fail(),
    }
}

async fn refresh_token(data: web::Data<AppState>, request: GetToken) -> Result<Json<Token>> {
    let Some(client_id) = request.client_id else {
        return OauthSnafu {
            error: "invalid_client",
            description: "Client id was not found in the headers or body",
            status_code: StatusCode::BAD_REQUEST,
        }
        .fail();
    };

    let Some(client_secret) = request.client_secret else {
        return OauthSnafu {
            error: "invalid_client",
            description: "The client credentials are invalid",
            status_code: StatusCode::BAD_REQUEST,
        }
        .fail();
    };

    if clients::find_by_client_id_and_secret(&data.pool, &client_id, &client_secret)
        .await?
        .is_none()
    {
        return OauthSnafu {
            error: "invalid_client",
            description: "The client credentials are invalid",
            status_code: StatusCode::BAD_REQUEST,
        }
        .fail();
    }

    let Some(refresh_token) = request.refresh_token else {
        return OauthSnafu {
            error: "invalid_request",
            description: "No \"refresh_token\" parameter found",
            status_code: StatusCode::BAD_REQUEST,
        }
        .fail();
    };

    let Some(new_token) = data
        .token_storage
        .refresh(&data.pool, &refresh_token)
        .await
        .context(TokenStorageSnafu)?
    else {
        return OauthSnafu {
            error: "invalid_grant",
            description: "Invalid refresh token",
            status_code: StatusCode::BAD_REQUEST,
        }
        .fail();
    };

    Ok(Json(Token {
        access_token: new_token.access_token,
        expires_in: new_token.expires_in,
        token_type: BEARER.to_owned(),
        scope: None,
        refresh_token: new_token.refresh_token,
    }))
}

async fn new_token(data: web::Data<AppState>, request: GetToken) -> Result<Json<Token>> {
    let Some(username) = request.username else {
        return OauthSnafu {
            error: "invalid_request",
            description: "Missing parameters. \"username\" and \"password\" required",
            status_code: StatusCode::BAD_REQUEST,
        }
        .fail();
    };

    let Some(password) = request.password else {
        return OauthSnafu {
            error: "invalid_request",
            description: "Missing parameters. \"username\" and \"password\" required",
            status_code: StatusCode::BAD_REQUEST,
        }
        .fail();
    };

    let Some(client_id) = request.client_id else {
        return OauthSnafu {
            error: "invalid_client",
            description: "Client id was not found in the headers or body",
            status_code: StatusCode::BAD_REQUEST,
        }
        .fail();
    };

    let Some(client_secret) = request.client_secret else {
        return OauthSnafu {
            error: "invalid_client",
            description: "The client credentials are invalid",
            status_code: StatusCode::BAD_REQUEST,
        }
        .fail();
    };

    let mut tx = data.pool.begin().await?;

    let Some(user_row) = find_user(&mut *tx, &username, &password).await? else {
        return OauthSnafu {
            error: "invalid_grant",
            description: "Invalid username and password combination",
            status_code: StatusCode::BAD_REQUEST,
        }
        .fail();
    };

    let Some(client_row) = clients::find_by_user_id_client_id_and_secret(
        &mut *tx,
        user_row.id,
        &client_id,
        &client_secret,
    )
    .await?
    else {
        return OauthSnafu {
            error: "invalid_client",
            description: "The client credentials are invalid",
            status_code: StatusCode::BAD_REQUEST,
        }
        .fail();
    };

    tx.commit().await?;

    let new_token = data
        .token_storage
        .new_token(&data.pool, user_row.id, client_row.id)
        .await
        .context(TokenStorageSnafu)?;

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
) -> std::result::Result<ServiceRequest, (actix_web::Error, ServiceRequest)> {
    let Some(credentials) = credentials else {
        return Err((
            OauthSnafu {
                error: "access_denied",
                description: "OAuth2 authentication required",
                status_code: StatusCode::UNAUTHORIZED,
            }
            .build()
            .into(),
            req,
        ));
    };

    let token_storage = &req
        .app_data::<web::Data<AppState>>()
        .expect("App data for the request is not configured properly")
        .token_storage;

    match token_storage
        .validate(credentials.token())
        .context(TokenStorageSnafu)
    {
        Ok(Some(claim)) => {
            req.extensions_mut().insert(UserInfo {
                user_id: claim.user_id,
                client_id: claim.client_id,
            });

            Ok(req)
        }
        Ok(None) => Err((
            OauthSnafu {
                error: "invalid_grant",
                description: "The access token provided is invalid",
                status_code: StatusCode::UNAUTHORIZED,
            }
            .build()
            .into(),
            req,
        )),
        Err(err) => Err((err.into(), req)),
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
}
