//! Skill Context Management
//!
//! Provides execution context for skills, including:
//! - Session tracking
//! - User input and extracted arguments
//! - Execution history for multi-skill chains
//! - Shared state between skills
//! - Memory system integration

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Execution context for skill invocation
#[derive(Debug, Clone)]
pub struct SkillContext {
    /// Current session ID
    pub session_id: String,
    /// Original user input
    pub user_input: String,
    /// Arguments extracted from input (via triggers or parsing)
    pub extracted_args: serde_json::Value,
    /// Execution history for multi-skill chains
    pub execution_history: Vec<ExecutionRecord>,
    /// Shared state accessible across skills in a chain
    pub shared_state: HashMap<String, serde_json::Value>,
    /// Memory system context (SOUL, user preferences, etc.)
    pub memory_context: Option<MemoryContext>,
    /// Maximum history size to prevent unbounded growth
    max_history_size: usize,
}

/// Record of a single skill execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    /// Name of the executed skill
    pub skill_name: String,
    /// Input arguments provided to the skill
    pub input: serde_json::Value,
    /// Output produced by the skill
    pub output: String,
    /// Whether execution was successful
    pub success: bool,
    /// Execution timestamp
    pub timestamp: DateTime<Utc>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
}

/// Memory system context for skill execution
#[derive(Debug, Clone, Default)]
pub struct MemoryContext {
    /// SOUL.md content (agent personality)
    pub soul_context: Option<String>,
    /// User preferences from MEMORY.md
    pub user_preferences: HashMap<String, String>,
    /// Relevant memories retrieved for current context
    pub relevant_memories: Vec<String>,
    /// Session-specific context
    pub session_context: Option<String>,
}

impl SkillContext {
    /// Create a new skill context
    pub fn new(session_id: impl Into<String>, user_input: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            user_input: user_input.into(),
            extracted_args: serde_json::Value::Null,
            execution_history: Vec::new(),
            shared_state: HashMap::new(),
            memory_context: None,
            max_history_size: 100,
        }
    }
    
    /// Create a new context with extracted arguments
    pub fn with_args(mut self, args: serde_json::Value) -> Self {
        self.extracted_args = args;
        self
    }
    
    /// Create a new context with memory context
    pub fn with_memory(mut self, memory: MemoryContext) -> Self {
        self.memory_context = Some(memory);
        self
    }
    
    /// Set the maximum history size
    pub fn with_max_history(mut self, size: usize) -> Self {
        self.max_history_size = size;
        self
    }
    
    /// Record a skill execution in history
    pub fn record_execution(
        &mut self, 
        skill_name: &str, 
        input: serde_json::Value, 
        output: &str, 
        success: bool,
        duration_ms: u64,
    ) {
        self.execution_history.push(ExecutionRecord {
            skill_name: skill_name.to_string(),
            input,
            output: output.to_string(),
            success,
            timestamp: Utc::now(),
            duration_ms,
        });
        
        // Trim history if it exceeds max size
        if self.execution_history.len() > self.max_history_size {
            let excess = self.execution_history.len() - self.max_history_size;
            self.execution_history.drain(0..excess);
        }
    }
    
    /// Get a value from shared state
    pub fn get_state(&self, key: &str) -> Option<&serde_json::Value> {
        self.shared_state.get(key)
    }
    
    /// Get a typed value from shared state
    pub fn get_state_typed<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.shared_state.get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }
    
    /// Set a value in shared state
    pub fn set_state(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.shared_state.insert(key.into(), value);
    }
    
    /// Set a typed value in shared state
    pub fn set_state_typed<T: Serialize>(&mut self, key: impl Into<String>, value: T) {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.shared_state.insert(key.into(), json_value);
        }
    }
    
    /// Remove a value from shared state
    pub fn remove_state(&mut self, key: &str) -> Option<serde_json::Value> {
        self.shared_state.remove(key)
    }
    
    /// Check if a key exists in shared state
    pub fn has_state(&self, key: &str) -> bool {
        self.shared_state.contains_key(key)
    }
    
    /// Get the last execution record
    pub fn last_execution(&self) -> Option<&ExecutionRecord> {
        self.execution_history.last()
    }
    
    /// Get execution history for a specific skill
    pub fn executions_of(&self, skill_name: &str) -> Vec<&ExecutionRecord> {
        self.execution_history.iter()
            .filter(|r| r.skill_name == skill_name)
            .collect()
    }
    
    /// Check if a skill has been executed successfully in this context
    pub fn has_successful_execution(&self, skill_name: &str) -> bool {
        self.execution_history.iter()
            .any(|r| r.skill_name == skill_name && r.success)
    }
    
    /// Get the count of executions in this context
    pub fn execution_count(&self) -> usize {
        self.execution_history.len()
    }
    
    /// Get successful execution count
    pub fn successful_count(&self) -> usize {
        self.execution_history.iter()
            .filter(|r| r.success)
            .count()
    }
    
    /// Clear execution history
    pub fn clear_history(&mut self) {
        self.execution_history.clear();
    }
    
    /// Clear shared state
    pub fn clear_state(&mut self) {
        self.shared_state.clear();
    }
    
    /// Build a context summary for LLM prompts
    pub fn build_context_summary(&self) -> String {
        let mut parts = Vec::new();
        
        // Add execution history summary
        if !self.execution_history.is_empty() {
            parts.push("## Execution History".to_string());
            for record in self.execution_history.iter().rev().take(5) {
                let status = if record.success { "✓" } else { "✗" };
                parts.push(format!(
                    "{} {} ({}ms)",
                    status,
                    record.skill_name,
                    record.duration_ms
                ));
            }
        }
        
        // Add shared state summary
        if !self.shared_state.is_empty() {
            parts.push("\n## Shared State".to_string());
            for (key, value) in &self.shared_state {
                let value_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    _ => value.to_string(),
                };
                parts.push(format!("{}: {}", key, value_str));
            }
        }
        
        parts.join("\n")
    }
    
    /// Merge another context into this one
    pub fn merge(&mut self, other: &SkillContext) {
        // Merge execution history
        for record in &other.execution_history {
            self.execution_history.push(record.clone());
        }
        
        // Merge shared state (other takes precedence)
        for (key, value) in &other.shared_state {
            self.shared_state.insert(key.clone(), value.clone());
        }
        
        // Trim history if needed
        if self.execution_history.len() > self.max_history_size {
            let excess = self.execution_history.len() - self.max_history_size;
            self.execution_history.drain(0..excess);
        }
    }
    
    /// Create a child context for nested skill execution
    pub fn create_child(&self, skill_name: impl Into<String>) -> Self {
        Self {
            session_id: format!("{}:{}", self.session_id, skill_name.into()),
            user_input: self.user_input.clone(),
            extracted_args: serde_json::Value::Null,
            execution_history: Vec::new(),
            shared_state: self.shared_state.clone(), // Share state with parent
            memory_context: self.memory_context.clone(),
            max_history_size: self.max_history_size,
        }
    }
}

