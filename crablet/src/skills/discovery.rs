//! Skill Discovery System
//!
//! Automatically discovers and registers skills from multiple sources:
//! - Local skills directory
//! - MCP servers
//! - Built-in plugins
//! - Remote registries

use anyhow::Result;
use tracing::{info, warn, error};
use crate::skills::{SkillRegistry, SkillTriggerEngine};
use crate::config::Config;

/// Result of skill discovery
#[derive(Debug, Default)]
pub struct DiscoveryResult {
    /// Number of local skills discovered
    pub local_count: usize,
    /// Number of MCP tools discovered
    pub mcp_count: usize,
    /// Number of plugins discovered
    pub plugin_count: usize,
    /// Number of OpenClaw skills discovered
    pub openclaw_count: usize,
    /// Total triggers registered
    pub trigger_count: usize,
    /// Errors encountered during discovery
    pub errors: Vec<String>,
}

impl DiscoveryResult {
    /// Total number of skills discovered
    pub fn total(&self) -> usize {
        self.local_count + self.mcp_count + self.plugin_count + self.openclaw_count
    }
    
    /// Check if discovery was successful (no errors)
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Skill discovery service
pub struct SkillDiscovery;

impl SkillDiscovery {
    /// Discover skills from all configured sources
    pub async fn discover_all(
        registry: &mut SkillRegistry,
        config: &Config,
    ) -> Result<DiscoveryResult> {
        let mut result = DiscoveryResult::default();
        
        info!("Starting skill discovery...");
        
        // 1. Discover local skills
        match Self::discover_local_skills(registry, config).await {
            Ok(count) => {
                result.local_count = count;
                info!("Discovered {} local skills", count);
            }
            Err(e) => {
                let msg = format!("Failed to discover local skills: {}", e);
                error!("{}", msg);
                result.errors.push(msg);
            }
        }
        
        // 2. Discover MCP skills
        match Self::discover_mcp_skills(registry, config).await {
            Ok(count) => {
                result.mcp_count = count;
                info!("Discovered {} MCP tools", count);
            }
            Err(e) => {
                let msg = format!("Failed to discover MCP skills: {}", e);
                error!("{}", msg);
                result.errors.push(msg);
            }
        }
        
        // 3. Discover built-in plugins
        match Self::discover_plugins(registry, config).await {
            Ok(count) => {
                result.plugin_count = count;
                info!("Discovered {} built-in plugins", count);
            }
            Err(e) => {
                let msg = format!("Failed to discover plugins: {}", e);
                error!("{}", msg);
                result.errors.push(msg);
            }
        }
        
        info!(
            "Skill discovery complete: {} total skills ({} local, {} MCP, {} plugins, {} OpenClaw)",
            result.total(),
            result.local_count,
            result.mcp_count,
            result.plugin_count,
            result.openclaw_count
        );
        
        Ok(result)
    }
    
    /// Discover local skills from the skills directory
    async fn discover_local_skills(
        registry: &mut SkillRegistry,
        config: &Config,
    ) -> Result<usize> {
        let skills_dir = &config.skills_dir;
        
        if !skills_dir.exists() {
            info!("Skills directory does not exist: {:?}", skills_dir);
            return Ok(0);
        }
        
        let initial_count = registry.len();
        registry.load_from_dir(skills_dir).await?;
        let final_count = registry.len();
        
        Ok(final_count - initial_count)
    }
    
    /// Discover skills from MCP servers
    async fn discover_mcp_skills(
        registry: &mut SkillRegistry,
        config: &Config,
    ) -> Result<usize> {
        let mut total_tools = 0;
        
        for (server_name, server_config) in &config.mcp_servers {
            match Self::connect_mcp_server(registry, server_name, server_config).await {
                Ok(count) => {
                    info!("Connected to MCP server '{}': {} tools", server_name, count);
                    total_tools += count;
                }
                Err(e) => {
                    warn!("Failed to connect to MCP server '{}': {}", server_name, e);
                }
            }
        }
        
        Ok(total_tools)
    }
    
    /// Connect to a single MCP server and register its tools
    async fn connect_mcp_server(
        registry: &mut SkillRegistry,
        server_name: &str,
        server_config: &crate::config::McpServerConfig,
    ) -> Result<usize> {
        use crate::tools::mcp::McpClient;
        
        info!("Connecting to MCP server: {}", server_name);
        
        let client = McpClient::new(&server_config.command, &server_config.args).await?;
        let client_arc = std::sync::Arc::new(client);
        
        // Register tools
        let tools = client_arc.list_tools().await?;
        let mut tool_count = 0;
        
        for tool in tools {
            registry.register_mcp_tool(
                tool.name.clone(),
                client_arc.clone(),
                tool.description.clone(),
                tool.input_schema.clone(),
            );
            tool_count += 1;
        }
        
        // Register resources
        if let Ok(resources) = client_arc.list_resources().await {
            for resource in resources {
                registry.register_mcp_resource(resource, client_arc.clone());
            }
        }
        
        // Register prompts
        if let Ok(prompts) = client_arc.list_prompts().await {
            for prompt in prompts {
                registry.register_mcp_prompt(prompt, client_arc.clone());
            }
        }
        
        Ok(tool_count)
    }
    
