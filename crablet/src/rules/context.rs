//! Rule Context
//!
//! Provides the evaluation context for rules. Contains all the information
//! a rule might need to make a decision.

use std::collections::HashMap;
use std::path::PathBuf;
use crate::safety::oracle::SafetyLevel;
use crate::rules::condition::Condition;

/// Context for rule evaluation
#[derive(Debug, Clone)]
pub struct RuleContext {
    /// The raw input text (user message, command, etc.)
    pub input: Option<String>,

    /// The tool/agent being called
    pub tool_name: Option<String>,

    /// Arguments for the tool call
    pub tool_args: Option<serde_json::Value>,

    /// Target file path (for file operations)
    pub target_path: Option<PathBuf>,

    /// Safety score from SafetyOracle (0.0 = safe, 1.0 = unsafe)
    pub safety_score: Option<f32>,

    /// Current safety level
    pub safety_level: Option<SafetyLevel>,

    /// User identifier
    pub user_id: Option<String>,

    /// Session identifier
    pub session_id: Option<String>,

    /// Tool call counts per tool in this session
    pub tool_call_counts: HashMap<String, u32>,

    /// Allowed directories for file access
    pub allowed_directories: Vec<PathBuf>,

    // RPA-specific context
    /// RPA action type (e.g., "mouse_move", "key_type")
    pub rpa_action: Option<String>,

    /// Screen region for RPA operations
    pub screen_region: Option<(i32, i32, u32, u32)>,

    /// Whitelisted screen regions (x, y, w, h)
    pub rpa_region_whitelist: Vec<(i32, i32, u32, u32)>,

    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl RuleContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self {
            input: None,
            tool_name: None,
            tool_args: None,
            target_path: None,
            safety_score: None,
            safety_level: None,
            user_id: None,
            session_id: None,
            tool_call_counts: HashMap::new(),
            allowed_directories: vec![
                std::env::current_dir().unwrap_or(PathBuf::from(".")),
                PathBuf::from("/tmp"),
            ],
            rpa_action: None,
            screen_region: None,
            rpa_region_whitelist: vec![],
            metadata: HashMap::new(),
        }
    }

    /// Create context for bash command evaluation
    pub fn for_bash(cmd: &str) -> Self {
        let mut ctx = Self::new();
        ctx.input = Some(cmd.to_string());
        ctx.tool_name = Some("bash".to_string());
        ctx.tool_args = Some(serde_json::json!({ "cmd": cmd }));
        ctx
    }

    /// Create context for file operation evaluation
    pub fn for_file(path: &str, operation: &str) -> Self {
        let mut ctx = Self::new();
        ctx.target_path = Some(PathBuf::from(path));
        ctx.tool_name = Some("file".to_string());
        ctx.input = Some(operation.to_string());
        ctx
    }

    /// Create context for RPA action evaluation
    pub fn for_rpa(action: &str, region: Option<(i32, i32, u32, u32)>) -> Self {
        let mut ctx = Self::new();
        ctx.rpa_action = Some(action.to_string());
        ctx.screen_region = region;
        ctx.tool_name = Some("rpa".to_string());
        ctx
    }

    /// Create context for tool call evaluation
    pub fn for_tool(tool_name: &str, args: serde_json::Value) -> Self {
        let mut ctx = Self::new();
        ctx.tool_name = Some(tool_name.to_string());
        ctx.tool_args = Some(args);
        ctx
    }

    /// Set the user ID
    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set the session ID
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set the safety score
    pub fn with_safety_score(mut self, score: f32) -> Self {
        self.safety_score = Some(score);
        self
    }

    /// Set the safety level
    pub fn with_safety_level(mut self, level: SafetyLevel) -> Self {
        self.safety_level = Some(level);
        self
    }

    /// Set allowed directories
    pub fn with_allowed_dirs(mut self, dirs: Vec<PathBuf>) -> Self {
        self.allowed_directories = dirs;
        self
    }

    /// Set RPA region whitelist
    pub fn with_rpa_whitelist(mut self, regions: Vec<(i32, i32, u32, u32)>) -> Self {
        self.rpa_region_whitelist = regions;
        self
    }

    /// Add a tool call count
    pub fn with_tool_call_count(mut self, tool: impl Into<String>, count: u32) -> Self {
        self.tool_call_counts.insert(tool.into(), count);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Extract the command from tool args (for bash/file contexts)
    pub fn command(&self) -> Option<&str> {
        self.tool_args
            .as_ref()
            .and_then(|args| args.get("cmd"))
            .and_then(|v| v.as_str())
            .or_else(|| self.input.as_deref())
    }

    /// Extract the first token of the command (the binary name)
    pub fn command_binary(&self) -> Option<&str> {
        self.command()
            .and_then(|cmd| cmd.split_whitespace().next())
    }

    /// Check if input matches a condition shorthand
    pub fn input_matches(&self, pattern: &str) -> bool {
        self.input.as_ref().map_or(false, |i| i.contains(pattern))
    }

    /// Increment tool call count and return updated context
    pub fn increment_tool_call(&mut self, tool: &str) {
        *self.tool_call_counts.entry(tool.to_string()).or_insert(0) += 1;
    }
}

impl Default for RuleContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = RuleContext::new();
        assert!(ctx.input.is_none());
        assert!(ctx.tool_name.is_none());
    }

    #[test]
    fn test_bash_context() {
        let ctx = RuleContext::for_bash("ls -la");
        assert_eq!(ctx.tool_name.as_deref(), Some("bash"));
        assert_eq!(ctx.command(), Some("ls -la"));
        assert_eq!(ctx.command_binary(), Some("ls"));
    }

    #[test]
    fn test_file_context() {
        let ctx = RuleContext::for_file("/tmp/test.txt", "write");
        assert_eq!(ctx.tool_name.as_deref(), Some("file"));
        assert_eq!(ctx.target_path.as_deref().map(|p| p.to_str()), Some(Some("/tmp/test.txt")));
    }

    #[test]
    fn test_rpa_context() {
        let ctx = RuleContext::for_rpa("mouse_click", Some((100, 200, 50, 50)));
        assert_eq!(ctx.rpa_action.as_deref(), Some("mouse_click"));
        assert_eq!(ctx.screen_region, Some((100, 200, 50, 50)));
    }

    #[test]
    fn test_builder_pattern() {
        let ctx = RuleContext::for_bash("rm file.txt")
            .with_user("user123")
            .with_session("session456")
            .with_safety_score(0.5);

        assert_eq!(ctx.user_id.as_deref(), Some("user123"));
        assert_eq!(ctx.session_id.as_deref(), Some("session456"));
        assert_eq!(ctx.safety_score, Some(0.5));
    }

    #[test]
    fn test_tool_call_count() {
        let mut ctx = RuleContext::new()
            .with_tool_call_count("bash", 3);

        assert_eq!(ctx.tool_call_counts.get("bash"), Some(&3));
        ctx.increment_tool_call("bash");
        assert_eq!(ctx.tool_call_counts.get("bash"), Some(&4));
    }
}
