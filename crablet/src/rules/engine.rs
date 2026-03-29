//! Rule Engine Core
//!
//! Central rule evaluation engine that orchestrates condition matching and action execution.

use std::sync::Arc;
use tracing::{info, warn, debug};
use parking_lot::RwLock;
use crate::rules::{Rule, RuleDecision, RuleContext, Condition, Action};
use crate::rules::loader::RuleLoader;

/// The main rule engine that evaluates rules against contexts
pub struct RuleEngine {
    rules: Arc<RwLock<Vec<Rule>>>,
    /// When true, the first matching rule wins (stop evaluation)
    first_match_wins: bool,
    /// Default decision when no rule matches
    default_decision: RuleDecision,
}

impl RuleEngine {
    /// Create a new empty rule engine
    pub fn new() -> Self {
        Self {
            rules: Arc::new(RwLock::new(Vec::new())),
            first_match_wins: true,
            default_decision: RuleDecision::NoMatch,
        }
    }

    /// Create a rule engine with default built-in safety rules
    pub fn with_default_rules() -> Self {
        let engine = Self::new();
        engine.add_default_safety_rules();
        engine
    }

    /// Add default safety rules for common scenarios
    fn add_default_safety_rules(&self) {
        let mut rules = self.rules.write();

        // Rule: Block rm -rf commands
        rules.push(Rule::builder("block-rm-rf")
            .name("Block dangerous rm -rf")
            .priority(100)
            .description("Blocks rm -rf commands that can destroy data")
            .condition(Condition::Contains("rm -rf".to_string()))
            .action(Action::Block("Dangerous command 'rm -rf' is not allowed".to_string()))
            .build()
        );

        // Rule: Block fork bombs
        rules.push(Rule::builder("block-fork-bomb")
            .name("Block fork bombs")
            .priority(100)
            .description("Blocks fork bomb patterns")
            .condition(Condition::Contains(":(){:|:&};:".to_string()))
            .action(Action::Block("Fork bomb pattern detected".to_string()))
            .build()
        );

        // Rule: Block mkfs
        rules.push(Rule::builder("block-mkfs")
            .name("Block filesystem formatting")
            .priority(100)
            .condition(Condition::Contains("mkfs".to_string()))
            .action(Action::Block("Filesystem formatting is not allowed".to_string()))
            .build()
        );

        // Rule: Block dd to /dev/zero
        rules.push(Rule::builder("block-dd-zero")
            .name("Block dd to /dev/zero")
            .priority(100)
            .condition(Condition::Regex(
                regex::Regex::new(r"dd\s+.*if=/dev/zero").unwrap()
            ))
            .action(Action::Block("Block device write operations are not allowed".to_string()))
            .build()
        );

        // Rule: Require confirmation for dangerous commands
        rules.push(Rule::builder("confirm-dangerous")
            .name("Confirm dangerous commands")
            .priority(50)
            .description("Requires confirmation for commands like rm, sudo, ssh, curl, wget")
            .condition(Condition::Any(vec![
                Condition::Contains("rm ".to_string()),
                Condition::Contains("sudo ".to_string()),
                Condition::Contains("ssh ".to_string()),
                Condition::Contains("curl ".to_string()),
                Condition::Contains("wget ".to_string()),
                Condition::Contains("nmap ".to_string()),
                Condition::Contains("chmod ".to_string()),
                Condition::Contains("chown ".to_string()),
            ]))
            .action(Action::RequireConfirmation(
                "This command requires user confirmation".to_string()
            ))
            .build()
        );

        // Rule: Warn on high safety score
        rules.push(Rule::builder("warn-unsafe")
            .name("Warn on unsafe input")
            .priority(30)
            .description("Logs a warning when safety score is high")
            .condition(Condition::SafetyScoreAbove(0.5))
            .action(Action::Warn("Input has elevated safety risk".to_string()))
            .build()
        );

        // Sort rules by priority (highest first)
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        info!("Loaded {} default safety rules", rules.len());
    }

    /// Add a rule to the engine
    pub fn add_rule(&self, rule: Rule) {
        let mut rules = self.rules.write();
        rules.push(rule);
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        debug!("Rule added, total rules: {}", rules.len());
    }

    /// Remove a rule by ID
    pub fn remove_rule(&self, rule_id: &str) -> bool {
        let mut rules = self.rules.write();
        let len_before = rules.len();
        rules.retain(|r| r.id != rule_id);
        let removed = rules.len() < len_before;
        if removed {
            debug!("Rule '{}' removed", rule_id);
        }
        removed
    }

