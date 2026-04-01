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
    pub sender: Option<mpsc::Sender<super::types::RpcResponse>>,
}

#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<DashMap<String, Session>>,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
        }
    }

    pub fn create_session(&self, user_id: String, sender: mpsc::Sender<super::types::RpcResponse>) -> String {
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

    pub async fn send_to_session(&self, session_id: &str, response: super::types::RpcResponse) -> Result<(), GatewayError> {
        if let Some(session) = self.sessions.get(session_id) {
            if let Some(sender) = &session.sender {
                sender.send(response).await.map_err(|e| GatewayError::InternalError(e.to_string()))?;
                return Ok(());
            }
        }
        Err(GatewayError::NotFound(format!("Session {} not found", session_id)))
    }

    pub async fn broadcast(&self, response: super::types::RpcResponse) {
        for session in self.sessions.iter() {
            if let Some(sender) = &session.sender {
                let _ = sender.send(response.clone()).await;
            }
        }
    }

    pub fn count(&self) -> usize {
        self.sessions.len()
    }

    pub fn list_sessions(&self) -> Vec<Session> {
        // Collect sessions into a Vec
        // DashMap iterator might deadlock if not careful, but map+collect is usually safe
        // We clone the sessions to avoid holding locks
        self.sessions.iter().map(|s| s.value().clone()).collect()
    }

    pub fn get_history(&self, _session_id: &str) -> Option<Vec<serde_json::Value>> {
        // Placeholder for session history
        // In a real implementation, this would query the episodic memory or a separate history store
        Some(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    fn make_session_manager() -> (SessionManager, mpsc::Sender<super::super::types::RpcResponse>, mpsc::Receiver<super::super::types::RpcResponse>) {
        let mgr = SessionManager::new();
        let (tx, rx) = mpsc::channel(16);
        (mgr, tx, rx)
    }

    #[tokio::test]
    async fn test_create_and_get_session() {
        let (mgr, tx, _rx) = make_session_manager();
        let id = mgr.create_session("user-1".to_string(), tx);
        let session = mgr.get_session(&id).expect("session should exist");
        assert_eq!(session.user_id, "user-1");
        assert_eq!(session.id, id);
    }

    #[tokio::test]
    async fn test_remove_session() {
        let (mgr, tx, _rx) = make_session_manager();
        let id = mgr.create_session("user-1".to_string(), tx);
        mgr.remove_session(&id);
        assert!(mgr.get_session(&id).is_none());
    }

    #[tokio::test]
    async fn test_send_to_session() {
        let (mgr, tx, mut rx) = make_session_manager();
        let id = mgr.create_session("user-1".to_string(), tx);
        let response = super::super::types::RpcResponse::new(
            Some("1".to_string()),
            Some(serde_json::json!("hello")),
            None,
        );
        mgr.send_to_session(&id, response).await.expect("send");
        let received = rx.recv().await.expect("receive");
        assert_eq!(received.id, Some("1".to_string()));
    }

    #[tokio::test]
    async fn test_send_to_nonexistent_session() {
        let mgr = SessionManager::new();
        let response = super::super::types::RpcResponse::new(
            Some("1".to_string()),
            None,
            None,
        );
        let result = mgr.send_to_session("fake-id", response).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let (mgr, tx1, _) = make_session_manager();
        let (_, tx2, _) = make_session_manager();

        let id1 = mgr.create_session("user-1".to_string(), tx1);
        let id2 = mgr.create_session("user-2".to_string(), tx2);

        let sessions = mgr.list_sessions();
        assert_eq!(sessions.len(), 2);

        let ids: Vec<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&id1.as_str()));
        assert!(ids.contains(&id2.as_str()));
    }

    #[tokio::test]
    async fn test_count() {
        let (mgr, tx, _) = make_session_manager();
        assert_eq!(mgr.count(), 0);
        mgr.create_session("user-1".to_string(), tx);
        assert_eq!(mgr.count(), 1);
    }

    #[test]
    fn test_default() {
        let mgr = SessionManager::default();
        assert_eq!(mgr.count(), 0);
    }

    #[tokio::test]
    async fn test_get_history() {
        let mgr = SessionManager::new();
        let history = mgr.get_history("any-id");
        assert!(history.is_some());
        assert!(history.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_broadcast() {
        let (mgr, tx1, _) = make_session_manager();
        let (_, tx2, mut rx2) = make_session_manager();

        mgr.create_session("u1".to_string(), tx1);
        mgr.create_session("u2".to_string(), tx2);

        let response = super::super::types::RpcResponse::new(
            Some("1".to_string()),
            Some(serde_json::json!("broadcast")),
            None,
        );
        mgr.broadcast(response).await;

        let received = rx2.recv().await.expect("receive broadcast");
        assert_eq!(received.result, Some(serde_json::json!("broadcast")));
    }
}
