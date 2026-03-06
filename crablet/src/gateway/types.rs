use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    pub host: String,
    pub port: u16,
    pub auth_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GatewayError {
    InternalError(String),
    AuthError(String),
    InvalidRequest(String),
    NotFound(String),
}

impl fmt::Display for GatewayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for GatewayError {}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
    pub id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: Option<String>,
    pub result: Option<Value>,
    pub error: Option<RpcError>,
}

impl RpcResponse {
    pub fn new(id: Option<String>, result: Option<Value>, error: Option<RpcError>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result,
            error,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i32, message: &str, data: Option<Value>) -> Self {
        Self {
            code,
            message: message.to_string(),
            data,
        }
    }
}
