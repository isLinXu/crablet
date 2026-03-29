//! Rule Loader
//!
//! Trait and implementations for loading rules from external sources.

use crate::rules::Rule;
use anyhow::Result;

/// Trait for loading rules from external sources
#[async_trait::async_trait]
pub trait RuleLoader: Send + Sync {
    /// Load rules from the source
    async fn load_rules(&self) -> Result<Vec<Rule>>;
}

/// In-memory rule loader (for testing and programmatic configuration)
pub struct InMemoryRuleLoader {
    rules: Vec<Rule>,
}

impl InMemoryRuleLoader {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }
}

#[async_trait::async_trait]
impl RuleLoader for InMemoryRuleLoader {
    async fn load_rules(&self) -> Result<Vec<Rule>> {
        Ok(self.rules.clone())
    }
}

/// YAML file rule loader
pub struct YamlRuleLoader;

impl YamlRuleLoader {
    /// Parse rules from a YAML string
    pub fn parse_yaml(yaml_str: &str) -> Result<Vec<Rule>> {
        let raw_rules: Vec<serde_yaml::Value> = serde_yaml::from_str(yaml_str)
            .map_err(|e| anyhow::anyhow!("Failed to parse YAML rules: {}", e))?;

        let mut rules = Vec::new();
        for raw in &raw_rules {
            let rule = Self::parse_single_rule(raw)?;
            rules.push(rule);
        }

        Ok(rules)
    }

    fn parse_single_rule(value: &serde_yaml::Value) -> Result<Rule> {
        use crate::rules::{Condition, Action};

        let id = value.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Rule missing 'id'"))?
            .to_string();

        let name = value.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&id)
            .to_string();

        let priority = value.get("priority")
            .and_then(|v| v.as_i64())
            .unwrap_or(0) as i32;

        let enabled = value.get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let description = value.get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let condition = Self::parse_condition(value.get("condition"))?;
        let action = Self::parse_action(value.get("action"))?;

        Ok(Rule {
            id,
            name,
            priority,
            enabled,
            condition,
            action,
            description,
        })
    }

    fn parse_condition(value: Option<&serde_yaml::Value>) -> Result<Condition> {
        let value = value.ok_or_else(|| anyhow::anyhow!("Rule missing 'condition'"))?;

        // String shorthand: "contains:rm -rf"
        if let Some(s) = value.as_str() {
            return Ok(Condition::Contains(s.to_string()));
        }

        // Object format
        if let Some(obj) = value.as_mapping() {
            let type_str = obj.get(&serde_yaml::Value::String("type".to_string()))
                .and_then(|v| v.as_str())
                .unwrap_or("contains");

            let param = obj.get(&serde_yaml::Value::String("value".to_string()))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            return match type_str {
                "always" => Ok(Condition::Always),
                "never" => Ok(Condition::Never),
                "contains" => Ok(Condition::Contains(param.to_string())),
                "starts_with" => Ok(Condition::StartsWith(param.to_string())),
                "ends_with" => Ok(Condition::EndsWith(param.to_string())),
                "tool_is" => Ok(Condition::ToolIs(param.to_string())),
                "any_tool" => Ok(Condition::AnyTool),
                "regex" => Ok(Condition::regex_str(param)),
                "safety_above" => {
                    let threshold: f32 = param.parse().unwrap_or(0.5);
                    Ok(Condition::SafetyScoreAbove(threshold))
                }
                "safety_below" => {
                    let threshold: f32 = param.parse().unwrap_or(0.5);
                    Ok(Condition::SafetyScoreBelow(threshold))
                }
                other => Err(anyhow::anyhow!("Unknown condition type: {}", other)),
            };
        }

        Err(anyhow::anyhow!("Invalid condition format"))
    }

    fn parse_action(value: Option<&serde_yaml::Value>) -> Result<Action> {
        let value = value.ok_or_else(|| anyhow::anyhow!("Rule missing 'action'"))?;

        // String shorthand
        if let Some(s) = value.as_str() {
            return match s {
                "allow" => Ok(Action::Allow),
                "no_match" => Ok(Action::Log("No action".to_string())),
                other => Ok(Action::Block(other.to_string())),
            };
        }

        // Object format
        if let Some(obj) = value.as_mapping() {
            let type_str = obj.get(&serde_yaml::Value::String("type".to_string()))
                .and_then(|v| v.as_str())
                .unwrap_or("allow");

            let message = obj.get(&serde_yaml::Value::String("message".to_string()))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            return match type_str {
                "allow" => Ok(Action::Allow),
                "block" => Ok(Action::Block(message.to_string())),
                "confirm" => Ok(Action::RequireConfirmation(message.to_string())),
                "warn" => Ok(Action::Warn(message.to_string())),
                "log" => Ok(Action::Log(message.to_string())),
                other => Err(anyhow::anyhow!("Unknown action type: {}", other)),
            };
        }

        Err(anyhow::anyhow!("Invalid action format"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Rule, Condition, Action};

    #[test]
    fn test_yaml_parse_simple() {
        let yaml = r#"
- id: block-rm
  name: Block rm
  priority: 100
  description: Block rm commands
  condition: "rm "
  action:
    type: block
    message: "rm is not allowed"
"#;

        let rules = YamlRuleLoader::parse_yaml(yaml).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, "block-rm");
        assert_eq!(rules[0].priority, 100);
    }

    #[test]
    fn test_yaml_parse_multiple() {
        let yaml = r#"
- id: rule1
  name: Rule 1
  priority: 10
  condition: "sudo"
  action:
    type: confirm
    message: "sudo requires confirmation"

- id: rule2
  name: Rule 2
  priority: 20
  condition:
    type: tool_is
    value: "bash"
  action: allow
"#;

        let rules = YamlRuleLoader::parse_yaml(yaml).unwrap();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].id, "rule1");
        assert_eq!(rules[1].id, "rule2");
    }

    #[test]
    fn test_in_memory_loader() {
        let rules = vec![
            Rule::builder("test")
                .name("Test")
                .condition(Condition::Always)
                .action(Action::Allow)
                .build()
        ];

        let loader = InMemoryRuleLoader::new(rules);
        let rt = tokio::runtime::Runtime::new().unwrap();
        let loaded = rt.block_on(loader.load_rules()).unwrap();
        assert_eq!(loaded.len(), 1);
    }
}
