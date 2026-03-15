//! L3: TOOLS Layer - Dynamic Tools
//!
//! The TOOLS layer manages dynamic tools and skills that can be loaded,
//! unloaded, and invoked at runtime. This layer provides:
//! - Tool registration and discovery
//! - Permission management
//! - Tool chain orchestration
//! - Hot loading/unloading of skills

use std::collections::HashMap;
use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{info, debug, warn, error};

use crate::memory::fusion::MemoryError;

/// Tool definition from config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub category: String,
    pub parameters: serde_json::Value,
}

/// Tool permission from config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPermission {
    pub tool: Option<String>,
    pub category: Option<String>,
    pub level: String,
}

/// Tool chain step config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChainStepConfig {
    pub name: String,
    pub tool: String,
    pub param_mapping: HashMap<String, String>,
    pub condition: Option<String>,
    pub continue_on_error: bool,
}

/// Tool chain config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChainConfig {
    pub name: String,
    pub description: String,
    pub steps: Vec<ToolChainStepConfig>,
}

/// Tools configuration (local definition)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    pub available_tools: Vec<ToolDefinition>,
    pub permissions: Vec<ToolPermission>,
    pub tool_chains: Vec<ToolChainConfig>,
}

/// L3 TOOLS Layer - Dynamic tool management
pub struct ToolsLayer {
    /// Tool registry - all available tools
    registry: DashMap<String, Arc<dyn Tool>>,
    
    /// Tool definitions from config
    definitions: RwLock<HashMap<String, ToolDefinition>>
    ,
    
    /// Permission settings
    permissions: RwLock<ToolPermissions>,
    
    /// Tool chains - predefined sequences
    chains: DashMap<String, ToolChain>,
    
    /// Usage statistics
    stats: RwLock<ToolStats>,
}

/// Tool trait - all tools must implement this
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    /// Get tool name
    fn name(&self) -> &str;
    
    /// Get tool description
    fn description(&self) -> &str;
    
    /// Get tool parameters schema
    fn parameters_schema(&self) -> serde_json::Value;
    
    /// Execute the tool
    async fn execute(&self, params: serde_json::Value) -> Result<ToolResult, ToolError>;
    
    /// Check if tool is available
    fn is_available(&self) -> bool;
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Success status
    pub success: bool,
    
    /// Result data
    pub data: serde_json::Value,
    
    /// Execution metadata
    pub metadata: ToolExecutionMetadata,
}

/// Tool execution metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionMetadata {
    /// Tool name
    pub tool_name: String,
    
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    
    /// Timestamp
    pub timestamp: String,
    
    /// Token usage (if applicable)
    pub token_usage: Option<usize>,
}

/// Tool error
#[derive(Debug, Clone)]
pub enum ToolError {
    /// Tool not found
    NotFound(String),
    /// Invalid parameters
    InvalidParameters(String),
    /// Execution failed
    ExecutionFailed(String),
    /// Permission denied
    PermissionDenied(String),
    /// Tool unavailable
    Unavailable(String),
    /// Timeout
    Timeout,
    /// Internal error
    Internal(String),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolError::NotFound(name) => write!(f, "Tool not found: {}", name),
            ToolError::InvalidParameters(msg) => write!(f, "Invalid parameters: {}", msg),
            ToolError::ExecutionFailed(msg) => write!(f, "Execution failed: {}", msg),
            ToolError::PermissionDenied(msg) => write!(f, "Permission denied: {}", msg),
            ToolError::Unavailable(msg) => write!(f, "Tool unavailable: {}", msg),
            ToolError::Timeout => write!(f, "Tool execution timed out"),
            ToolError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for ToolError {}

/// Tool permissions
#[derive(Debug, Clone, Default)]
pub struct ToolPermissions {
    /// Default permission level
    pub default_level: PermissionLevel,
    
    /// Tool-specific permissions
    pub tool_permissions: HashMap<String, PermissionLevel>,
    
    /// Category permissions
    pub category_permissions: HashMap<String, PermissionLevel>,
}

/// Permission level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionLevel {
    /// No access
    Denied,
    /// Read-only access
    ReadOnly,
    /// Standard access
    Standard,
    /// Elevated access
    Elevated,
    /// Full access
    Full,
}

impl Default for PermissionLevel {
    fn default() -> Self {
        PermissionLevel::Standard
    }
}

