use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::Response,
    extract::State,
};
use axum_extra::extract::cookie::CookieJar;
use jsonwebtoken::{decode, DecodingKey, Validation};
use crate::auth::{JwtClaims, UserContext};
use crate::auth::handlers::AuthState;
use std::sync::Arc;

pub async fn auth_middleware(
    State(auth_state): State<Arc<AuthState>>,
    cookie_jar: CookieJar,
    mut req: axum::http::Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = cookie_jar
        .get("access_token")
        .map(|cookie| cookie.value().to_string())
        .or_else(|| {
            req.headers()
                .get("Authorization")
                .and_then(|h| h.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer "))
                .map(|s| s.to_string())
        });

    let context = if let Some(token) = token {
        match decode::<JwtClaims>(
            &token,
            &DecodingKey::from_secret(auth_state.jwt_secret.as_bytes()),
            &Validation::default(),
        ) {
            Ok(token_data) => {
                UserContext {
                    user_id: token_data.claims.sub,
                    username: token_data.claims.username.unwrap_or_else(|| "user".to_string()),
                    email: None,
                    roles: token_data.claims.roles.unwrap_or_default(),
                    tenant_id: token_data.claims.tenant_id,
                }
            }
            Err(_) => {
                // Invalid token
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    } else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    req.extensions_mut().insert(context);
    Ok(next.run(req).await)
}

// Extractor for UserContext
#[async_trait]
impl<S> FromRequestParts<S> for UserContext
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<UserContext>()
            .cloned()
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)
    }
}
