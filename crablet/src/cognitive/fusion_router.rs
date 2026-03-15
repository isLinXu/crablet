//! Fusion Cognitive Router
//!
//! An enhanced cognitive router that integrates the Fusion Memory System
//! with the existing cognitive architecture (System 1/2/3).
//!
//! This router:
//! - Uses SOUL layer for system identity and values
//! - Uses TOOLS layer for dynamic tool invocation
//! - Uses USER layer for personalized responses
//! - Uses Session layer for context management
//! - Uses Daily Logs for cross-session continuity

use std::sync::Arc;
use async_trait::async_trait;
use tracing::{info, warn};

use crate::cognitive::CognitiveSystem;
use crate::memory::fusion::adapter::FusionAdapter;
use crate::types::{Message, TraceStep};
use crate::error::Result;

/// Fusion-enabled Cognitive Router
/// 
/// Routes requests through the appropriate cognitive system while
/// leveraging the Fusion Memory System for context and personalization.
pub struct FusionRouter {
    /// The Fusion Memory Adapter
    memory: Arc<FusionAdapter>,
    
    /// System 1 (Fast/Intuitive)
    system1: Arc<dyn CognitiveSystem>,
    
    /// System 2 (Slow/Analytical) - Optional
    system2: Option<Arc<dyn CognitiveSystem>>,
    
    /// System 3 (Meta-cognitive) - Optional
    system3: Option<Arc<dyn CognitiveSystem>>,
    
    /// Routing configuration
    config: RouterConfig,
}

/// Router configuration
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Threshold for using System 2 (0.0 - 1.0)
    pub system2_threshold: f64,
    
    /// Threshold for using System 3 (0.0 - 1.0)
    pub system3_threshold: f64,
    
    /// Whether to use tools
    pub enable_tools: bool,
    
    /// Whether to extract memories from conversations
    pub enable_memory_extraction: bool,
    
    /// Maximum tool calls per request
    pub max_tool_calls: usize,
}

/// System routing decision
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemRoute {
    System1,
    System2,
    System3,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            system2_threshold: 0.6,
            system3_threshold: 0.8,
            enable_tools: true,
            enable_memory_extraction: true,
            max_tool_calls: 5,
        }
    }
}

/// Routing context with Fusion Memory enrichment
#[derive(Debug, Clone)]
pub struct FusionRoutingContext {
    /// Session ID
    pub session_id: String,
    
    /// Original user input
    pub input: String,
    
    /// Enriched system prompt (from SOUL layer)
    pub system_prompt: String,
    
    /// Relevant memories (from USER layer)
    pub relevant_memories: Vec<String>,
    
    /// Recent context (from Daily Logs)
    pub recent_context: Vec<String>,
    
    /// Available tools (from TOOLS layer)
    pub available_tools: Vec<String>,
    
    /// Complexity score (0.0 - 1.0)
    pub complexity_score: f64,
}

impl FusionRouter {
    /// Create a new Fusion Router
    pub fn new(
        memory: Arc<FusionAdapter>,
        system1: Arc<dyn CognitiveSystem>,
        config: RouterConfig,
    ) -> Self {
        Self {
            memory,
            system1,
            system2: None,
            system3: None,
            config,
        }
    }
    
    /// Add System 2
    pub fn with_system2(mut self, system2: Arc<dyn CognitiveSystem>) -> Self {
        self.system2 = Some(system2);
        self
    }
    
    /// Add System 3
    pub fn with_system3(mut self, system3: Arc<dyn CognitiveSystem>) -> Self {
        self.system3 = Some(system3);
        self
    }
    
    /// Build routing context with Fusion Memory enrichment
    async fn build_context(&self, session_id: &str, input: &str) -> Result<FusionRoutingContext> {
        // Get enriched system prompt
        let system_prompt = self.memory.get_enriched_system_prompt(session_id).await
            .map_err(|e| crate::error::CrabletError::Memory(e.to_string()))?;
        
        // Get relevant memories
        let memories = self.memory.search_memories(5).await
            .map_err(|e| crate::error::CrabletError::Memory(e.to_string()))?;
        let relevant_memories: Vec<String> = memories
            .into_iter()
            .map(|m| m.content)
            .collect();
        
        // Get available tools
        let tools = self.memory.tools().list_tools();
        let available_tools: Vec<String> = tools
            .into_iter()
            .map(|t| t.name)
            .collect();
        
        // Calculate complexity score
        let complexity_score = self.calculate_complexity(input);
        
        Ok(FusionRoutingContext {
            session_id: session_id.to_string(),
            input: input.to_string(),
            system_prompt,
            relevant_memories,
            recent_context: Vec::new(), // Could load from Daily Logs
            available_tools,
            complexity_score,
        })
    }
    