/// Tool chain - sequence of tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChain {
    /// Chain name
    pub name: String,
    
    /// Chain description
    pub description: String,
    
    /// Steps in the chain
    pub steps: Vec<ToolChainStep>,
}

/// Tool chain step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChainStep {
    /// Step name
    pub name: String,
    
    /// Tool to invoke
    pub tool: String,
    
    /// Parameter mapping (input -> tool param)
    pub param_mapping: HashMap<String, String>,
    
    /// Condition for execution
    pub condition: Option<String>,
    
    /// Whether to continue on error
    pub continue_on_error: bool,
}

/// Tool statistics
#[derive(Debug, Clone, Default)]
pub struct ToolStats {
    /// Total invocations
    pub total_invocations: u64,
    
    /// Successful invocations
    pub successful_invocations: u64,
    
    /// Failed invocations
    pub failed_invocations: u64,
    
    /// Tool-specific stats
    pub tool_stats: HashMap<String, ToolSpecificStats>,
}

/// Tool-specific statistics
#[derive(Debug, Clone, Default)]
pub struct ToolSpecificStats {
    /// Invocation count
    pub invocations: u64,
    
    /// Average execution time (ms)
    pub avg_duration_ms: f64,
    
    /// Success rate (0.0 - 1.0)
    pub success_rate: f64,
    
    /// Last used timestamp
    pub last_used: Option<String>,
}

impl ToolsLayer {
    /// Initialize tools layer from configuration
    pub async fn from_config(config: &ToolsConfig) -> Result<Self, MemoryError> {
        info!("Initializing TOOLS layer...");
        
        let layer = Self {
            registry: DashMap::new(),
            definitions: RwLock::new(HashMap::new()),
            permissions: RwLock::new(ToolPermissions::default()),
            chains: DashMap::new(),
            stats: RwLock::new(ToolStats::default()),
        };
        
        // Load tool definitions from config
        {
            let mut defs = layer.definitions.write().await;
            for tool_def in &config.available_tools {
                defs.insert(tool_def.name.clone(), tool_def.clone());
                debug!("Registered tool definition: {}", tool_def.name);
            }
        }
        
        // Set up permissions
        {
            let mut perms = layer.permissions.write().await;
            perms.default_level = PermissionLevel::Standard;
            
            for perm in &config.permissions {
                let level = match perm.level.as_str() {
                    "denied" => PermissionLevel::Denied,
                    "read_only" => PermissionLevel::ReadOnly,
                    "standard" => PermissionLevel::Standard,
                    "elevated" => PermissionLevel::Elevated,
                    "full" => PermissionLevel::Full,
                    _ => PermissionLevel::Standard,
                };
                
                if let Some(ref tool) = perm.tool {
                    let tool_name: String = tool.clone();
                    perms.tool_permissions.insert(tool_name, level);
                }
                
                if let Some(ref category) = perm.category {
                    let cat_name: String = category.clone();
                    perms.category_permissions.insert(cat_name, level);
                }
            }
        }
        
        // Load built-in tools
        layer.load_builtin_tools().await?;
        
        // Load tool chains
        for chain in &config.tool_chains {
            let tool_chain = ToolChain {
                name: chain.name.clone(),
                description: chain.description.clone(),
                steps: chain.steps.iter().map(|s| ToolChainStep {
                    name: s.name.clone(),
                    tool: s.tool.clone(),
                    param_mapping: s.param_mapping.clone(),
                    condition: s.condition.clone(),
                    continue_on_error: s.continue_on_error,
                }).collect(),
            };
            layer.chains.insert(chain.name.clone(), tool_chain);
            debug!("Registered tool chain: {}", chain.name);
        }
        
        let tool_count = layer.registry.len();
        let chain_count = layer.chains.len();
        
        info!(
            "TOOLS layer initialized: {} tools, {} chains",
            tool_count, chain_count
        );
        
        Ok(layer)
    }
    
