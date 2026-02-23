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
        let mut handlers = self.handlers.write().await;
        handlers.insert(
            method.to_string(),
            Box::new(move |params| Box::pin(handler(params))),
        );
    }

    pub async fn dispatch(&self, request: RpcRequest) -> RpcResponse {
        let handlers = self.handlers.read().await;
        
        let result = if let Some(handler) = handlers.get(&request.method) {
            match handler(request.params).await {
                Ok(res) => Ok(res),
                Err(e) => Err(e),
            }
        } else {
            Err(RpcError {
                code: -32601,
                message: format!("Method not found: {}", request.method),
                data: None,
            })
        };

        match result {
            Ok(val) => RpcResponse {
                jsonrpc: "2.0".to_string(),
                result: val,
                error: None,
                id: request.id,
            },
            Err(e) => RpcResponse {
                jsonrpc: "2.0".to_string(),
                result: None,
                error: Some(e),
                id: request.id,
            },
        }
    }
}
