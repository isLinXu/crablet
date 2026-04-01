//! Shared utilities for web handlers
//!
//! Common helper functions used across multiple handler modules.

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::collections::HashSet;
use crate::types::TraceStep;

/// Resolve the path to the .env file
pub fn resolve_env_file_path() -> PathBuf {
    if let Ok(v) = std::env::var("CRABLET_ENV_FILE") {
        let p = PathBuf::from(v);
        if p.exists() {
            return p;
        }
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidates = [
        cwd.join(".env"),
        cwd.join("crablet").join(".env"),
        cwd.join("../crablet").join(".env"),
    ];
    candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| cwd.join(".env"))
}

/// Read environment variable from .env file
pub fn env_value_from_file(key: &str) -> Option<String> {
    let content = fs::read_to_string(resolve_env_file_path()).ok()?;
    for line in content.lines() {
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                let value = v.trim().to_string();
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
    }
    None
}

/// Load markdown file content from common locations
pub fn load_markdown_file(filename: &str) -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let candidates = [
        cwd.join(filename),
        cwd.join("crablet").join(filename),
        cwd.join("../").join(filename),
    ];
    let path = candidates.into_iter().find(|p| p.exists())?;
    let content = fs::read_to_string(path).ok()?;
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Build system prompt from markdown files (identity, soul, user, etc.)
pub fn system_prompt_markdown() -> String {
    static CONTENT: OnceLock<String> = OnceLock::new();
    CONTENT
        .get_or_init(|| {
            let mut prompt = String::new();
            if let Some(identity) = load_markdown_file("IDENTITY.md") {
                prompt.push_str(&format!("【身份设定】\n{}\n\n", identity));
            }
            if let Some(soul) = load_markdown_file("SOUL.md") {
                prompt.push_str(&format!("【核心准则】\n{}\n\n", soul));
            }
            if let Some(user) = load_markdown_file("USER.md") {
                prompt.push_str(&format!("【用户信息】\n{}\n\n", user));
            }
            if let Some(agents) = load_markdown_file("AGENTS.md") {
                prompt.push_str(&format!("【工作空间指南】\n{}\n\n", agents));
            }
            if let Some(tools) = load_markdown_file("TOOLS.md") {
                prompt.push_str(&format!("【工具配置】\n{}\n\n", tools));
            }
            if let Some(heartbeat) = load_markdown_file("HEARTBEAT.md") {
                prompt.push_str(&format!("【定时任务】\n{}\n\n", heartbeat));
            }
            prompt
        })
        .clone()
}

/// Inject identity/persona into user message
pub fn with_identity_persona_input(message: &str) -> String {
    let system_prompt = system_prompt_markdown();
    if system_prompt.is_empty() {
        message.to_string()
    } else {
        format!("{}\n【用户消息】\n{}", system_prompt, message)
    }
}

/// Infer cognitive layer from response and traces
pub fn infer_cognitive_layer(response: &str, traces: &[TraceStep]) -> &'static str {
    let mut text = response.to_lowercase();
    for t in traces {
        text.push(' ');
        text.push_str(&t.thought.to_lowercase());
        if let Some(a) = &t.action {
            text.push(' ');
            text.push_str(&a.to_lowercase());
        }
        if let Some(o) = &t.observation {
            text.push(' ');
            text.push_str(&o.to_lowercase());
        }
    }
    if text.contains("system 1") || text.contains("system1") || text.contains("trie hit") || text.contains("fastrespond") {
        return "system1";
    }
    if text.contains("system 3") || text.contains("system3") || text.contains("plan") || text.contains("planner") || text.contains("verify") {
        return "system3";
    }
    if text.contains("system 2") || text.contains("system2") || text.contains("reason") || text.contains("deliberate") {
        return "system2";
    }
    if !traces.is_empty() {
        return "system2";
    }
    "unknown"
}

