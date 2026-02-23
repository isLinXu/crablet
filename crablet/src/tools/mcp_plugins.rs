use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use crate::plugins::Plugin;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::skills::SkillRegistry;

pub struct McpResourcePlugin {
    registry: Arc<RwLock<SkillRegistry>>,
}

impl McpResourcePlugin {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Plugin for McpResourcePlugin {
    fn name(&self) -> &str {
        "read_mcp_resource"
    }

    fn description(&self) -> &str {
        "Read content of an MCP resource. Args: { \"uri\": \"resource_uri\" }"
    }

    async fn initialize(&mut self) -> Result<()> { Ok(()) }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let uri = args.get("uri").and_then(|v| v.as_str()).unwrap_or("");
        if uri.is_empty() {
            return Err(anyhow::anyhow!("Missing 'uri' argument"));
        }
        
        let registry = self.registry.read().await;
        registry.read_resource(uri).await
    }

    async fn shutdown(&mut self) -> Result<()> { Ok(()) }
}

pub struct McpPromptPlugin {
    registry: Arc<RwLock<SkillRegistry>>,
}

impl McpPromptPlugin {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Plugin for McpPromptPlugin {
    fn name(&self) -> &str {
        "get_mcp_prompt"
    }

    fn description(&self) -> &str {
        "Get an MCP prompt template. Args: { \"name\": \"prompt_name\", \"arguments\": { ... } }"
    }

    async fn initialize(&mut self) -> Result<()> { Ok(()) }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.is_empty() {
            return Err(anyhow::anyhow!("Missing 'name' argument"));
        }
        
        let arguments = args.get("arguments").cloned();
        
        let registry = self.registry.read().await;
        registry.get_prompt(name, arguments).await
    }

    async fn shutdown(&mut self) -> Result<()> { Ok(()) }
}
