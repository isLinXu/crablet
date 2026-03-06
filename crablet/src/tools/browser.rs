use async_trait::async_trait;
use serde_json::Value;
// use tracing::{info, warn};
use anyhow::Result;
use crate::plugins::Plugin;

pub struct BrowserPlugin;

#[async_trait]
impl Plugin for BrowserPlugin {
    fn name(&self) -> &str {
        "browser"
    }

    fn description(&self) -> &str {
        "A headless browser tool for interacting with web pages (Not implemented yet)."
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, _args: Value) -> Result<String> {
        Ok("Browser tool is not yet implemented. Use 'search' for information retrieval.".to_string())
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}
