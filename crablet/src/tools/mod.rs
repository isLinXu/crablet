use async_trait::async_trait;
use serde_json::Value;
use anyhow::Result;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(&self, command: &str, args: Value) -> Result<String>;
}

pub mod bash;
pub mod file;
pub mod http;
pub mod search;
pub mod vision;
pub mod mcp;
pub mod manager;
pub mod management_plugin;
pub mod demo;
pub mod mcp_plugins;
pub mod browser;
