//! Unified Rule Engine (PRA - Prompt-driven Rule Agent)
//!
//! Provides a centralized rule evaluation system for the entire Crablet framework.
//! Replaces the scattered rule logic in SafetyOracle, BashTool, and SkillTriggerEngine
//! with a unified, extensible rule engine.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────┐
//! │                RuleEngine                      │
//! │                                                │
//! │  ┌─────────┐  ┌──────────┐  ┌──────────────┐ │
//! │  │ Rule    │  │ Condition│  │ Action       │ │
//! │  │ (ID+Pri)│→ │ Evaluator│→ │ Executor     │ │
//! │  └─────────┘  └──────────┘  └──────────────┘ │
//! │       │              │              │         │
//! │       └──────────────┴──────────────┘         │
//! │                     │                          │
//! │              RuleContext                       │
//! │  (user, session, tool, args, history)          │
//! └──────────────────────────────────────────────┘
//! ```

pub mod condition;
pub mod action;
pub mod engine;
pub mod context;
pub mod loader;

pub use engine::RuleEngine;
pub use condition::Condition;
pub use action::Action;
pub use context::RuleContext;

/// A compiled rule ready for evaluation
#[derive(Debug, Clone)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub priority: i32, // Higher = evaluated first
    pub enabled: bool,
    pub condition: Condition,
    pub action: Action,
    pub description: String,
}

/// Result of rule evaluation
#[derive(Debug, Clone, PartialEq)]
pub enum RuleDecision {
    /// Action is explicitly allowed
    Allow,
    /// Action is blocked with reason
    Block(String),
    /// Action requires user confirmation
    RequireConfirmation(String),
    /// No rule matched; default behavior applies
    NoMatch,
}

impl Rule {
    /// Create a new rule builder
    pub fn builder(id: impl Into<String>) -> RuleBuilder {
        RuleBuilder {
            rule: Rule {
                id: id.into(),
                name: String::new(),
                priority: 0,
                enabled: true,
                condition: Condition::Always,
                action: Action::Allow,
                description: String::new(),
            },
        }
    }
}

/// Builder pattern for constructing rules
pub struct RuleBuilder {
    rule: Rule,
}

impl RuleBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.rule.name = name.into();
        self
    }

    pub fn priority(mut self, priority: i32) -> Self {
        self.rule.priority = priority;
        self
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.rule.description = desc.into();
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.rule.enabled = enabled;
        self
    }

    pub fn condition(mut self, condition: Condition) -> Self {
        self.rule.condition = condition;
        self
    }

    pub fn action(mut self, action: Action) -> Self {
        self.rule.action = action;
        self
    }

    pub fn build(self) -> Rule {
        self.rule
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_builder() {
        let rule = Rule::builder("test-rule")
            .name("Test Rule")
            .priority(10)
            .description("A test rule")
            .condition(Condition::Always)
            .action(Action::Allow)
            .build();

        assert_eq!(rule.id, "test-rule");
        assert_eq!(rule.name, "Test Rule");
        assert_eq!(rule.priority, 10);
        assert!(rule.enabled);
    }
}