    /// Load built-in tools
    async fn load_builtin_tools(&self) -> Result<(), MemoryError> {
        // These would be actual tool implementations
        // For now, we just register placeholder tools
        
        // Example: File system tool
        self.register_tool(Arc::new(BuiltinTool {
            name: "file_read".to_string(),
            description: "Read file contents".to_string(),
            category: "filesystem".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "limit": {"type": "integer"}
                },
                "required": ["path"]
            }),
        })).await?;
        
        // Example: Web search tool
        self.register_tool(Arc::new(BuiltinTool {
            name: "web_search".to_string(),
            description: "Search the web".to_string(),
            category: "web".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer"}
                },
                "required": ["query"]
            }),
        })).await?;
        
        // Example: Memory tool
        self.register_tool(Arc::new(BuiltinTool {
            name: "memory_search".to_string(),
            description: "Search long-term memory".to_string(),
            category: "memory".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer"}
                },
                "required": ["query"]
            }),
        })).await?;
        
        Ok(())
    }
    
    /// Register a tool
    pub async fn register_tool(&self, tool: Arc<dyn Tool>) -> Result<(), MemoryError> {
        let name = tool.name().to_string();
        
        // Check permission
        if !self.check_permission(&name, PermissionLevel::Standard).await {
            return Err(MemoryError::LayerError(format!(
                "Permission denied for tool: {}", name
            )));
        }
        
        self.registry.insert(name.clone(), tool);
        debug!("Registered tool: {}", name);
        
        Ok(())
    }
    
    /// Unregister a tool
    pub fn unregister_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let removed = self.registry.remove(name).map(|(_, tool)| tool);
        if removed.is_some() {
            debug!("Unregistered tool: {}", name);
        }
        removed
    }
    
    /// Get a tool by name
    pub fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.registry.get(name).map(|t| t.clone())
    }
    
    /// List all available tools
    pub fn list_tools(&self) -> Vec<ToolInfo> {
        self.registry
            .iter()
            .map(|entry| {
                let tool = entry.value();
                ToolInfo {
                    name: tool.name().to_string(),
                    description: tool.description().to_string(),
                    available: tool.is_available(),
                }
            })
            .collect()
    }
    
    /// Invoke a tool
    pub async fn invoke(&self, name: &str, params: serde_json::Value) -> Result<ToolResult, ToolError> {
        let start = std::time::Instant::now();
        
        // Get tool
        let tool = self.get_tool(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;
        
        // Check availability
        if !tool.is_available() {
            return Err(ToolError::Unavailable(name.to_string()));
        }
        
        // Check permission
        if !self.check_permission(name, PermissionLevel::Standard).await {
            return Err(ToolError::PermissionDenied(name.to_string()));
        }
        
        debug!("Invoking tool: {} with params: {:?}", name, params);
        
        // Execute with timeout
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            tool.execute(params)
        ).await;
        
        let duration_ms = start.elapsed().as_millis() as u64;
        
        match result {
            Ok(Ok(tool_result)) => {
                // Update stats
                self.update_stats(name, true, duration_ms).await;
                
                info!("Tool {} executed successfully in {}ms", name, duration_ms);
                Ok(tool_result)
            }
            Ok(Err(e)) => {
                // Update stats
                self.update_stats(name, false, duration_ms).await;
                
                warn!("Tool {} failed: {}", name, e);
                Err(e)
            }
            Err(_) => {
                // Update stats
                self.update_stats(name, false, duration_ms).await;
                
                error!("Tool {} timed out after {}ms", name, duration_ms);
                Err(ToolError::Timeout)
            }
        }
    }
    
    /// Execute a tool chain
    pub async fn execute_chain(
        &self,
        chain_name: &str,
        initial_input: serde_json::Value,
    ) -> Result<ToolChainResult, ToolError> {
        let chain = self.chains
            .get(chain_name)
            .ok_or_else(|| ToolError::NotFound(format!("Chain: {}", chain_name)))?;
        
        info!("Executing tool chain: {} ({} steps)", chain_name, chain.steps.len());
        
        let mut results = Vec::new();
        let mut current_input = initial_input;
        
        for (idx, step) in chain.steps.iter().enumerate() {
            debug!("Chain step {}/{}: {}", idx + 1, chain.steps.len(), step.name);
            
            // Map parameters
            let params = self.map_chain_params(&current_input, &step.param_mapping);
            
            // Execute tool
            match self.invoke(&step.tool, params).await {
                Ok(result) => {
                    results.push(ChainStepResult {
                        step_name: step.name.clone(),
                        tool_name: step.tool.clone(),
                        success: true,
                        result: result.data.clone(),
                    });
                    
                    // Update input for next step
                    current_input = result.data;
                }
                Err(e) => {
                    results.push(ChainStepResult {
                        step_name: step.name.clone(),
                        tool_name: step.tool.clone(),
                        success: false,
                        result: serde_json::json!({"error": e.to_string()}),
                    });
                    
                    if !step.continue_on_error {
                        return Err(e);
                    }
                }
            }
        }
        
        Ok(ToolChainResult {
            chain_name: chain_name.to_string(),
            results,
            final_output: current_input,
        })
    }
    
    /// Check permission for a tool
    async fn check_permission(&self, tool_name: &str, required: PermissionLevel) -> bool {
        let perms = self.permissions.read().await;
        
        // Check tool-specific permission
        if let Some(&level) = perms.tool_permissions.get(tool_name) {
            return level as i32 >= required as i32;
        }
        
        // Check category permission
        let defs = self.definitions.read().await;
        if let Some(def) = defs.get(tool_name) {
            if let Some(&level) = perms.category_permissions.get(&def.category) {
                return level as i32 >= required as i32;
            }
        }
        
        // Use default
        perms.default_level as i32 >= required as i32
    }
    
    /// Update tool statistics
    async fn update_stats(&self, tool_name: &str, success: bool, duration_ms: u64) {
        let mut stats = self.stats.write().await;
        
        stats.total_invocations += 1;
        if success {
            stats.successful_invocations += 1;
        } else {
            stats.failed_invocations += 1;
        }
        
        let tool_stats = stats.tool_stats
            .entry(tool_name.to_string())
            .or_default();
        
        tool_stats.invocations += 1;
        
        // Update average duration
        let old_avg = tool_stats.avg_duration_ms;
        let count = tool_stats.invocations as f64;
        tool_stats.avg_duration_ms = (old_avg * (count - 1.0) + duration_ms as f64) / count;
        
        // Update success rate
        let total = tool_stats.invocations;
        let successes = if success { total } else { total - 1 };
        tool_stats.success_rate = successes as f64 / total as f64;
        
        // Update last used
        tool_stats.last_used = Some(chrono::Utc::now().to_rfc3339());
    }
    
    /// Map chain parameters
    fn map_chain_params(
        &self,
        input: &serde_json::Value,
        mapping: &HashMap<String, String>,
    ) -> serde_json::Value {
        let mut params = serde_json::Map::new();
        
        for (target, source) in mapping {
            if let Some(value) = input.get(source) {
                params.insert(target.clone(), value.clone());
            }
        }
        
        serde_json::Value::Object(params)
    }
    
    /// Get tool count
    pub fn tool_count(&self) -> usize {
        self.registry.len()
    }
    
    /// Get chain count
    pub fn chain_count(&self) -> usize {
        self.chains.len()
    }
    
    /// Get statistics
    pub async fn get_stats(&self) -> ToolStats {
        self.stats.read().await.clone()
    }
    
    /// Get tool definitions
    pub async fn get_definitions(&self) -> HashMap<String, ToolDefinition> {
        let defs = self.definitions.read().await;
        defs.clone()
    }
}