    /// Enable/disable a rule
    pub fn set_rule_enabled(&self, rule_id: &str, enabled: bool) -> bool {
        let mut rules = self.rules.write();
        if let Some(rule) = rules.iter_mut().find(|r| r.id == rule_id) {
            rule.enabled = enabled;
            debug!("Rule '{}' {}", rule_id, if enabled { "enabled" } else { "disabled" });
            true
        } else {
            false
        }
    }

    /// Evaluate all rules against a context and return the first matching decision
    pub fn evaluate(&self, ctx: &RuleContext) -> RuleDecision {
        let rules = self.rules.read();

        for rule in rules.iter() {
            if !rule.enabled {
                continue;
            }

            if rule.condition.evaluate(ctx) {
                let decision = self.action_to_decision(&rule.action);
                info!(
                    "Rule '{}' matched: condition fired, action: {:?}",
                    rule.id, decision
                );

                if self.first_match_wins {
                    return decision;
                }
            }
        }

        self.default_decision.clone()
    }

    /// Evaluate and return ALL matching rule decisions (not just first match)
    pub fn evaluate_all(&self, ctx: &RuleContext) -> Vec<(&str, RuleDecision)> {
        let rules = self.rules.read();
        let mut results = Vec::new();

        for rule in rules.iter() {
            if !rule.enabled {
                continue;
            }

            if rule.condition.evaluate(ctx) {
                let decision = self.action_to_decision(&rule.action);
                results.push((rule.id.as_str(), decision));
            }
        }

        // Sort by rule priority (highest first) — already sorted, but ensure
        results.sort_by(|a, b| {
            let pa = rules.iter().find(|r| r.id == a.0).map(|r| r.priority).unwrap_or(0);
            let pb = rules.iter().find(|r| r.id == b.0).map(|r| r.priority).unwrap_or(0);
            pb.cmp(&pa)
        });

        results
    }

    /// Convert an Action to a RuleDecision
    fn action_to_decision(&self, action: &Action) -> RuleDecision {
        match action {
            Action::Allow => RuleDecision::Allow,
            Action::Block(reason) => RuleDecision::Block(reason.clone()),
            Action::RequireConfirmation(msg) => RuleDecision::RequireConfirmation(msg.clone()),
            Action::Warn(msg) => {
                warn!("Rule warning: {}", msg);
                RuleDecision::Allow // Warn doesn't block
            }
            Action::Log(msg) => {
                info!("Rule log: {}", msg);
                RuleDecision::NoMatch // Log doesn't affect decision
            }
            Action::Redirect { target, reason } => {
                RuleDecision::RequireConfirmation(format!("Redirect to '{}': {}", target, reason))
            }
            Action::Transform { transform_type } => {
                debug!("Rule transform: {:?}", transform_type);
                RuleDecision::Allow // Transform doesn't block, but caller should apply transform
            }
        }
    }

    /// Get all registered rules (for inspection/debugging)
    pub fn list_rules(&self) -> Vec<RuleInfo> {
        let rules = self.rules.read();
        rules.iter().map(|r| RuleInfo {
            id: r.id.clone(),
            name: r.name.clone(),
            priority: r.priority,
            enabled: r.enabled,
            description: r.description.clone(),
        }).collect()
    }

    /// Get rule count
    pub fn rule_count(&self) -> usize {
        self.rules.read().len()
    }

    /// Load rules from a loader (YAML/JSON file, database, etc.)
    pub async fn load_from(&self, loader: &dyn RuleLoader) -> anyhow::Result<usize> {
        let rules = loader.load_rules().await?;
        let count = rules.len();
        let mut stored = self.rules.write();
        for rule in rules {
            // Replace existing rule with same ID, or add new
            if let Some(existing) = stored.iter_mut().find(|r| r.id == rule.id) {
                *existing = rule;
            } else {
                stored.push(rule);
            }
        }
        stored.sort_by(|a, b| b.priority.cmp(&a.priority));
        info!("Loaded {} rules from loader, total: {}", count, stored.len());
        Ok(count)
    }

    /// Clear all rules
    pub fn clear(&self) {
        self.rules.write().clear();
        info!("All rules cleared");
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::with_default_rules()
    }
}

/// Lightweight info about a rule (for listing)
#[derive(Debug, Clone, serde::Serialize)]
pub struct RuleInfo {
    pub id: String,
    pub name: String,
    pub priority: i32,
    pub enabled: bool,
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::oracle::SafetyLevel;