    /// Discover built-in plugins
    async fn discover_plugins(
        _registry: &mut SkillRegistry,
        _config: &Config,
    ) -> Result<usize> {
        // Built-in plugins are registered at compile time
        // This is a placeholder for dynamic plugin discovery in the future
        Ok(0)
    }
    
    /// Build a trigger engine from the registry
    pub fn build_trigger_engine(registry: &SkillRegistry) -> SkillTriggerEngine {
        let mut engine = SkillTriggerEngine::new();
        
        for manifest in registry.list_skills() {
            // Register explicit triggers from manifest
            for trigger in &manifest.triggers {
                engine.register(manifest.name.clone(), trigger.clone());
            }
            
            // Auto-generate triggers if none defined
            if manifest.triggers.is_empty() {
                let auto_triggers = Self::generate_triggers(&manifest);
                for trigger in auto_triggers {
                    engine.register(manifest.name.clone(), trigger);
                }
            }
        }
        
        engine
    }
    
    /// Generate default triggers for a skill
    fn generate_triggers(manifest: &crate::skills::SkillManifest) -> Vec<crate::skills::SkillTrigger> {
        use crate::skills::SkillTrigger;
        
        let mut triggers = Vec::new();
        
        // 1. Command trigger from skill name
        triggers.push(SkillTrigger::Command {
            prefix: format!("/{}", manifest.name.to_lowercase()),
            args_schema: Some(manifest.parameters.clone()),
        });
        
        // 2. Keyword trigger from description
        let keywords: Vec<String> = manifest.description
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .take(5)
            .map(|w| w.to_lowercase().trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| !w.is_empty())
            .collect();
        
        if !keywords.is_empty() {
            triggers.push(SkillTrigger::Keyword {
                keywords,
                case_sensitive: false,
            });
        }
        
        // 3. Semantic trigger
        triggers.push(SkillTrigger::Semantic {
            description: manifest.description.clone(),
            threshold: 0.75,
        });
        
        triggers
    }
    
    /// Refresh skills from all sources
    pub async fn refresh(
        registry: &mut SkillRegistry,
        config: &Config,
    ) -> Result<DiscoveryResult> {
        info!("Refreshing skills...");
        
        // Clear existing skills
        registry.clear();
        
        // Re-discover
        Self::discover_all(registry, config).await
    }
}

/// Skill discovery watcher for file system changes
pub struct SkillDiscoveryWatcher {
    skills_dir: std::path::PathBuf,
}

impl SkillDiscoveryWatcher {
    /// Create a new watcher
    pub fn new(skills_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            skills_dir: skills_dir.into(),
        }
    }
    
    /// Start watching for changes
    pub async fn start(self, _registry: std::sync::Arc<tokio::sync::RwLock<SkillRegistry>>) -> Result<()> {
        info!("Starting skill discovery watcher for: {:?}", self.skills_dir);
        
        // TODO: Implement file system watching using notify crate
        // For now, this is a placeholder
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{SkillManifest, SkillTrigger};

    #[test]
    fn test_generate_triggers() {
        let manifest = SkillManifest {
            name: "weather".to_string(),
            description: "Get weather information for any location".to_string(),
            version: "1.0.0".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                }
            }),
            entrypoint: "python main.py".to_string(),
            env: std::collections::HashMap::new(),
            requires: vec![],
            runtime: Some("python3".to_string()),
            dependencies: None,
            resources: None,
            permissions: vec![],
            conflicts: vec![],
            min_crablet_version: None,
            author: None,
            triggers: vec![],
        };
        
        let triggers = SkillDiscovery::generate_triggers(&manifest);
        
        assert_eq!(triggers.len(), 3);
        
        // Check command trigger
        match &triggers[0] {
            SkillTrigger::Command { prefix, .. } => {
                assert_eq!(prefix, "/weather");
            }
            _ => panic!("Expected Command trigger"),
        }
        
        // Check keyword trigger
        match &triggers[1] {
            SkillTrigger::Keyword { keywords, .. } => {
                assert!(keywords.contains(&"weather".to_string()));
                assert!(keywords.contains(&"information".to_string()));
            }
            _ => panic!("Expected Keyword trigger"),
        }
        
        // Check semantic trigger
        match &triggers[2] {
            SkillTrigger::Semantic { description, threshold } => {
                assert_eq!(description, "Get weather information for any location");
                assert_eq!(*threshold, 0.75);
            }
            _ => panic!("Expected Semantic trigger"),
        }
    }

    #[test]
    fn test_discovery_result() {
        let result = DiscoveryResult {
            local_count: 5,
            mcp_count: 3,
            plugin_count: 2,
            openclaw_count: 1,
            trigger_count: 20,
            errors: vec![],
        };
        
        assert_eq!(result.total(), 11);
        assert!(result.is_success());
        
        let result_with_errors = DiscoveryResult {
            errors: vec!["Some error".to_string()],
            ..result
        };
        
        assert!(!result_with_errors.is_success());
    }
}