/// Infer cognitive layer from input message
pub fn infer_cognitive_layer_from_input(input: &str) -> &'static str {
    let input_lower = input.to_lowercase();

    // Greeting patterns -> System 1
    let greeting_patterns = [
        "hi", "hello", "hey", "greetings", "good morning", "good afternoon", "good evening",
        "你好", "嗨", "您好", "早上好", "下午好", "晚上好", "在吗", "在么",
    ];
    for pattern in &greeting_patterns {
        if input_lower.trim() == *pattern || input_lower.starts_with(pattern) {
            return "system1";
        }
    }

    // Persona patterns -> System 1
    let persona_patterns = [
        "who are you", "what are you", "your name", "introduce yourself", "tell me about yourself",
        "你是谁", "你是什么", "你叫什么", "介绍一下", "你是干嘛的", "你是做什么的",
        "你的身份", "你的角色", "你是ai吗", "你是人工智能吗", "你是机器人吗",
    ];
    for pattern in &persona_patterns {
        if input_lower.contains(pattern) {
            return "system1";
        }
    }

    // Chat patterns -> System 1
    let chat_patterns = [
        "how are you", "what's up", "how's it going", "nice to meet you", "thank you", "thanks",
        "你好吗", "最近怎么样", "很高兴认识你", "谢谢", "多谢", "哈哈", "呵呵", "嘿嘿",
        "好的", "ok", "okay", "嗯", "哦", "啊", "呢", "吧", "吗",
    ];
    for pattern in &chat_patterns {
        if input_lower.trim() == *pattern || input_lower.starts_with(pattern) {
            return "system1";
        }
    }

    // Simple personal questions -> System 1
    let personal_patterns = [
        "how old are you", "where are you from", "what do you like", "your favorite",
        "你多大了", "你几岁了", "你喜欢什么", "你的爱好", "你喜欢", "你的",
    ];
    for pattern in &personal_patterns {
        if input_lower.contains(pattern) {
            return "system1";
        }
    }

    // Help patterns -> System 1
    let help_patterns = [
        "help", "assist", "support", "how to", "what can you do",
        "帮助", "怎么用", "如何使用", "你能做什么", "有什么功能",
    ];
    for pattern in &help_patterns {
        if input_lower.contains(pattern) {
            return "system1";
        }
    }

    // Status patterns -> System 1
    let status_patterns = [
        "status", "system info", "health", "check", "diagnostics",
        "状态", "系统信息", "健康检查", "诊断",
    ];
    for pattern in &status_patterns {
        if input_lower.contains(pattern) {
            return "system1";
        }
    }

    // Deep research patterns -> System 3
    let research_patterns = [
        "research", "deep research", "investigate", "explore in depth", "comprehensive analysis",
        "研究", "深度研究", "深入分析", "全面调查", "详细探讨",
    ];
    for pattern in &research_patterns {
        if input_lower.contains(pattern) {
            return "system3";
        }
    }

    // Multi-step task patterns -> System 3
    let multistep_patterns = [
        "first", "then", "next", "after", "finally", "step by step",
        "首先", "然后", "接着", "最后", "一步步", "步骤",
    ];
    let multistep_count = multistep_patterns.iter().filter(|p| input_lower.contains(*p)).count();
    if multistep_count >= 2 {
        return "system3";
    }

    // Code/analysis patterns -> System 2
    let code_patterns = [
        "code", "function", "implement", "program", "debug", "refactor", "algorithm",
        "代码", "编写", "实现", "函数", "调试", "程序", "算法",
        "analyze", "compare", "evaluate", "assess", "review", "examine",
        "分析", "比较", "评估", "评价", "优缺点",
    ];
    for pattern in &code_patterns {
        if input_lower.contains(pattern) {
            return "system2";
        }
    }

    // Default to System 2
    "system2"
}

