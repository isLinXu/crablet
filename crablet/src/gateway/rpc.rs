use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use serde_json::Value;
use super::types::{RpcRequest, RpcResponse, RpcError};
use tokio::sync::RwLock;

// Define a type for async RPC handlers
// Handlers take optional params and return a Future that resolves to a Result
type RpcHandler = Box<dyn Fn(Option<Value>) -> Pin<Box<dyn Future<Output = Result<Option<Value>, RpcError>> + Send>> + Send + Sync>;

#[derive(Clone)]
pub struct RpcDispatcher {
    handlers: Arc<RwLock<HashMap<String, RpcHandler>>>,
}

impl Default for RpcDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl RpcDispatcher {
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register<F, Fut>(&self, method: &str, handler: F)
    where
        F: Fn(Option<Value>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Option<Value>, RpcError>> + Send + 'static,
    {
        let mut map = self.handlers.write().await;
        map.insert(method.to_string(), Box::new(move |params| Box::pin(handler(params))));
    }

    pub async fn dispatch(&self, req: RpcRequest) -> RpcResponse {
        let map = self.handlers.read().await;
        if let Some(handler) = map.get(&req.method) {
            match handler(req.params).await {
                Ok(result) => RpcResponse::new(req.id, result, None),
                Err(e) => RpcResponse::new(req.id, None, Some(e)),
            }
        } else {
            RpcResponse::new(req.id, None, Some(RpcError::new(-32601, "Method not found", None)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_dispatch() {
        let dispatcher = RpcDispatcher::new();
        dispatcher.register("echo", |params| async {
            Ok(params)
        }).await;

        let req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "echo".to_string(),
            params: Some(serde_json::json!("hello")),
            id: Some("1".to_string()),
        };
        let resp = dispatcher.dispatch(req).await;
        assert_eq!(resp.id, Some("1".to_string()));
        assert_eq!(resp.result, Some(serde_json::json!("hello")));
        assert!(resp.error.is_none());
    }

    #[tokio::test]
    async fn test_dispatch_unknown_method() {
        let dispatcher = RpcDispatcher::new();
        let req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "nonexistent".to_string(),
            params: None,
            id: Some("1".to_string()),
        };
        let resp = dispatcher.dispatch(req).await;
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, -32601);
    }

    #[tokio::test]
    async fn test_dispatch_handler_error() {
        let dispatcher = RpcDispatcher::new();
        dispatcher.register("fail", |_params| async {
            Err(RpcError::new(-32000, "Custom error", None))
        }).await;

        let req = RpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "fail".to_string(),
            params: None,
            id: Some("2".to_string()),
        };
        let resp = dispatcher.dispatch(req).await;
        assert!(resp.result.is_none());
        assert_eq!(resp.error.unwrap().code, -32000);
    }

    #[tokio::test]
    async fn test_default() {
        let dispatcher = RpcDispatcher::default();
        assert_eq!(dispatcher.handlers.read().await.len(), 0);
    }

    #[test]
    fn test_rpc_response_new() {
        let resp = RpcResponse::new(
            Some("id-1".to_string()),
            Some(serde_json::json!({"key": "value"})),
            None,
        );
        assert_eq!(resp.jsonrpc, "2.0");
        assert_eq!(resp.id, Some("id-1".to_string()));
    }

    #[test]
    fn test_rpc_error_new() {
        let err = RpcError::new(-32600, "Invalid Request", Some(serde_json::json!(null)));
        assert_eq!(err.code, -32600);
        assert_eq!(err.message, "Invalid Request");
    }
}
