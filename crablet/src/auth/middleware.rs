use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::cookie::CookieJar;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use crate::auth::UserContext;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String, // User ID
    exp: usize,
    username: Option<String>,
    roles: Option<Vec<String>>,
    tenant_id: Option<String>,
}

pub async fn auth_middleware(
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
        // Validate token
        // Use secret from environment variable, otherwise fallback to "secret"
        // But in production, we should enforce a secure secret.
        let secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
            if cfg!(debug_assertions) {
                tracing::warn!("JWT_SECRET not set, using default 'secret' for development.");
                "secret".to_string()
            } else {
                tracing::error!("JWT_SECRET not set in production! Generating random secret.");
                // Generate a random secret to prevent crashing, but invalidating all existing tokens
                use rand::Rng;
                use base64::prelude::*;
                let random_bytes: Vec<u8> = (0..32).map(|_| rand::thread_rng().gen()).collect();
                BASE64_STANDARD.encode(&random_bytes)
            }
        });
        
        match decode::<Claims>(
            &token,
            &DecodingKey::from_secret(secret.as_bytes()),
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