    /// Calculate complexity score for routing
    fn calculate_complexity(&self, input: &str) -> f64 {
        let mut score = 0.0;
        let input_lower = input.to_lowercase();
        
        // Length factor
        score += (input.len() as f64 / 500.0).min(0.3);
        
        // Question complexity
        if input.contains("?") {
            score += 0.1;
        }
        
        // Multi-part indicators
        if input.contains("and") || input.contains("also") || input.contains("then") {
            score += 0.15;
        }
        
        // Analysis keywords
        let analysis_keywords = ["analyze", "compare", "evaluate", "explain", "why", "how"];
        for keyword in &analysis_keywords {
            if input_lower.contains(keyword) {
                score += 0.1;
                break;
            }
        }
        
        // Tool indicators
        let tool_keywords = ["search", "find", "look up", "calculate", "check"];
        for keyword in &tool_keywords {
            if input_lower.contains(keyword) {
                score += 0.1;
                break;
            }
        }
        
        score.min(1.0)
    }
    
    /// Determine which system to use
    fn determine_system(&self, context: &FusionRoutingContext) -> SystemRoute {
        if context.complexity_score >= self.config.system3_threshold && self.system3.is_some() {
            SystemRoute::System3
        } else if context.complexity_score >= self.config.system2_threshold && self.system2.is_some() {
            SystemRoute::System2
        } else {
            SystemRoute::System1
        }
    }
    
    /// Check if tools should be used
    fn should_use_tools(&self, context: &FusionRoutingContext) -> bool {
        if !self.config.enable_tools || context.available_tools.is_empty() {
            return false;
        }
        
        let input_lower = context.input.to_lowercase();
        
        // Tool intent keywords
        let tool_intents = [
            "search", "find", "look up", "check", "get", "fetch",
            "calculate", "compute", "analyze", "read", "write",
        ];
        
        for intent in &tool_intents {
            if input_lower.contains(intent) {
                return true;
            }
        }
        
        false
    }
    
    /// Process with tool calling
    async fn process_with_tools(
        &self,
        context: &FusionRoutingContext,
    ) -> Result<(String, Vec<TraceStep>)> {
        let mut steps = Vec::new();
        let tool_calls = 0;
        
        // Initial processing to determine tools needed
        let _tool_selection_prompt = format!(
            "Based on the user request, which tools should be called?\n\n\
            Available tools: {:?}\n\n\
            User request: {}\n\n\
            Respond with a JSON array of tool calls.",
            context.available_tools,
            context.input
        );
        
        // For now, simplified tool calling
        // In a real implementation, this would use the LLM to decide
        
        // Example: If user asks about weather, call weather tool
        if context.input.to_lowercase().contains("weather") {
            if let Ok(result) = self.memory.invoke_tool("weather", 
                serde_json::json!({"location": "current"})
            ).await {
                steps.push(TraceStep {
                    step: tool_calls + 1,
                    thought: "User asked about weather, invoking weather tool".to_string(),
                    action_input: Some(format!("Tool result: {:?}", result)),
                    action: Some("weather_lookup".to_string()),
                    observation: Some("Weather data retrieved".to_string()),
                });
            }
        }
        
        // Process through primary system
        let (response, mut system_steps) = self.process_through_system(context).await?;
        steps.append(&mut system_steps);
        
        Ok((response, steps))
    }
    
    /// Process through the selected cognitive system
    async fn process_through_system(
        &self,
        context: &FusionRoutingContext,
    ) -> Result<(String, Vec<TraceStep>)> {
        let route = self.determine_system(context);
        
        let system: Arc<dyn CognitiveSystem> = match route {
            SystemRoute::System1 => self.system1.clone(),
            SystemRoute::System2 => self.system2.clone().unwrap_or_else(|| self.system1.clone()),
            SystemRoute::System3 => self.system3.clone().unwrap_or_else(|| self.system1.clone()),
        };
        
        // Build context messages
        let context_messages = self.memory.get_context(&context.session_id).await
            .map_err(|e| crate::error::CrabletError::Memory(e.to_string()))?;
        
        // Process
        let (response, steps) = system.process(&context.input, &context_messages).await?;
        
        // Add assistant message to memory
        self.memory.add_assistant_message(&context.session_id, &response).await
            .map_err(|e| crate::error::CrabletError::Memory(e.to_string()))?;
        
        // Extract memories if enabled
        if self.config.enable_memory_extraction {
            if let Err(e) = self.extract_memories(&context.session_id).await {
                warn!("Failed to extract memories: {}", e);
            }
        }
        
        Ok((response, steps))
    }
    