impl Default for SkillContext {
    fn default() -> Self {
        Self::new("default", "")
    }
}

impl MemoryContext {
    /// Create a new memory context
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Add SOUL context
    pub fn with_soul(mut self, soul: impl Into<String>) -> Self {
        self.soul_context = Some(soul.into());
        self
    }
    
    /// Add user preference
    pub fn with_preference(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.user_preferences.insert(key.into(), value.into());
        self
    }
    
    /// Add relevant memory
    pub fn with_memory(mut self, memory: impl Into<String>) -> Self {
        self.relevant_memories.push(memory.into());
        self
    }
    
    /// Build memory context for prompts
    pub fn build_prompt_context(&self) -> String {
        let mut parts = Vec::new();
        
        if let Some(soul) = &self.soul_context {
            parts.push(format!("## Agent Personality\n{}", soul));
        }
        
        if !self.user_preferences.is_empty() {
            parts.push("\n## User Preferences".to_string());
            for (key, value) in &self.user_preferences {
                parts.push(format!("{}: {}", key, value));
            }
        }
        
        if !self.relevant_memories.is_empty() {
            parts.push("\n## Relevant Memories".to_string());
            for memory in &self.relevant_memories {
                parts.push(format!("- {}", memory));
            }
        }
        
        parts.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = SkillContext::new("session-123", "Hello world");
        assert_eq!(ctx.session_id, "session-123");
        assert_eq!(ctx.user_input, "Hello world");
        assert!(ctx.execution_history.is_empty());
    }

    #[test]
    fn test_context_with_args() {
        let ctx = SkillContext::new("session-123", "Hello")
            .with_args(serde_json::json!({"name": "World"}));
        
        assert_eq!(ctx.extracted_args["name"], "World");
    }

    #[test]
    fn test_record_execution() {
        let mut ctx = SkillContext::new("session-123", "Hello");
        
        ctx.record_execution(
            "test-skill",
            serde_json::json!({"arg": 1}),
            "result",
            true,
            100
        );
        
        assert_eq!(ctx.execution_count(), 1);
        assert!(ctx.has_successful_execution("test-skill"));
        
        let last = ctx.last_execution().unwrap();
        assert_eq!(last.skill_name, "test-skill");
        assert!(last.success);
    }

    #[test]
    fn test_shared_state() {
        let mut ctx = SkillContext::new("session-123", "Hello");
        
        ctx.set_state("key1", serde_json::json!("value1"));
        assert!(ctx.has_state("key1"));
        assert_eq!(ctx.get_state("key1").unwrap(), "value1");
        
        ctx.set_state_typed("key2", 42i32);
        assert_eq!(ctx.get_state_typed::<i32>("key2").unwrap(), 42);
        
        ctx.remove_state("key1");
        assert!(!ctx.has_state("key1"));
    }

    #[test]
    fn test_child_context() {
        let mut parent = SkillContext::new("session-123", "Hello");
        parent.set_state("shared", serde_json::json!("data"));
        
        let child = parent.create_child("child-skill");
        
        assert!(child.session_id.contains("child-skill"));
        assert_eq!(child.get_state("shared").unwrap(), "data");
        assert!(child.execution_history.is_empty()); // Fresh history
    }

    #[test]
    fn test_history_limit() {
        let mut ctx = SkillContext::new("session-123", "Hello")
            .with_max_history(3);
        
        for i in 0..5 {
            ctx.record_execution(
                &format!("skill-{}", i),
                serde_json::json!({}),
                "result",
                true,
                100
            );
        }
        
        assert_eq!(ctx.execution_count(), 3);
        // Should keep the most recent
        assert!(ctx.has_successful_execution("skill-4"));
        assert!(!ctx.has_successful_execution("skill-0"));
    }

    #[test]
    fn test_memory_context() {
        let mem = MemoryContext::new()
            .with_soul("You are a helpful assistant")
            .with_preference("language", "zh-CN")
            .with_memory("User likes Python");
        
        let prompt = mem.build_prompt_context();
        assert!(prompt.contains("helpful assistant"));
        assert!(prompt.contains("language: zh-CN"));
        assert!(prompt.contains("likes Python"));
    }
}
