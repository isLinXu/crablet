//! Rule Conditions
//!
//! Evaluates whether a rule should fire based on the current context.

use std::path::PathBuf;
use regex::Regex;
use crate::rules::context::RuleContext;
use crate::safety::oracle::SafetyLevel;

/// Condition types for rule evaluation
#[derive(Debug, Clone)]
pub enum Condition {
    /// Always matches
    Always,
    /// Never matches
    Never,

    // Text-based conditions
    /// Input text contains a substring
    Contains(String),
    /// Input text matches a regex pattern
    Regex(Regex),
    /// Input text starts with a prefix
    StartsWith(String),
    /// Input text ends with a suffix
    EndsWith(String),

    // Safety-based conditions
    /// Safety score exceeds threshold (0.0 - 1.0)
    SafetyScoreAbove(f32),
    /// Safety score is below threshold
    SafetyScoreBelow(f32),
    /// Safety level matches exactly
    SafetyLevelIs(SafetyLevel),

    // Tool-based conditions
    /// Specific tool is being called
    ToolIs(String),
    /// Tool call count for a tool exceeds threshold
    ToolCallCountAbove { tool: String, threshold: u32 },
    /// Any tool is being called
    AnyTool,

    // Path-based conditions
    /// File path is within allowed directories
    PathWithinAllowed,
    /// File path matches a pattern
    PathMatches(String),

    // RPA-specific conditions
    /// RPA action is of a specific type
    RpaActionIs(String),
    /// Screen region is within whitelist
    RpaRegionInWhitelist,

    // Composite conditions
    /// All conditions must match (AND)
    All(Vec<Condition>),
    /// Any condition must match (OR)
    Any(Vec<Condition>),
    /// Condition must NOT match
    Not(Box<Condition>),
}

impl Condition {
    /// Evaluate this condition against the given context
    pub fn evaluate(&self, ctx: &RuleContext) -> bool {
        match self {
            Condition::Always => true,
            Condition::Never => false,

            // Text conditions
            Condition::Contains(s) => ctx.input.as_ref().map_or(false, |i| i.contains(s)),
            Condition::Regex(re) => ctx.input.as_ref().map_or(false, |i| re.is_match(i)),
            Condition::StartsWith(prefix) => ctx.input.as_ref().map_or(false, |i| i.starts_with(prefix)),
            Condition::EndsWith(suffix) => ctx.input.as_ref().map_or(false, |i| i.ends_with(suffix)),

            // Safety conditions
            Condition::SafetyScoreAbove(threshold) => ctx.safety_score.map_or(false, |s| s > *threshold),
            Condition::SafetyScoreBelow(threshold) => ctx.safety_score.map_or(false, |s| s < *threshold),
            Condition::SafetyLevelIs(level) => ctx.safety_level == Some(*level),

            // Tool conditions
            Condition::ToolIs(tool) => ctx.tool_name.as_ref().map_or(false, |t| t == tool),
            Condition::ToolCallCountAbove { tool, threshold } => {
                ctx.tool_call_counts.get(tool).map_or(false, |&count| count > *threshold)
            }
            Condition::AnyTool => ctx.tool_name.is_some(),

            // Path conditions
            Condition::PathWithinAllowed => {
                ctx.target_path.as_ref().map_or(false, |path| {
                    ctx.allowed_directories.iter().any(|dir| path.starts_with(dir))
                })
            }
            Condition::PathMatches(pattern) => {
                ctx.target_path.as_ref().map_or(false, |path| {
                    // Simple glob-like matching: check if path ends with pattern
                    // For full glob support, we could use the glob crate
                    path.to_string_lossy().contains(pattern)
                })
            }

            // RPA conditions
            Condition::RpaActionIs(action_type) => {
                ctx.rpa_action.as_ref().map_or(false, |a| a == action_type)
            }
            Condition::RpaRegionInWhitelist => {
                // Default to true if no region specified or no whitelist defined
                ctx.screen_region.is_none() || ctx.rpa_region_whitelist.is_empty()
            }

            // Composite conditions
            Condition::All(conditions) => conditions.iter().all(|c| c.evaluate(ctx)),
            Condition::Any(conditions) => conditions.iter().any(|c| c.evaluate(ctx)),
            Condition::Not(inner) => !inner.evaluate(ctx),
        }
    }
}

