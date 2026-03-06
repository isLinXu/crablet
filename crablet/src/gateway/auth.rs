use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey, Algorithm};
// use axum::{
//     extract::{Request, State},
//     http::{StatusCode, header},
//     middleware::Next,
//     response::Response,
// };
use chrono::Utc;
use serde::{Deserialize, Serialize};
use moka::future::Cache;
use std::time::Duration;
use std::sync::Arc;
use dashmap::DashMap;
use sqlx::{SqlitePool, Row};
use uuid::Uuid;
use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};
// use tracing::{info, warn};
// use tracing::info;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthMode {
    Off,
    Local,
    Token,
    ApiKey,
    MTLS,
    JWT,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub id: String,
    pub name: String,
    pub key: String, // Truncated or masked
    pub created_at: i64,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id
    pub role: String,
    pub exp: usize,
}

#[derive(Clone)]
pub struct AuthManager {
    mode: AuthMode,
    api_keys: Cache<String, String>, // Key (Plain) -> UserID (Cache with TTL)
    tokens: Arc<DashMap<String, String>>,   // Token -> UserID
    pub pool: Option<SqlitePool>,
    jwt_secret: String,
}

impl AuthManager {
    pub fn new(mode: AuthMode, pool: Option<SqlitePool>) -> Self {
        let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
            if cfg!(debug_assertions) {
                tracing::warn!("JWT_SECRET not set, using default 'crablet-secret-key-change-me' for development.");
                "crablet-secret-key-change-me".to_string()
            } else {
                tracing::error!("JWT_SECRET not set in production! Generating random secret.");
                use rand::Rng;
                use base64::prelude::*;
                let random_bytes: Vec<u8> = (0..32).map(|_| rand::thread_rng().gen()).collect();
                BASE64_STANDARD.encode(&random_bytes)
            }
        });
        
        Self {
            mode,
            api_keys: Cache::builder()
                .max_capacity(1000)
                .time_to_live(Duration::from_secs(300)) // 5 minutes TTL
                .build(),
            tokens: Arc::new(DashMap::new()),
            pool,
            jwt_secret,
        }
    }

    pub fn mode(&self) -> &AuthMode {
        &self.mode
    }
    
    pub fn has_tokens(&self) -> bool {
        !self.tokens.is_empty()
    }
    
    pub fn generate_token(&self, user_id: &str, role: &str) -> String {
        match self.mode {
            AuthMode::JWT => self.generate_jwt(user_id, role).unwrap_or_else(|e| {
                tracing::error!("Failed to generate JWT: {}", e);
                Uuid::new_v4().to_string()
            }),
            _ => {
                let token = Uuid::new_v4().to_string();
                self.tokens.insert(token.clone(), user_id.to_string());
                token
            }
        }
    }
    
    pub fn generate_jwt(&self, user_id: &str, role: &str) -> jsonwebtoken::errors::Result<String> {
        let expiration = Utc::now()
            .checked_add_signed(chrono::Duration::hours(24))
            .expect("valid timestamp")
            .timestamp() as usize;

        let claims = Claims {
            sub: user_id.to_owned(),
            role: role.to_owned(),
            exp: expiration,
        };

        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
    }

    pub fn validate_jwt(&self, token: &str) -> Option<String> {
        let validation = Validation::new(Algorithm::HS256);
        match decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &validation,
        ) {
            Ok(token_data) => Some(token_data.claims.sub),
            Err(_) => None,
        }
    }

    pub fn validate_token(&self, token: &str) -> Option<String> {
        match self.mode {
            AuthMode::Off => Some("anonymous".to_string()),
            AuthMode::Local => Some("local_user".to_string()),
            AuthMode::Token => self.tokens.get(token).map(|v| v.value().clone()),
            AuthMode::JWT => self.validate_jwt(token),
            AuthMode::ApiKey => {
                // Try blocking read from cache if possible, otherwise fail.
                // Moka's get() is async.
                // We should use blocking feature or sync cache if we need sync access.
                // But we are in async context mostly.
                // For MVP, if called sync, we just return None to force async check or fail.
                None
            },
            AuthMode::MTLS => None, 
        }
    }
    
    pub async fn validate_token_async(&self, token: &str) -> Option<String> {
        match self.mode {
            AuthMode::ApiKey => {
                // 1. Fast path: Memory Cache (Moka)
                if let Some(user_id) = self.api_keys.get(token).await {
                    return Some(user_id);
                }
                
                if let Some(pool) = &self.pool {
                    let prefix = if token.len() >= 8 { &token[..8] } else { token };
                    // Query candidates
                    let rows = sqlx::query("SELECT key_hash, user_id FROM api_keys WHERE key_prefix = ? AND status = 'active'")
                        .bind(prefix)
                        .fetch_all(pool)
                        .await
                        .ok()?;
                    
                    for row in rows {
                        let hash: String = row.get("key_hash");
                        let user_id: String = row.get("user_id");
                        
                        let parsed_hash = PasswordHash::new(&hash).ok()?;
                        if Argon2::default().verify_password(token.as_bytes(), &parsed_hash).is_ok() {
                            // Valid! Cache it.
                            self.api_keys.insert(token.to_string(), user_id.clone()).await;
                            return Some(user_id);
                        }
                    }
                }
                None
            },
            AuthMode::JWT => self.validate_jwt(token),
            _ => self.validate_token(token),
        }
    }

    pub async fn load_keys_from_db(&self) -> anyhow::Result<()> {
        // Can't load plain keys anymore.
        // We just clear cache or do nothing.
        Ok(())
    }

    pub async fn create_api_key(&self, name: &str, user_id: &str) -> anyhow::Result<String> {
        let key = format!("sk-{}", Uuid::new_v4()); // Simple key format
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().timestamp();
        
        // Hash
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default().hash_password(key.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("Hashing failed: {}", e))?
            .to_string();
            
        let prefix = &key[..8];
        
        if let Some(pool) = &self.pool {
            sqlx::query("INSERT INTO api_keys (id, key_hash, key_prefix, user_id, name, created_at, status) VALUES (?, ?, ?, ?, ?, ?, 'active')")
                .bind(id)
                .bind(password_hash)
                .bind(prefix)
                .bind(user_id)
                .bind(name)
                .bind(now)
                .execute(pool)
                .await?;
        }
        
        // Cache the new key immediately so it works
        self.api_keys.insert(key.clone(), user_id.to_string()).await;
        Ok(key)
    }

    pub async fn list_api_keys(&self) -> anyhow::Result<Vec<ApiKeyInfo>> {
        if let Some(pool) = &self.pool {
            let rows = sqlx::query("SELECT id, name, key_prefix, created_at, status FROM api_keys WHERE status = 'active' ORDER BY created_at DESC")
                .fetch_all(pool)
                .await?;
            
            let mut keys = Vec::new();
            for row in rows {
                let prefix: String = row.get("key_prefix");
                keys.push(ApiKeyInfo {
                    id: row.get("id"),
                    name: row.get("name"),
                    key: format!("{}...", prefix), // Masked
                    created_at: row.get("created_at"),
                    status: row.get("status"),
                });
            }
            Ok(keys)
        } else {
            Ok(vec![])
        }
    }

    pub async fn revoke_api_key(&self, id: &str) -> anyhow::Result<()> {
        if let Some(pool) = &self.pool {
            sqlx::query("UPDATE api_keys SET status = 'revoked' WHERE id = ?")
                .bind(id)
                .execute(pool)
                .await?;
                
            // Invalidate entire cache to be safe (since we can't map ID -> Token easily without extra index)
            // Or we just rely on TTL (5 mins).
            // For security, strict revocation requires clearing cache.
            self.api_keys.invalidate_all();
        }
        Ok(())
    }
}