/// Shared disabled skills store
pub fn disabled_skills_store() -> &'static tokio::sync::RwLock<HashSet<String>> {
    static STORE: OnceLock<tokio::sync::RwLock<HashSet<String>>> = OnceLock::new();
    STORE.get_or_init(|| tokio::sync::RwLock::new(HashSet::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TraceStep;

    // ── infer_cognitive_layer_from_input ──

    #[test]
    fn test_greeting_patterns_system1() {
        assert_eq!(infer_cognitive_layer_from_input("hello"), "system1");
        assert_eq!(infer_cognitive_layer_from_input("你好"), "system1");
        assert_eq!(infer_cognitive_layer_from_input("Hi there"), "system1");
        assert_eq!(infer_cognitive_layer_from_input("早上好"), "system1");
        assert_eq!(infer_cognitive_layer_from_input("在吗"), "system1");
    }

    #[test]
    fn test_persona_patterns_system1() {
        assert_eq!(infer_cognitive_layer_from_input("你是谁"), "system1");
        assert_eq!(infer_cognitive_layer_from_input("what are you"), "system1");
        assert_eq!(infer_cognitive_layer_from_input("你叫什么名字"), "system1");
        assert_eq!(infer_cognitive_layer_from_input("你是人工智能吗"), "system1");
    }

    #[test]
    fn test_chat_patterns_system1() {
        assert_eq!(infer_cognitive_layer_from_input("谢谢"), "system1");
        assert_eq!(infer_cognitive_layer_from_input("ok"), "system1");
        assert_eq!(infer_cognitive_layer_from_input("好的"), "system1");
    }

    #[test]
    fn test_help_and_status_system1() {
        assert_eq!(infer_cognitive_layer_from_input("help"), "system1");
        assert_eq!(infer_cognitive_layer_from_input("状态"), "system1");
        assert_eq!(infer_cognitive_layer_from_input("how to use"), "system1");
    }

    #[test]
    fn test_research_patterns_system3() {
        assert_eq!(infer_cognitive_layer_from_input("研究这个问题"), "system3");
        assert_eq!(infer_cognitive_layer_from_input("深度研究"), "system3");
        assert_eq!(infer_cognitive_layer_from_input("comprehensive analysis"), "system3");
    }

    #[test]
    fn test_multistep_patterns_system3() {
        assert_eq!(infer_cognitive_layer_from_input("首先分析，然后总结，最后给出建议"), "system3");
        // Single step keyword should not trigger system3
        assert_ne!(infer_cognitive_layer_from_input("首先我们开始"), "system3");
    }

    #[test]
    fn test_code_patterns_system2() {
        assert_eq!(infer_cognitive_layer_from_input("写一个函数"), "system2");
        assert_eq!(infer_cognitive_layer_from_input("analyze the data"), "system2");
        assert_eq!(infer_cognitive_layer_from_input("实现算法"), "system2");
        assert_eq!(infer_cognitive_layer_from_input("evaluate this approach"), "system2");
    }

    #[test]
    fn test_default_system2() {
        assert_eq!(infer_cognitive_layer_from_input("some random text"), "system2");
        assert_eq!(infer_cognitive_layer_from_input("tell me about the weather tomorrow"), "system2");
    }

    // ── infer_cognitive_layer (response + traces) ──

    #[test]
    fn test_infer_from_response_system1() {
        let traces = vec![TraceStep {
            step: 0,
            thought: "trie hit".to_string(),
            action: Some("FastRespond".to_string()),
            action_input: None,
            observation: None,
        }];
        assert_eq!(infer_cognitive_layer("response", &traces), "system1");
    }

    #[test]
    fn test_infer_from_response_system3() {
        let traces = vec![TraceStep {
            step: 0,
            thought: "plan and verify".to_string(),
            action: None,
            action_input: None,
            observation: None,
        }];
        assert_eq!(infer_cognitive_layer("response", &traces), "system3");
    }

    #[test]
    fn test_infer_from_response_system2() {
        let traces = vec![TraceStep {
            step: 0,
            thought: "deliberate reasoning".to_string(),
            action: None,
            action_input: None,
            observation: None,
        }];
        assert_eq!(infer_cognitive_layer("response", &traces), "system2");
    }

    #[test]
    fn test_infer_from_response_empty_traces_unknown() {
        assert_eq!(infer_cognitive_layer("random text", &[]), "unknown");
    }

    #[test]
    fn test_infer_from_response_nonempty_traces_default_system2() {
        let traces = vec![TraceStep {
            step: 0,
            thought: "some generic thought".to_string(),
            action: None,
            action_input: None,
            observation: None,
        }];
        assert_eq!(infer_cognitive_layer("response", &traces), "system2");
    }

    // ── disabled_skills_store ──

    #[tokio::test]
    async fn test_disabled_skills_store() {
        let store = disabled_skills_store();
        store.write().await.insert("test_skill".to_string());
        assert!(store.read().await.contains("test_skill"));
        store.write().await.remove("test_skill");
        assert!(!store.read().await.contains("test_skill"));
    }

    // ── GatewayConfig serialization ──

    #[test]
    fn test_gateway_config_serde() {
        let config = crate::gateway::types::GatewayConfig {
            host: "0.0.0.0".to_string(),
            port: 18790,
            auth_mode: "off".to_string(),
        };
        let json = serde_json::to_string(&config).expect("serialize config");
        let back: crate::gateway::types::GatewayConfig =
            serde_json::from_str(&json).expect("deserialize config");
        assert_eq!(back.port, 18790);
        assert_eq!(back.auth_mode, "off");
    }
}