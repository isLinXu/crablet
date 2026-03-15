//! Skill Trigger System
//! 
//! Provides multiple trigger types for skill activation:
//! - Keyword: Exact keyword matching
//! - Regex: Regular expression matching
//! - Intent: Intent classification matching
//! - Semantic: Vector similarity matching
//! - Command: Command prefix matching

use serde::{Deserialize, Serialize};
use regex::Regex;
use std::collections::HashMap;

/// Skill trigger types for automatic skill activation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SkillTrigger {
    /// Exact keyword matching
    Keyword { 
        keywords: Vec<String>,
        #[serde(default)]
        case_sensitive: bool,
    },
    /// Regular expression matching
    Regex { 
        pattern: String,
        #[serde(skip)]
        #[serde(default)]
        compiled: Option<Regex>,
    },
    /// Intent matching (integrates with classifier)
    Intent { 
        intent: String,
        #[serde(default = "default_confidence_threshold")]
        confidence_threshold: f32,
    },
    /// Semantic matching (based on vector similarity)
    Semantic {
        description: String,
        #[serde(default = "default_semantic_threshold")]
        threshold: f32,
    },
    /// Command prefix matching
    Command {
        prefix: String,
        #[serde(default)]
        args_schema: Option<serde_json::Value>,
    },
}

fn default_confidence_threshold() -> f32 {
    0.7
}

fn default_semantic_threshold() -> f32 {
    0.75
}

impl SkillTrigger {
    /// Compile regex patterns for efficient matching
    pub fn compile(&mut self) -> anyhow::Result<()> {
        if let SkillTrigger::Regex { pattern, compiled } = self {
            *compiled = Some(Regex::new(pattern)?);
        }
        Ok(())
    }
    
    /// Get the trigger type name
    pub fn type_name(&self) -> &'static str {
        match self {
            SkillTrigger::Keyword { .. } => "keyword",
            SkillTrigger::Regex { .. } => "regex",
            SkillTrigger::Intent { .. } => "intent",
            SkillTrigger::Semantic { .. } => "semantic",
            SkillTrigger::Command { .. } => "command",
        }
    }
}

/// Result of a trigger match
#[derive(Debug, Clone)]
pub struct TriggerMatch {
    pub skill_name: String,
    pub trigger_type: String,
    pub confidence: f32,
    pub extracted_args: Option<serde_json::Value>,
    pub matched_text: Option<String>,
}

/// Engine for matching user input against skill triggers
pub struct SkillTriggerEngine {
    triggers: Vec<(String, SkillTrigger)>,
}

impl Default for SkillTriggerEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillTriggerEngine {
    /// Create a new trigger engine
    pub fn new() -> Self {
        Self {
            triggers: Vec::new(),
        }
    }
    
    /// Register a skill trigger
    pub fn register(&mut self, skill_name: String, trigger: SkillTrigger) {
        self.triggers.push((skill_name, trigger));
    }
    
    /// Register multiple triggers for a skill
    pub fn register_triggers(&mut self, skill_name: String, triggers: Vec<SkillTrigger>) {
        for trigger in triggers {
            self.triggers.push((skill_name.clone(), trigger));
        }
    }
    
    /// Remove all triggers for a skill
    pub fn unregister(&mut self, skill_name: &str) {
        self.triggers.retain(|(name, _)| name != skill_name);
    }
    
    /// Match input against all registered triggers
    pub fn match_input(&self, input: &str) -> Vec<TriggerMatch> {
        let mut matches = Vec::new();
        
        for (skill_name, trigger) in &self.triggers {
            if let Some(m) = self.evaluate_trigger(input, skill_name, trigger) {
                matches.push(m);
            }
        }
        
        // Sort by confidence (highest first)
        matches.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        matches
    }
    
    /// Match input and return only the best match above threshold
    pub fn match_best(&self, input: &str, threshold: f32) -> Option<TriggerMatch> {
        let matches = self.match_input(input);
        matches.into_iter().find(|m| m.confidence >= threshold)
    }
    
