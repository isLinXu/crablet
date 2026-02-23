use std::sync::Arc;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde_json::Value;
use crate::plugins::Plugin;
use crate::tools::manager::SkillManagerTool;

pub struct InstallSkillPlugin {
    manager: Arc<SkillManagerTool>,
}

impl InstallSkillPlugin {
    pub fn new(manager: Arc<SkillManagerTool>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Plugin for InstallSkillPlugin {
    fn name(&self) -> &str {
        "install_skill"
    }

    fn description(&self) -> &str {
        "Install a new skill from a Git URL. Args: {\"url\": \"...\", \"name\": \"(optional)\"}"
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let url = args.get("url")
            .and_then(|v| v.as_str())
            .context("Missing 'url' argument")?;
            
        let name = args.get("name").and_then(|v| v.as_str());
        
        // This operation might be slow (git clone), so we wrap it in spawn_blocking if needed,
        // but for now direct call is fine as we are in async context and Tool execution is awaited.
        // However, SkillManagerTool::install_from_git is synchronous (std::process::Command).
        // To avoid blocking the runtime, we should use spawn_blocking.
        
        let manager = self.manager.clone();
        let url = url.to_string();
        let name = name.map(|s| s.to_string());
        
        let url_clone = url.clone();
        let name_clone = name.clone();
        
        tokio::task::spawn_blocking(move || {
            manager.install_from_git(&url_clone, name_clone.as_deref())
        }).await??;
        
        Ok(format!("Successfully installed skill from {}", url))
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct CreateSkillPlugin {
    manager: Arc<SkillManagerTool>,
}

impl CreateSkillPlugin {
    pub fn new(manager: Arc<SkillManagerTool>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Plugin for CreateSkillPlugin {
    fn name(&self) -> &str {
        "create_skill"
    }

    fn description(&self) -> &str {
        "Create a new Python skill. Args: {\"name\": \"...\", \"description\": \"...\", \"code\": \"...\", \"params_json\": \"...\"}"
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let name = args.get("name").and_then(|v| v.as_str()).context("Missing 'name'")?;
        let description = args.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let code = args.get("code").and_then(|v| v.as_str()).context("Missing 'code'")?;
        let params_json = args.get("params_json").and_then(|v| v.as_str()).unwrap_or("{}");

        let manager = self.manager.clone();
        let name = name.to_string();
        let description = description.to_string();
        let code = code.to_string();
        let params_json = params_json.to_string();

        let name_clone = name.clone();
        let description_clone = description.clone();
        let code_clone = code.clone();
        let params_json_clone = params_json.clone();

        tokio::task::spawn_blocking(move || {
            manager.create_python_skill(&name_clone, &description_clone, &code_clone, &params_json_clone)
        }).await??;
        
        Ok(format!("Successfully created skill '{}'", name))
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}
