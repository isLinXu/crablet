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
