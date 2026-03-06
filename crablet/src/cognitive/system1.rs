use crate::cognitive::CognitiveSystem;
use crate::types::{Message, TraceStep};
use crate::error::{Result, CrabletError};
use async_trait::async_trait;
use std::sync::Arc;
use strsim::levenshtein;
use std::collections::HashMap;

#[derive(Clone)]
struct CommandRule {
    primary_command: String,
    aliases: Vec<String>,
    #[allow(dead_code)]
    description: String,
    handler: Arc<dyn Fn(&str) -> String + Send + Sync>,
}

#[derive(Clone)]
pub struct System1 {
    rules: Vec<CommandRule>,
    intent_trie: Arc<IntentTrie>,
}

struct IntentTrieNode {
    children: HashMap<char, IntentTrieNode>,
    is_end: bool,
    intent: Option<String>,
}

impl IntentTrieNode {
    fn new() -> Self {
        Self {
            children: HashMap::new(),
            is_end: false,
            intent: None,
        }
    }
}

struct IntentTrie {
    root: IntentTrieNode,
}

impl IntentTrie {
    fn new() -> Self {
        Self {
            root: IntentTrieNode::new(),
        }
    }

    fn insert(&mut self, text: &str, intent: &str) {
        let mut node = &mut self.root;
        for c in text.to_lowercase().chars() {
            node = node.children.entry(c).or_insert_with(IntentTrieNode::new);
        }
        node.is_end = true;
        node.intent = Some(intent.to_string());
    }

    fn search(&self, text: &str) -> Option<String> {
        let mut node = &self.root;
        for c in text.to_lowercase().chars() {
            if let Some(n) = node.children.get(&c) {
                node = n;
            } else {
                return None;
            }
        }
        if node.is_end {
            node.intent.clone()
        } else {
            None
        }
    }
}

impl Default for System1 {
    fn default() -> Self {
        Self::new()
    }
}

impl System1 {
    pub fn new() -> Self {
        let mut rules = Vec::new();
        let mut trie = IntentTrie::new();

        // 1. Define Code-as-Policy Handlers
        
        // Greeting Handler
        let greeting_handler = Arc::new(|_: &str| -> String {
            "你好！我是 Crablet，你的智能助手。".to_string()
        });
        
        let identity_handler = Arc::new(|_: &str| -> String {
            "我是 Crablet，一个基于大模型的智能助手。我能够帮助你完成各种任务，比如搜索信息、执行命令、创建技能等。有什么我可以帮你的吗？".to_string()
        });

        rules.push(CommandRule {
            primary_command: "hello".to_string(),
            aliases: vec!["hi".to_string(), "hey".to_string(), "你好".to_string(), "您好".to_string()],
            description: "Say hello".to_string(),
            handler: greeting_handler.clone(),
        });
        
        rules.push(CommandRule {
            primary_command: "identity".to_string(),
            aliases: vec!["who are you".to_string(), "你是谁".to_string(), "what is your name".to_string(), "你叫什么".to_string()],
            description: "Identity".to_string(),
            handler: identity_handler.clone(),
        });
        
        // Help Handler
        let help_handler = Arc::new(|_: &str| -> String {
            "Available commands:\n- /help: Show this message\n- /status: Check system status\n- /exit: Quit session".to_string()
        });
        
        rules.push(CommandRule {
            primary_command: "help".to_string(),
            aliases: vec!["/help".to_string(), "帮助".to_string()],
            description: "Show help".to_string(),
            handler: help_handler,
        });

        // Status Handler
        let status_handler = Arc::new(|_: &str| -> String {
            "System Status: ONLINE. All subsystems operational.".to_string()
        });
        
        rules.push(CommandRule {
            primary_command: "status".to_string(),
            aliases: vec!["/status".to_string(), "stats".to_string(), "状态".to_string()],
            description: "Check status".to_string(),
            handler: status_handler,
        });
        
        // Build Trie from rules
        for rule in &rules {
            trie.insert(&rule.primary_command, &rule.primary_command);
            for alias in &rule.aliases {
                trie.insert(alias, &rule.primary_command);
            }
        }

        Self {
            rules,
            intent_trie: Arc::new(trie),
        }
    }
    
    fn fuzzy_match(&self, input: &str) -> Option<&CommandRule> {
        let input_lower = input.to_lowercase();
        let input_cmd = input_lower.split_whitespace().next().unwrap_or("");
        
        let mut best_match: Option<&CommandRule> = None;
        let mut min_dist = usize::MAX;
        
        for rule in &self.rules {
            // Check primary
            let dist = levenshtein(input_cmd, &rule.primary_command);
            if dist < min_dist {
                min_dist = dist;
                best_match = Some(rule);
            }
            
            // Check aliases
            for alias in &rule.aliases {
                let dist = levenshtein(input_cmd, alias);
                if dist < min_dist {
                    min_dist = dist;
                    best_match = Some(rule);
                }
            }
        }
        
        // Threshold: allow 1 error for short commands, 2 for longer
        let threshold = if input_cmd.len() < 4 { 0 } else if input_cmd.len() < 7 { 1 } else { 2 };
        
        if min_dist <= threshold {
            best_match
        } else {
            None
        }
    }
}