/// Tool information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub available: bool,
}

/// Tool chain result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChainResult {
    pub chain_name: String,
    pub results: Vec<ChainStepResult>,
    pub final_output: serde_json::Value,
}

/// Chain step result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainStepResult {
    pub step_name: String,
    pub tool_name: String,
    pub success: bool,
    pub result: serde_json::Value,
}

/// Built-in tool implementation
struct BuiltinTool {
    name: String,
    description: String,
    category: String,
    parameters: serde_json::Value,
}

#[async_trait::async_trait]
impl Tool for BuiltinTool {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn description(&self) -> &str {
        &self.description
    }
    
    fn parameters_schema(&self) -> serde_json::Value {
        self.parameters.clone()
    }
    
    async fn execute(&self, params: serde_json::Value) -> Result<ToolResult, ToolError> {
        // Placeholder implementation
        // Real tools would have actual logic here
        Ok(ToolResult {
            success: true,
            data: serde_json::json!({
                "message": format!("Tool {} executed with params: {:?}", self.name, params)
            }),
            metadata: ToolExecutionMetadata {
                tool_name: self.name.clone(),
                duration_ms: 0,
                timestamp: chrono::Utc::now().to_rfc3339(),
                token_usage: None,
            },
        })
    }
    
    fn is_available(&self) -> bool {
        true
    }
}
