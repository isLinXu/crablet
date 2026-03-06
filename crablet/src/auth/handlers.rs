use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
    Json,
    debug_handler,
};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use crate::auth::oidc::OidcProvider;
use crate::auth::UserContext;
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
                let access_token = token.access_token.clone();
                let cookie = Cookie::build(("access_token", access_token))
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