    #[test]
    fn test_empty_engine() {
        let engine = RuleEngine::new();
        let ctx = RuleContext::for_bash("ls");
        assert_eq!(engine.evaluate(&ctx), RuleDecision::NoMatch);
    }

    #[test]
    fn test_default_rules_block_rm_rf() {
        let engine = RuleEngine::with_default_rules();
        let ctx = RuleContext::for_bash("rm -rf /");
        let decision = engine.evaluate(&ctx);
        assert!(matches!(decision, RuleDecision::Block(_)));
    }

    #[test]
    fn test_default_rules_allow_safe() {
        let engine = RuleEngine::with_default_rules();
        let ctx = RuleContext::for_bash("ls -la");
        let decision = engine.evaluate(&ctx);
        assert!(matches!(decision, RuleDecision::Allow | RuleDecision::NoMatch));
    }

    #[test]
    fn test_custom_rule() {
        let engine = RuleEngine::new();

        engine.add_rule(Rule::builder("block-npm")
            .name("Block npm")
            .priority(10)
            .condition(Condition::ToolIs("bash".to_string()))
            .action(Action::Block("npm is blocked".to_string()))
            .build()
        );

        let ctx = RuleContext::for_bash("npm install");
        let decision = engine.evaluate(&ctx);
        assert!(matches!(decision, RuleDecision::Block(_)));
    }

    #[test]
    fn test_rule_priority() {
        let engine = RuleEngine::new();

        // Lower priority rule
        engine.add_rule(Rule::builder("allow-all")
            .name("Allow all")
            .priority(1)
            .condition(Condition::Always)
            .action(Action::Allow)
            .build()
        );

        // Higher priority rule
        engine.add_rule(Rule::builder("block-specific")
            .name("Block specific")
            .priority(100)
            .condition(Condition::Contains("rm".to_string()))
            .action(Action::Block("rm blocked".to_string()))
            .build()
        );

        let ctx = RuleContext::for_bash("rm file");
        let decision = engine.evaluate(&ctx);
        assert!(matches!(decision, RuleDecision::Block(_)));
    }

    #[test]
    fn test_rule_enable_disable() {
        let engine = RuleEngine::new();

        engine.add_rule(Rule::builder("test-rule")
            .name("Test")
            .condition(Condition::Always)
            .action(Action::Block("blocked".to_string()))
            .build()
        );

        let ctx = RuleContext::for_bash("ls");
        assert!(matches!(engine.evaluate(&ctx), RuleDecision::Block(_)));

        engine.set_rule_enabled("test-rule", false);
        assert_eq!(engine.evaluate(&ctx), RuleDecision::NoMatch);

        engine.set_rule_enabled("test-rule", true);
        assert!(matches!(engine.evaluate(&ctx), RuleDecision::Block(_)));
    }

    #[test]
    fn test_evaluate_all() {
        let engine = RuleEngine::with_default_rules();
        let ctx = RuleContext::for_bash("sudo rm file");
        let all = engine.evaluate_all(&ctx);
        // Should match both "confirm-dangerous" and the contains check
        assert!(!all.is_empty());
    }

    #[test]
    fn test_list_rules() {
        let engine = RuleEngine::with_default_rules();
        let rules = engine.list_rules();
        assert!(!rules.is_empty());
        assert!(rules.iter().any(|r| r.id == "block-rm-rf"));
    }

    #[test]
    fn test_remove_rule() {
        let engine = RuleEngine::with_default_rules();
        let count_before = engine.rule_count();
        assert!(engine.remove_rule("block-rm-rf"));
        assert_eq!(engine.rule_count(), count_before - 1);
    }

    #[test]
    fn test_composite_condition() {
        let engine = RuleEngine::new();

        // Block only when tool is bash AND command contains "sudo"
        engine.add_rule(Rule::builder("block-sudo")
            .name("Block sudo in bash")
            .priority(50)
            .condition(Condition::All(vec![
                Condition::ToolIs("bash".to_string()),
                Condition::Contains("sudo".to_string()),
            ]))
            .action(Action::Block("sudo not allowed".to_string()))
            .build()
        );

        // Should block
        let ctx = RuleContext::for_bash("sudo apt install");
        assert!(matches!(engine.evaluate(&ctx), RuleDecision::Block(_)));

        // Should NOT block (different tool)
        let ctx2 = RuleContext::for_tool("file", serde_json::json!({ "cmd": "sudo" }));
        assert_eq!(engine.evaluate(&ctx2), RuleDecision::NoMatch);
    }
}
