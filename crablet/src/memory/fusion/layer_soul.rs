//! L4: SOUL Layer - Immutable Core
//!
//! The SOUL layer represents the immutable identity and core values of the agent.
//! This layer is loaded once at startup and never changes during runtime.
//! It defines:
//! - Agent identity (name, description, role)
//! - Core values and principles
//! - Immutable rules that must never be violated
//! - Behavioral guidelines

use serde::{Deserialize, Serialize};
use tracing::{info, debug};

use crate::memory::fusion::MemoryError;

/// Core value definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreValue {
    pub name: String,
    pub description: String,
    pub priority: u8,
    pub category: String,
}

/// Immutable rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImmutableRule {
    pub rule: String,
    pub reason: Option<String>,
}

/// Agent identity from config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentityConfig {
    pub name: String,
    pub description: String,
    pub role: String,
    pub version: String,
}

/// Soul metadata from config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulMetadataConfig {
    pub created_at: String,
    pub updated_at: String,
    pub author: String,
}

/// Soul configuration (local definition to avoid dependency issues)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulConfig {
    pub identity: AgentIdentityConfig,
    pub core_values: Vec<CoreValue>,
    pub immutable_rules: Vec<ImmutableRule>,
    pub metadata: SoulMetadataConfig,
}

/// L4 SOUL Layer - Immutable core identity
///
/// This struct represents the agent's fundamental identity and values.
/// Once loaded from configuration, it remains immutable throughout the session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulLayer {
    /// Agent identity information
    identity: AgentIdentity,
    
    /// Core values sorted by priority
    core_values: Vec<CoreValue>,
    
    /// Immutable rules that must never be violated
    immutable_rules: Vec<ImmutableRule>,
    
    /// Behavioral guidelines
    guidelines: Vec<Guideline>,
    
    /// Metadata
    metadata: SoulMetadata,
}

/// Agent identity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    /// Agent name
    pub name: String,
    
    /// Agent description
    pub description: String,
    
    /// Agent role/persona
    pub role: String,
    
    /// Version of the SOUL configuration
    pub version: String,
}

/// Behavioral guideline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Guideline {
    /// Category of the guideline
    pub category: String,
    
    /// Description of expected behavior
    pub description: String,
    
    /// Examples of correct behavior
    pub examples: Vec<String>,
}

/// SOUL metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulMetadata {
    /// When the SOUL was created
    pub created_at: String,
    
    /// Last updated timestamp
    pub updated_at: String,
    
    /// Configuration hash for integrity checking
    pub config_hash: String,
}

impl SoulLayer {
    /// Create SOUL layer from configuration
    pub fn from_config(config: &SoulConfig) -> Result<Self, MemoryError> {
        info!("Loading SOUL layer from configuration...");
        
        // Sort core values by priority (highest first)
        let mut core_values = config.core_values.clone();
        core_values.sort_by(|a, b| b.priority.cmp(&a.priority));
        
        let identity = AgentIdentity {
            name: config.identity.name.clone(),
            description: config.identity.description.clone(),
            role: config.identity.role.clone(),
            version: config.identity.version.clone(),
        };
        
        // Convert immutable rules
        let immutable_rules = config.immutable_rules.clone();
        
        // Build guidelines from config
        let guidelines = Self::build_guidelines(config);
        
        let metadata = SoulMetadata {
            created_at: config.metadata.created_at.clone(),
            updated_at: config.metadata.updated_at.clone(),
            config_hash: Self::compute_config_hash(config),
        };
        
        let soul = Self {
            identity,
            core_values,
            immutable_rules,
            guidelines,
            metadata,
        };
        
        // Validate the SOUL layer
        soul.validate()?;
        
        info!(
            "SOUL layer loaded: {} (v{})",
            soul.identity.name,
            soul.identity.version
        );
        debug!(
            "Core values: {}, Immutable rules: {}",
            soul.core_values.len(),
            soul.immutable_rules.len()
        );
        
        Ok(soul)
    }
    