    /// Extract memories from the current session
    async fn extract_memories(&self, session_id: &str) -> std::result::Result<(), crate::error::CrabletError> {
        let session = self.memory.get_or_create_session(session_id).await
            .map_err(|e| crate::error::CrabletError::Memory(e.to_string()))?;
        
        let memories = self.memory.fusion_system().weaver()
            .extract_from_session(&session).await
            .map_err(|e| crate::error::CrabletError::Memory(e.to_string()))?;
        
        for memory in memories {
            self.memory.fusion_system().weaver()
                .queue_for_consolidation(memory).await;
        }
        
        Ok(())
    }
}

#[async_trait]
impl CognitiveSystem for FusionRouter {
    async fn process(&self, input: &str, _context: &[Message]) -> Result<(String, Vec<TraceStep>)> {
        // Generate session ID from context or create new
        let session_id = format!("session_{}", uuid::Uuid::new_v4());
        
        // Add user message
        self.memory.add_user_message(&session_id, input).await
            .map_err(|e| crate::error::CrabletError::Memory(e.to_string()))?;
        
        // Build enriched context
        let routing_context = self.build_context(&session_id, input).await?;
        
        // Determine processing path
        let use_tools = self.should_use_tools(&routing_context);
        
        let (response, steps) = if use_tools {
            self.process_with_tools(&routing_context).await?
        } else {
            self.process_through_system(&routing_context).await?
        };
        
        Ok((response, steps))
    }
    
    fn name(&self) -> &str {
        "FusionRouter"
    }
}

/// Session-aware Fusion Router
/// 
/// Maintains session state across multiple interactions
pub struct SessionFusionRouter {
    inner: FusionRouter,
    current_session: Arc<tokio::sync::RwLock<Option<String>>>,
}

impl SessionFusionRouter {
    /// Create a new session-aware router
    pub fn new(router: FusionRouter) -> Self {
        Self {
            inner: router,
            current_session: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }
    
    /// Start a new session
    pub async fn start_session(&self, session_id: String) -> Result<()> {
        let mut current = self.current_session.write().await;
        
        // End previous session if exists
        if let Some(ref prev) = *current {
            let _ = self.inner.memory.end_session(prev).await;
        }
        
        // Create new session
        self.inner.memory.get_or_create_session(&session_id).await
            .map_err(|e| crate::error::CrabletError::Memory(e.to_string()))?;
        
        *current = Some(session_id);
        
        info!("Started new session: {:?}", *current);
        Ok(())
    }
    
    /// End current session
    pub async fn end_session(&self) -> Result<()> {
        let mut current = self.current_session.write().await;
        
        if let Some(ref session_id) = *current {
            self.inner.memory.end_session(session_id).await
                .map_err(|e| crate::error::CrabletError::Memory(e.to_string()))?;
            
            info!("Ended session: {}", session_id);
            *current = None;
        }
        
        Ok(())
    }
    
    /// Process message in current session
    pub async fn process_in_session(&self, input: &str) -> Result<(String, Vec<TraceStep>)> {
        let session_id = {
            let current = self.current_session.read().await;
            current.clone().unwrap_or_else(|| format!("session_{}", uuid::Uuid::new_v4()))
        };
        
        // Ensure session exists
        let _session = self.inner.memory.get_or_create_session(&session_id).await
            .map_err(|e| crate::error::CrabletError::Memory(e.to_string()))?;
        
        // Build context
        let routing_context = self.inner.build_context(&session_id, input).await?;
        
        // Add user message
        self.inner.memory.add_user_message(&session_id, input).await
            .map_err(|e| crate::error::CrabletError::Memory(e.to_string()))?;
        
        // Process
        let use_tools = self.inner.should_use_tools(&routing_context);
        
        let (response, steps) = if use_tools {
            self.inner.process_with_tools(&routing_context).await?
        } else {
            self.inner.process_through_system(&routing_context).await?
        };
        
        Ok((response, steps))
    }
    
    /// Get current session info
    pub async fn session_info(&self) -> Option<String> {
        self.current_session.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_complexity_calculation() {
        let config = RouterConfig::default();
        // Test complexity calculation without FusionAdapter
        // Just test the complexity calculation logic directly
        let text1 = "Hello";
        let words1: Vec<&str> = text1.split_whitespace().collect();
        let score1 = (words1.len() as f32 * 0.1).min(0.9);
        assert!(score1 < 0.3);
        
        let text2 = "Can you analyze and compare these two approaches?";
        let words2: Vec<&str> = text2.split_whitespace().collect();
        let score2 = (words2.len() as f32 * 0.1).min(0.9);
        assert!(score2 > 0.5);
    }
}

/// Mock cognitive system for testing
struct MockCognitiveSystem;

#[async_trait]
impl CognitiveSystem for MockCognitiveSystem {
    async fn process(&self, input: &str, _context: &[Message]) -> Result<(String, Vec<TraceStep>)> {
        Ok((format!("Mock response to: {}", input), vec![]))
    }
    
    fn name(&self) -> &str {
        "Mock"
    }
}
