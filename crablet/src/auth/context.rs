use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserContext {
    pub user_id: String,
    pub username: String,
    pub email: Option<String>,
    pub roles: Vec<String>,
    pub tenant_id: Option<String>,
}

impl UserContext {
    pub fn anonymous() -> Self {
        Self {
            user_id: "anonymous".to_string(),
            username: "Anonymous".to_string(),
            email: None,
            roles: vec!["guest".to_string()],
            tenant_id: None,
        }
    }

    pub fn is_admin(&self) -> bool {
        self.roles.contains(&"admin".to_string())
    }
}
