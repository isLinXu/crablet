use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(&self, command: &str, args: Value) -> Result<String>;
}

pub mod bash;
pub mod browser;
pub mod demo;
pub mod file;
pub mod http;
pub mod management_plugin;
pub mod manager;
pub mod mcp;
pub mod mcp_plugins;
pub mod memory_tools;
pub mod search;
pub mod vision;
