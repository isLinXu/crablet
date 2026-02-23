use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::mpsc;
use serde::Serialize;
use uuid::Uuid;
use crate::gateway::types::GatewayError;

#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub connected_at: chrono::DateTime<chrono::Utc>,
    #[serde(skip)]
    pub sender: Option<mpsc::UnboundedSender<super::types::RpcResponse>>,
}

#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<DashMap<String, Session>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
        }
    }

    pub fn create_session(&self, user_id: String, sender: mpsc::UnboundedSender<super::types::RpcResponse>) -> String {
        let session_id = Uuid::new_v4().to_string();
        let session = Session {
            id: session_id.clone(),
            user_id,
            connected_at: chrono::Utc::now(),
            sender: Some(sender),
        };
        self.sessions.insert(session_id.clone(), session);
        session_id
    }

    pub fn remove_session(&self, session_id: &str) {
        self.sessions.remove(session_id);
    }

    pub fn get_session(&self, session_id: &str) -> Option<Session> {
        self.sessions.get(session_id).map(|s| s.value().clone())
    }

    pub fn send_to_session(&self, session_id: &str, response: super::types::RpcResponse) -> Result<(), GatewayError> {
        if let Some(session) = self.sessions.get(session_id) {
            if let Some(sender) = &session.sender {
                sender.send(response).map_err(|e| GatewayError::InternalError(e.to_string()))?;
                return Ok(());
            }
        }
        Err(GatewayError::NotFound(format!("Session {} not found", session_id)))
    }

    pub fn broadcast(&self, response: super::types::RpcResponse) {
        for session in self.sessions.iter() {
            if let Some(sender) = &session.sender {
                let _ = sender.send(response.clone());
            }
        }
    }
}