    /// Build behavioral guidelines from configuration
    fn build_guidelines(_config: &SoulConfig) -> Vec<Guideline> {
        let mut guidelines = Vec::new();
        
        // Communication guidelines
        guidelines.push(Guideline {
            category: "communication".to_string(),
            description: "How to communicate with users".to_string(),
            examples: vec![
                "Be clear and concise".to_string(),
                "Use appropriate tone based on user preference".to_string(),
                "Ask clarifying questions when needed".to_string(),
            ],
        });
        
        // Problem-solving guidelines
        guidelines.push(Guideline {
            category: "problem_solving".to_string(),
            description: "How to approach problems".to_string(),
            examples: vec![
                "Break complex problems into smaller steps".to_string(),
                "Consider multiple approaches".to_string(),
                "Explain your reasoning".to_string(),
            ],
        });
        
        // Learning guidelines
        guidelines.push(Guideline {
            category: "learning".to_string(),
            description: "How to learn and adapt".to_string(),
            examples: vec![
                "Learn from user feedback".to_string(),
                "Remember user preferences".to_string(),
                "Improve over time".to_string(),
            ],
        });
        
        guidelines
    }
    
    /// Validate the SOUL layer
    fn validate(&self) -> Result<(), MemoryError> {
        // Check required fields
        if self.identity.name.is_empty() {
            return Err(MemoryError::ConfigError(
                "SOUL identity name cannot be empty".to_string()
            ));
        }
        
        if self.identity.description.is_empty() {
            return Err(MemoryError::ConfigError(
                "SOUL identity description cannot be empty".to_string()
            ));
        }
        
        // Check for at least one core value
        if self.core_values.is_empty() {
            return Err(MemoryError::ConfigError(
                "SOUL must have at least one core value".to_string()
            ));
        }
        
        // Check for at least one immutable rule
        if self.immutable_rules.is_empty() {
            return Err(MemoryError::ConfigError(
                "SOUL must have at least one immutable rule".to_string()
            ));
        }
        
        // Validate core value priorities (should be 1-10)
        for value in &self.core_values {
            if value.priority < 1 || value.priority > 10 {
                return Err(MemoryError::ConfigError(format!(
                    "Core value '{}' has invalid priority: {} (must be 1-10)",
                    value.name, value.priority
                )));
            }
        }
        
        Ok(())
    }
    
    /// Compute configuration hash for integrity checking
    fn compute_config_hash(config: &SoulConfig) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        config.identity.name.hash(&mut hasher);
        config.identity.version.hash(&mut hasher);
        config.core_values.len().hash(&mut hasher);
        config.immutable_rules.len().hash(&mut hasher);
        
        format!("{:x}", hasher.finish())
    }
    
    /// Get agent identity
    pub fn identity(&self) -> &AgentIdentity {
        &self.identity
    }
    
    /// Get core values (sorted by priority)
    pub fn core_values(&self) -> &[CoreValue] {
        &self.core_values
    }
    
    /// Get immutable rules
    pub fn immutable_rules(&self) -> &[ImmutableRule] {
        &self.immutable_rules
    }
    
    /// Get behavioral guidelines
    pub fn guidelines(&self) -> &[Guideline] {
        &self.guidelines
    }
    
    /// Get metadata
    pub fn metadata(&self) -> &SoulMetadata {
        &self.metadata
    }
    
    /// Check if an action violates any immutable rule
    pub fn check_action(&self, action_description: &str) -> ActionCheckResult {
        for rule in &self.immutable_rules {
            if Self::action_matches_rule(action_description, &rule.rule) {
                return ActionCheckResult::Violation {
                    rule: rule.clone(),
                    action: action_description.to_string(),
                };
            }
        }
        
        ActionCheckResult::Allowed
    }
    
    /// Check if action description matches a rule (simple keyword matching)
    fn action_matches_rule(action: &str, rule: &str) -> bool {
        let action_lower = action.to_lowercase();
        let rule_lower = rule.to_lowercase();
        
        // Check for prohibited keywords
        let prohibited: Vec<&str> = rule_lower
            .split("never")
            .nth(1)
            .unwrap_or("")
            .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .filter(|s| !s.is_empty())
            .collect();
        
        for keyword in prohibited {
            if action_lower.contains(keyword) {
                return true;
            }
        }
        
        false
    }
    
    /// Get the highest priority core value
    pub fn highest_priority_value(&self) -> Option<&CoreValue> {
        self.core_values.first()
    }
    
    /// Get core values by category
    pub fn values_by_category(&self, category: &str) -> Vec<&CoreValue> {
        self.core_values
            .iter()
            .filter(|v| v.category == category)
            .collect()
    }
    
    /// Generate system prompt from SOUL
    pub fn to_system_prompt(&self) -> String {
        let mut parts = vec![
            format!("You are {}, {}", self.identity.name, self.identity.description),
            format!("Your role: {}", self.identity.role),
            String::new(),
            "Core Values (in priority order):".to_string(),
        ];
        
        for value in &self.core_values {
            parts.push(format!(
                "- {} [{}]: {} (Priority: {})",
                value.name, value.category, value.description, value.priority
            ));
        }
        
        parts.push(String::new());
        parts.push("Immutable Rules (you must NEVER violate these):".to_string());
        
        for rule in &self.immutable_rules {
            parts.push(format!("- {}", rule.rule));
            if let Some(ref reason) = rule.reason {
                parts.push(format!("  Reason: {}", reason));
            }
        }
        
        parts.push(String::new());
        parts.push("Guidelines:".to_string());
        
        for guideline in &self.guidelines {
            parts.push(format!("- {}: {}", guideline.category, guideline.description));
            for example in &guideline.examples {
                parts.push(format!("  * {}", example));
            }
        }
        
        parts.join("\n")
    }
    
    /// Get SOUL statistics
    pub fn stats(&self) -> SoulStats {
        SoulStats {
            name: self.identity.name.clone(),
            version: self.identity.version.clone(),
            core_values_count: self.core_values.len(),
            immutable_rules_count: self.immutable_rules.len(),
            guidelines_count: self.guidelines.len(),
            config_hash: self.metadata.config_hash.clone(),
        }
    }
}

