use serde::{Deserialize, Serialize};
use std::sync::Arc;
use dashmap::DashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuthMode {
    Off,
    Local,
    Token,
    ApiKey,
    MTLS,
}

#[derive(Clone)]
pub struct AuthManager {
    mode: AuthMode,
    api_keys: Arc<DashMap<String, String>>, // Key -> UserID
    tokens: Arc<DashMap<String, String>>,   // Token -> UserID
}

impl AuthManager {
    pub fn new(mode: AuthMode) -> Self {
        Self {
            mode,
            api_keys: Arc::new(DashMap::new()),
            tokens: Arc::new(DashMap::new()),
        }
    }

    pub fn validate_token(&self, token: &str) -> Option<String> {
        match self.mode {
            AuthMode::Off => Some("anonymous".to_string()),
            AuthMode::Local => Some("local_user".to_string()),
            AuthMode::Token => self.tokens.get(token).map(|v| v.value().clone()),
            AuthMode::ApiKey => self.api_keys.get(token).map(|v| v.value().clone()),
            AuthMode::MTLS => None, // mTLS handled by TLS layer
        }
    }

    pub fn add_api_key(&self, key: String, user_id: String) {
        self.api_keys.insert(key, user_id);
    }

    pub fn add_token(&self, token: String, user_id: String) {
        self.tokens.insert(token, user_id);
    }
}