impl Condition {
    /// Helper to create a regex condition (returns Never on invalid regex)
    pub fn regex_str(pattern: &str) -> Self {
        match Regex::new(pattern) {
            Ok(re) => Condition::Regex(re),
            Err(_) => Condition::Never,
        }
    }

    /// Helper to create an AND composite condition
    pub fn all(conditions: Vec<Condition>) -> Self {
        Condition::All(conditions)
    }

    /// Helper to create an OR composite condition
    pub fn any(conditions: Vec<Condition>) -> Self {
        Condition::Any(conditions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_ctx() -> RuleContext {
        RuleContext {
            input: Some("hello world".to_string()),
            tool_name: None,
            tool_args: None,
            target_path: None,
            safety_score: Some(0.3),
            safety_level: Some(SafetyLevel::Strict),
            user_id: None,
            session_id: None,
            tool_call_counts: HashMap::new(),
            allowed_directories: vec![PathBuf::from("/tmp")],
            rpa_action: None,
            screen_region: None,
            rpa_region_whitelist: vec![],
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_always_never() {
        let ctx = test_ctx();
        assert!(Condition::Always.evaluate(&ctx));
        assert!(!Condition::Never.evaluate(&ctx));
    }

    #[test]
    fn test_contains() {
        let ctx = test_ctx();
        assert!(Condition::Contains("hello".to_string()).evaluate(&ctx));
        assert!(!Condition::Contains("foo".to_string()).evaluate(&ctx));
    }

    #[test]
    fn test_regex() {
        let ctx = test_ctx();
        let re = Condition::Regex(Regex::new(r"hello\s+\w+").unwrap());
        assert!(re.evaluate(&ctx));
    }

    #[test]
    fn test_safety_score() {
        let ctx = test_ctx(); // score = 0.3
        assert!(Condition::SafetyScoreAbove(0.2).evaluate(&ctx));
        assert!(!Condition::SafetyScoreAbove(0.5).evaluate(&ctx));
        assert!(Condition::SafetyScoreBelow(0.5).evaluate(&ctx));
    }

    #[test]
    fn test_tool_conditions() {
        let mut ctx = test_ctx();
        ctx.tool_name = Some("bash".to_string());
        assert!(Condition::ToolIs("bash".to_string()).evaluate(&ctx));
        assert!(!Condition::ToolIs("file".to_string()).evaluate(&ctx));
        assert!(Condition::AnyTool.evaluate(&ctx));
    }

    #[test]
    fn test_composite_and() {
        let ctx = test_ctx();
        let cond = Condition::All(vec![
            Condition::Contains("hello".to_string()),
            Condition::Contains("world".to_string()),
        ]);
        assert!(cond.evaluate(&ctx));

        let cond_fail = Condition::All(vec![
            Condition::Contains("hello".to_string()),
            Condition::Contains("foo".to_string()),
        ]);
        assert!(!cond_fail.evaluate(&ctx));
    }

    #[test]
    fn test_composite_or() {
        let ctx = test_ctx();
        let cond = Condition::Any(vec![
            Condition::Contains("foo".to_string()),
            Condition::Contains("hello".to_string()),
        ]);
        assert!(cond.evaluate(&ctx));
    }

    #[test]
    fn test_composite_not() {
        let ctx = test_ctx();
        let cond = Condition::Not(Box::new(Condition::Contains("foo".to_string())));
        assert!(cond.evaluate(&ctx));

        let cond = Condition::Not(Box::new(Condition::Contains("hello".to_string())));
        assert!(!cond.evaluate(&ctx));
    }
}