/// Action check result
#[derive(Debug, Clone)]
pub enum ActionCheckResult {
    /// Action is allowed
    Allowed,
    /// Action violates an immutable rule
    Violation {
        rule: ImmutableRule,
        action: String,
    },
}

impl ActionCheckResult {
    /// Check if action is allowed
    pub fn is_allowed(&self) -> bool {
        matches!(self, ActionCheckResult::Allowed)
    }
    
    /// Check if action is a violation
    pub fn is_violation(&self) -> bool {
        matches!(self, ActionCheckResult::Violation { .. })
    }
    
    /// Get violation details if applicable
    pub fn violation_details(&self) -> Option<(&ImmutableRule, &str)> {
        match self {
            ActionCheckResult::Violation { rule, action } => Some((rule, action)),
            _ => None,
        }
    }
}

/// SOUL statistics
#[derive(Debug, Clone)]
pub struct SoulStats {
    pub name: String,
    pub version: String,
    pub core_values_count: usize,
    pub immutable_rules_count: usize,
    pub guidelines_count: usize,
    pub config_hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_config() -> SoulConfig {
        SoulConfig {
            identity: AgentIdentityConfig {
                name: "TestAgent".to_string(),
                description: "A test agent".to_string(),
                role: "assistant".to_string(),
                version: "1.0.0".to_string(),
            },
            core_values: vec![
                CoreValue {
                    name: "Honesty".to_string(),
                    description: "Always be truthful".to_string(),
                    priority: 10,
                    category: "ethics".to_string(),
                },
            ],
            immutable_rules: vec![
                ImmutableRule {
                    rule: "Never harm humans".to_string(),
                    reason: Some("Safety first".to_string()),
                },
            ],
            metadata: SoulMetadataConfig {
                created_at: "2024-01-01".to_string(),
                updated_at: "2024-01-01".to_string(),
                author: "test".to_string(),
            },
        }
    }
    
    #[test]
    fn test_soul_layer_creation() {
        let config = create_test_config();
        let soul = SoulLayer::from_config(&config).unwrap();
        
        assert_eq!(soul.identity().name, "TestAgent");
        assert_eq!(soul.core_values().len(), 1);
        assert_eq!(soul.immutable_rules().len(), 1);
    }
    
    #[test]
    fn test_action_check_allowed() {
        let config = create_test_config();
        let soul = SoulLayer::from_config(&config).unwrap();
        
        let result = soul.check_action("Help the user with coding");
        assert!(result.is_allowed());
    }
    
    #[test]
    fn test_system_prompt_generation() {
        let config = create_test_config();
        let soul = SoulLayer::from_config(&config).unwrap();
        
        let prompt = soul.to_system_prompt();
        assert!(prompt.contains("TestAgent"));
        assert!(prompt.contains("Core Values"));
        assert!(prompt.contains("Immutable Rules"));
    }
}
