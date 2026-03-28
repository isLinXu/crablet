use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
    Json,
    debug_handler,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use crate::auth::oidc::OidcProvider;
use crate::auth::{JwtClaims, UserContext};
use std::sync::Arc;
use serde::Deserialize;

#[derive(Clone)]
pub struct AuthState {
    pub oidc: Option<OidcProvider>,
    pub jwt_secret: String,
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: String,
    state: Option<String>,
}

#[debug_handler]
pub async fn login_handler(State(state): State<Arc<AuthState>>) -> Response {
    if let Some(oidc) = &state.oidc {
        let auth_url = oidc.get_authorization_url();
        Redirect::temporary(auth_url.as_str()).into_response()
    } else {
        (axum::http::StatusCode::SERVICE_UNAVAILABLE, "OIDC not configured").into_response()
    }
}

pub async fn callback_handler(
    State(state): State<Arc<AuthState>>,
    jar: CookieJar,
    Query(query): Query<CallbackQuery>,
) -> impl IntoResponse {
    if let Some(oidc) = &state.oidc {
        match oidc.exchange_code(&query.code).await {
            Ok(token) => {
                let userinfo = match token
                    .id_token
                    .as_ref()
                    .map(|id_token| id_token.payload())
                    .transpose()
                {
                    Ok(Some(claims)) => claims.userinfo.clone(),
                    Ok(None) => match oidc.request_userinfo(&token).await {
                        Ok(userinfo) => userinfo,
                        Err(e) => {
                            return (
                                axum::http::StatusCode::BAD_REQUEST,
                                format!("Failed to load user info: {}", e),
                            )
                                .into_response();
                        }
                    },
                    Err(e) => {
                        return (
                            axum::http::StatusCode::BAD_REQUEST,
                            format!("Failed to decode ID token: {}", e),
                        )
                            .into_response();
                    }
                };

                let user = UserContext {
                    user_id: userinfo.sub.clone(),
                    username: userinfo
                        .preferred_username
                        .clone()
                        .or_else(|| userinfo.name.clone())
                        .or_else(|| {
                            userinfo
                                .email
                                .as_ref()
                                .and_then(|email| email.split('@').next().map(|value| value.to_string()))
                        })
                        .unwrap_or_else(|| "user".to_string()),
                    email: userinfo.email.clone(),
                    roles: vec!["user".to_string()],
                    tenant_id: None,
                };

                let expiration = Utc::now()
                    .checked_add_signed(chrono::Duration::hours(24))
                    .expect("valid timestamp")
                    .timestamp() as usize;
                let claims = JwtClaims::from_user_context(&user, expiration);
                let session_token = match encode(
                    &Header::default(),
                    &claims,
                    &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
                ) {
                    Ok(token) => token,
                    Err(e) => {
                        return (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Failed to create session token: {}", e),
                        )
                            .into_response();
                    }
                };

                let cookie = Cookie::build(("access_token", session_token))
                    .path("/")
                    .http_only(true)
                    .same_site(axum_extra::extract::cookie::SameSite::Lax)
                    .secure(!cfg!(debug_assertions)) // Secure in release mode
                    .build();
                
                (jar.add(cookie), Redirect::to("/")).into_response()
            }
            Err(e) => {
                (axum::http::StatusCode::BAD_REQUEST, format!("Auth failed: {}", e)).into_response()
            }
        }
    } else {
        (axum::http::StatusCode::SERVICE_UNAVAILABLE, "OIDC not configured").into_response()
    }
}

pub async fn me_handler(user: UserContext) -> Json<UserContext> {
    Json(user)
}