#[async_trait]
impl CognitiveSystem for System1 {
    fn name(&self) -> &str {
        "System 1 (Intuitive)"
    }

    async fn process(&self, input: &str, _context: &[Message]) -> Result<(String, Vec<TraceStep>)> {
        let input_trim = input.trim();
        
        // 1. Trie Match (Exact Prefix O(L))
        // We only check the first word for command dispatch
        let first_word = input_trim.split_whitespace().next().unwrap_or("");
        
        // Try searching the whole input first (for multi-word commands like "who are you")
        // But our Trie is currently word-based in insertion? No, it's char based.
        // But we insert "who are you".
        // If we search "who", it won't match "who are you" because "who" is not end.
        // If we search "who are you", it matches.
        
        // Strategy: Check exact match of whole string, then first word.
        
        if let Some(cmd_key) = self.intent_trie.search(input_trim) {
            if let Some(rule) = self.rules.iter().find(|r| r.primary_command == cmd_key) {
                 return Ok(((rule.handler)(input_trim), vec![TraceStep {
                    step: 0,
                    thought: format!("System 1 Trie Hit: {}", cmd_key),
                    action: Some("FastRespond".to_string()),
                    action_input: Some(input_trim.to_string()),
                    observation: Some("Executed".to_string()),
                }]));
            }
        }
        
        if let Some(cmd_key) = self.intent_trie.search(first_word) {
            if let Some(rule) = self.rules.iter().find(|r| r.primary_command == cmd_key) {
                 return Ok(((rule.handler)(input_trim), vec![TraceStep {
                    step: 0,
                    thought: format!("System 1 Trie Hit: {}", cmd_key),
                    action: Some("FastRespond".to_string()),
                    action_input: Some(input_trim.to_string()),
                    observation: Some("Executed".to_string()),
                }]));
            }
        }
        
        // 2. Fuzzy Match (Levenshtein)
        if let Some(rule) = self.fuzzy_match(input_trim) {
             return Ok(((rule.handler)(input_trim), vec![TraceStep {
                step: 0,
                thought: format!("System 1 Fuzzy Hit: {} (Matched {})", input_trim, rule.primary_command),
                action: Some("FastRespond".to_string()),
                action_input: Some(input_trim.to_string()),
                observation: Some("Executed".to_string()),
            }]));
        }
        
        // 3. Regex Fallback (Legacy patterns if needed, or remove if Trie covers all)
        // Keeping regex for complex patterns not covered by simple command prefix
        // e.g. "what time is it"
        
        // ... (Optional: Regex logic can be added here if needed)

        // If no match, return error to fall through to System 2
        Err(CrabletError::NotFound("No intuitive match found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_trie_insert_search() {
        let mut trie = IntentTrie::new();
        trie.insert("help", "help_intent");
        trie.insert("hello", "greet_intent");
        
        assert_eq!(trie.search("help"), Some("help_intent".to_string()));
        assert_eq!(trie.search("hello"), Some("greet_intent".to_string()));
        assert_eq!(trie.search("he"), None);
        assert_eq!(trie.search("helpme"), None);
    }
    
    #[test]
    fn test_intent_trie_case_insensitive() {
        let mut trie = IntentTrie::new();
        trie.insert("Help", "help_intent");
        
        assert_eq!(trie.search("help"), Some("help_intent".to_string()));
        assert_eq!(trie.search("HELP"), Some("help_intent".to_string()));
    }

    #[tokio::test]
    async fn test_system1_exact_match() {
        let system1 = System1::new();
        let (response, _) = system1.process("hello", &[]).await.unwrap();
        assert!(response.contains("你好"));
        
        let (response, _) = system1.process("help", &[]).await.unwrap();
        assert!(response.contains("Available commands"));
    }
    
    #[tokio::test]
    async fn test_system1_alias_match() {
        let system1 = System1::new();
        let (response, _) = system1.process("hi", &[]).await.unwrap();
        assert!(response.contains("你好"));
        
        let (response, _) = system1.process("你是谁", &[]).await.unwrap();
        assert!(response.contains("Crablet"));
    }

    #[tokio::test]
    async fn test_system1_fuzzy_match() {
        let system1 = System1::new();
        // "halp" -> "help" (dist 1)
        let (response, _) = system1.process("halp", &[]).await.unwrap();
        assert!(response.contains("Available commands"));
        
        // "stats" -> "status" (dist 1, alias match)
        let (response, _) = system1.process("stats", &[]).await.unwrap();
        assert!(response.contains("System Status"));
    }

    #[tokio::test]
    async fn test_system1_no_match() {
        let system1 = System1::new();
        let result = system1.process("unknown_command_123", &[]).await;
        assert!(result.is_err());
    }
}