    /// Evaluate a single trigger against input
    fn evaluate_trigger(&self, input: &str, skill_name: &str, trigger: &SkillTrigger) -> Option<TriggerMatch> {
        match trigger {
            SkillTrigger::Keyword { keywords, case_sensitive } => {
                self.evaluate_keyword(input, skill_name, keywords, *case_sensitive)
            }
            SkillTrigger::Regex { pattern, compiled } => {
                self.evaluate_regex(input, skill_name, pattern, compiled.as_ref())
            }
            SkillTrigger::Intent { intent, confidence_threshold } => {
                self.evaluate_intent(input, skill_name, intent, *confidence_threshold)
            }
            SkillTrigger::Semantic { description, threshold } => {
                self.evaluate_semantic(input, skill_name, description, *threshold)
            }
            SkillTrigger::Command { prefix, args_schema } => {
                self.evaluate_command(input, skill_name, prefix, args_schema.as_ref())
            }
        }
    }
    
    /// Evaluate keyword trigger
    fn evaluate_keyword(
        &self, 
        input: &str, 
        skill_name: &str, 
        keywords: &[String], 
        case_sensitive: bool
    ) -> Option<TriggerMatch> {
        let input_to_check = if case_sensitive { 
            input.to_string() 
        } else { 
            input.to_lowercase() 
        };
        
        for kw in keywords {
            let kw_to_check = if case_sensitive { 
                kw.clone() 
            } else { 
                kw.to_lowercase() 
            };
            
            if input_to_check.contains(&kw_to_check) {
                return Some(TriggerMatch {
                    skill_name: skill_name.to_string(),
                    trigger_type: "keyword".to_string(),
                    confidence: 0.8,
                    extracted_args: None,
                    matched_text: Some(kw.clone()),
                });
            }
        }
        None
    }
    
    /// Evaluate regex trigger
    fn evaluate_regex(
        &self, 
        input: &str, 
        skill_name: &str, 
        _pattern: &str,
        compiled: Option<&Regex>
    ) -> Option<TriggerMatch> {
        if let Some(regex) = compiled {
            if let Some(captures) = regex.captures(input) {
                // Extract named groups as args
                let mut args = HashMap::new();
                for name in regex.capture_names().flatten() {
                    if let Some(value) = captures.name(name) {
                        args.insert(name.to_string(), serde_json::json!(value.as_str()));
                    }
                }
                
                return Some(TriggerMatch {
                    skill_name: skill_name.to_string(),
                    trigger_type: "regex".to_string(),
                    confidence: 0.9,
                    extracted_args: if args.is_empty() { None } else { Some(serde_json::json!(args)) },
                    matched_text: Some(captures.get(0)?.as_str().to_string()),
                });
            }
        }
        None
    }
    
    /// Evaluate intent trigger (placeholder - requires classifier integration)
    fn evaluate_intent(
        &self, 
        _input: &str, 
        _skill_name: &str, 
        _intent: &str, 
        _threshold: f32
    ) -> Option<TriggerMatch> {
        // This requires integration with the intent classifier
        // For now, return None - will be implemented when classifier is available
        None
    }
    
    /// Evaluate semantic trigger (placeholder - requires vector store)
    fn evaluate_semantic(
        &self, 
        _input: &str, 
        _skill_name: &str, 
        _description: &str, 
        _threshold: f32
    ) -> Option<TriggerMatch> {
        // This requires vector embedding and similarity calculation
        // For now, return None - will be implemented with vector store
        None
    }
    
    /// Evaluate command trigger
    fn evaluate_command(
        &self, 
        input: &str, 
        skill_name: &str, 
        prefix: &str,
        args_schema: Option<&serde_json::Value>
    ) -> Option<TriggerMatch> {
        if input.starts_with(prefix) {
            let args_str = input.strip_prefix(prefix).unwrap_or("").trim();
            
            let extracted = if let Some(schema) = args_schema {
                Self::parse_args(args_str, schema)
            } else {
                serde_json::json!({ "args": args_str })
            };
            
            return Some(TriggerMatch {
                skill_name: skill_name.to_string(),
                trigger_type: "command".to_string(),
                confidence: 0.95,
                extracted_args: Some(extracted),
                matched_text: Some(prefix.to_string()),
            });
        }
        None
    }
    
