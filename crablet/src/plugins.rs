use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait Plugin: Send + Sync {
    /// Unique name of the plugin
    fn name(&self) -> &str;
    
    /// Description of what the plugin does
    fn description(&self) -> &str;
    
    /// Initialize the plugin
    async fn initialize(&mut self) -> Result<()>;
    
    /// Execute a command provided by the plugin
    async fn execute(&self, command: &str, args: Value) -> Result<String>;
    
    /// Clean up resources
    async fn shutdown(&mut self) -> Result<()>;
}

pub struct PluginManager {
    plugins: std::collections::HashMap<String, Box<dyn Plugin>>,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        self.plugins.insert(plugin.name().to_string(), plugin);
    }

    pub fn get(&self, name: &str) -> Option<&dyn Plugin> {
        self.plugins.get(name).map(|b| b.as_ref())
    }
}