    /// Parse command arguments based on schema
    fn parse_args(args_str: &str, schema: &serde_json::Value) -> serde_json::Value {
        let mut result = serde_json::Map::new();
        
        // Simple parsing: try to extract key=value pairs or positional args
        if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
            let keys: Vec<&String> = properties.keys().collect();
            
            // Try key=value format first
            for (key, _) in properties {
                let pattern = format!("{}=", key);
                if let Some(start) = args_str.find(&pattern) {
                    let value_start = start + pattern.len();
                    let value_end = args_str[value_start..]
                        .find(' ')
                        .map(|i| value_start + i)
                        .unwrap_or(args_str.len());
                    let value = &args_str[value_start..value_end];
                    result.insert(key.clone(), serde_json::json!(value));
                }
            }
            
            // If no key=value found, use positional args
            if result.is_empty() && !keys.is_empty() {
                let parts: Vec<&str> = args_str.split_whitespace().collect();
                for (i, key) in keys.iter().enumerate() {
                    if i < parts.len() {
                        result.insert((*key).clone(), serde_json::json!(parts[i]));
                    }
                }
            }
        } else {
            // No schema, just store raw args
            result.insert("args".to_string(), serde_json::json!(args_str));
        }
        
        serde_json::Value::Object(result)
    }
    
    /// Get all registered skill names
    pub fn list_skills(&self) -> Vec<String> {
        self.triggers.iter().map(|(name, _)| name.clone()).collect()
    }
    
    /// Clear all triggers
    pub fn clear(&mut self) {
        self.triggers.clear();
    }
    
    /// Get trigger count
    pub fn len(&self) -> usize {
        self.triggers.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.triggers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_trigger() {
        let mut engine = SkillTriggerEngine::new();
        engine.register("weather".to_string(), SkillTrigger::Keyword {
            keywords: vec!["天气".to_string(), "weather".to_string()],
            case_sensitive: false,
        });
        
        let matches = engine.match_input("今天天气怎么样？");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].skill_name, "weather");
        assert_eq!(matches[0].confidence, 0.8);
    }

    #[test]
    fn test_command_trigger() {
        let mut engine = SkillTriggerEngine::new();
        engine.register("search".to_string(), SkillTrigger::Command {
            prefix: "/search".to_string(),
            args_schema: None,
        });
        
        let matches = engine.match_input("/search rust tutorial");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].skill_name, "search");
        assert_eq!(matches[0].extracted_args, Some(serde_json::json!({"args": "rust tutorial"})));
    }

    #[test]
    fn test_command_trigger_with_schema() {
        let mut engine = SkillTriggerEngine::new();
        engine.register("weather".to_string(), SkillTrigger::Command {
            prefix: "/weather".to_string(),
            args_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "location": { "type": "string" },
                    "units": { "type": "string" }
                }
            })),
        });
        
        let matches = engine.match_input("/weather Beijing metric");
        assert_eq!(matches.len(), 1);
        let args = matches[0].extracted_args.as_ref().unwrap();
        assert_eq!(args["location"], "Beijing");
        assert_eq!(args["units"], "metric");
    }

    #[test]
    fn test_regex_trigger() {
        let mut engine = SkillTriggerEngine::new();
        let mut trigger = SkillTrigger::Regex {
            pattern: r"计算\s*(\d+)\s*\+\s*(\d+)".to_string(),
            compiled: None,
        };
        trigger.compile().unwrap();
        engine.register("calculator".to_string(), trigger);
        
        let matches = engine.match_input("计算 5 + 3");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].skill_name, "calculator");
    }

    #[test]
    fn test_multiple_triggers_same_skill() {
        let mut engine = SkillTriggerEngine::new();
        engine.register_triggers("weather".to_string(), vec![
            SkillTrigger::Keyword {
                keywords: vec!["天气".to_string()],
                case_sensitive: false,
            },
            SkillTrigger::Command {
                prefix: "/weather".to_string(),
                args_schema: None,
            },
        ]);
        
        let matches_keyword = engine.match_input("今天天气如何？");
        assert_eq!(matches_keyword.len(), 1);
        assert_eq!(matches_keyword[0].trigger_type, "keyword");
        
        let matches_command = engine.match_input("/weather Beijing");
        assert_eq!(matches_command.len(), 1);
        assert_eq!(matches_command[0].trigger_type, "command");
    }

    #[test]
    fn test_best_match() {
        let mut engine = SkillTriggerEngine::new();
        engine.register("weather".to_string(), SkillTrigger::Keyword {
            keywords: vec!["天气".to_string()],
            case_sensitive: false,
        });
        engine.register("search".to_string(), SkillTrigger::Command {
            prefix: "/search".to_string(),
            args_schema: None,
        });
        
        // Command has higher confidence (0.95) than keyword (0.8)
        let best = engine.match_best("/search weather in Beijing", 0.7);
        assert!(best.is_some());
        assert_eq!(best.unwrap().skill_name, "search");
    }
}
