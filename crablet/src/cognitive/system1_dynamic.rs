//! System 1 Dynamic — Thread-safe dynamic registration wrapper
//!
//! This module provides a thread-safe wrapper around `System1Enhanced` that adds:
//! - Runtime rule registration/unregistration (async, lock-free reads)
//! - Per-session context snapshots with topic tracking
//! - Weighted scoring with context-aware boosting
//! - Condition predicates for conditional rule activation
//!
//! The core matching logic is delegated to `System1Enhanced` to avoid code duplication.

use crate::cognitive::system1_enhanced::System1Enhanced;
use crate::cognitive::CognitiveSystem;
use crate::error::{CrabletError, Result};
use crate::types::{Message, TraceStep};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// Re-export core types from the enhanced implementation
pub use crate::cognitive::system1_enhanced::{
    Command as EnhancedCommand, CommandCategory as EnhancedCommandCategory,
    ResponseTemplate, System1Enhanced as System1Core,
};

/// Type alias for the dynamic condition predicate.
pub type ConditionFn = Arc<dyn Fn(&ContextSnapshot) -> bool + Send + Sync>;
/// Type alias for the command handler closure.
pub type HandlerFn = Arc<dyn Fn(&str, &ContextSnapshot) -> String + Send + Sync>;

/// 动态上下文快照，用于上下文感知匹配
#[derive(Clone, Debug, Default)]
pub struct ContextSnapshot {
    pub session_id: String,
    pub user_id: Option<String>,
    pub last_intent: Option<String>,
    pub last_entities: Vec<String>,
    pub turn_count: u32,
    pub recent_topics: Vec<String>,
}

/// 命令匹配结果
#[derive(Clone, Debug)]
pub struct CommandMatch {
    pub rule_id: String,
    pub score: f32,
    pub is_exact: bool,
    pub context_boost: f32,
}

/// 动态命令规则，支持权重、条件、上下文标签
#[derive(Clone)]
pub struct DynamicCommandRule {
    pub id: String,
    pub primary_command: String,
    pub aliases: Vec<String>,
    pub description: String,
    pub weight: f32,
    pub context_tags: Vec<String>,
    pub condition: Option<ConditionFn>,
    pub handler: HandlerFn,
}

/// 动态 System 1 — 支持运行时注册、上下文感知、权重排序
///
/// Wraps `System1Enhanced` for core matching and adds dynamic registration,
/// context snapshots, and weighted scoring on top.
#[derive(Clone)]
pub struct System1Dynamic {
    /// Core matching engine (shared with the static command set)
    core: Arc<RwLock<System1Enhanced>>,
    /// Dynamic rules registered at runtime (supplement the core's built-in commands)
    dynamic_rules: Arc<RwLock<Vec<DynamicCommandRule>>>,
    /// Per-session context history
    context_history: Arc<RwLock<HashMap<String, ContextSnapshot>>>,
}

impl Default for System1Dynamic {
    fn default() -> Self {
        Self::new()
    }
}

impl System1Dynamic {
    pub fn new() -> Self {
        Self {
            core: Arc::new(RwLock::new(System1Enhanced::new())),
            dynamic_rules: Arc::new(RwLock::new(Vec::new())),
            context_history: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // ------------------------------------------------------------------
    // 动态注册接口
    // ------------------------------------------------------------------

    /// 注册一条命令规则（运行时安全）
    pub async fn register_rule(&self, rule: DynamicCommandRule) {
        let mut rules = self.dynamic_rules.write().await;
        // 去重：如果 id 已存在，先移除旧规则
        rules.retain(|r| r.id != rule.id);
        rules.push(rule);
    }

    /// 批量注册规则
    pub async fn register_rules(&self, rules: Vec<DynamicCommandRule>) {
        for rule in rules {
            self.register_rule(rule).await;
        }
    }

    /// 注销规则
    pub async fn unregister_rule(&self, rule_id: &str) -> bool {
        let mut rules = self.dynamic_rules.write().await;
        let len_before = rules.len();
        rules.retain(|r| r.id != rule_id);
        rules.len() < len_before
    }

    /// 更新规则权重
    pub async fn set_rule_weight(&self, rule_id: &str, weight: f32) -> bool {
        let mut rules = self.dynamic_rules.write().await;
        if let Some(rule) = rules.iter_mut().find(|r| r.id == rule_id) {
            rule.weight = weight.clamp(0.0, 10.0);
            return true;
        }
        false
    }

    /// 获取当前已注册动态规则数量
    pub async fn rule_count(&self) -> usize {
        self.dynamic_rules.read().await.len()
    }

    // ------------------------------------------------------------------
    // 上下文管理
    // ------------------------------------------------------------------

    /// 更新或创建会话上下文
    pub async fn update_context<F>(&self, session_id: &str, updater: F)
    where
        F: FnOnce(&mut ContextSnapshot),
    {
        let mut history = self.context_history.write().await;
        let ctx = history
            .entry(session_id.to_string())
            .or_insert_with(|| ContextSnapshot {
                session_id: session_id.to_string(),
                ..Default::default()
            });
        updater(ctx);
        ctx.turn_count += 1;
    }

    /// 获取会话上下文（或默认）
    pub async fn get_context(&self, session_id: &str) -> ContextSnapshot {
        let history = self.context_history.read().await;
        history
            .get(session_id)
            .cloned()
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------
    // 核心匹配逻辑
    // ------------------------------------------------------------------

    /// 上下文感知匹配：先尝试动态规则，再回退到核心引擎
    async fn find_dynamic_match(
        &self,
        input: &str,
        context: &ContextSnapshot,
    ) -> Option<(DynamicCommandRule, CommandMatch)> {
        let input_trim = input.trim();
        let input_lower = input_trim.to_lowercase();
        let first_word = input_lower.split_whitespace().next().unwrap_or("");

        let rules = self.dynamic_rules.read().await;
        let mut candidates: Vec<(DynamicCommandRule, CommandMatch)> = Vec::new();

        for rule in rules.iter() {
            // 条件检查
            if let Some(ref cond) = rule.condition {
                if !cond(context) {
                    continue;
                }
            }

            // 精确匹配
            let is_exact = input_lower == rule.primary_command.to_lowercase()
                || rule.aliases.iter().any(|a| input_lower == a.to_lowercase());

            if is_exact {
                let context_boost = self.compute_context_boost(rule, context);
                candidates.push((
                    rule.clone(),
                    CommandMatch {
                        rule_id: rule.id.clone(),
                        score: 1.0 + context_boost,
                        is_exact: true,
                        context_boost,
                    },
                ));
                continue;
            }

            // 模糊匹配 (Levenshtein)
            let mut best_dist = usize::MAX;
            best_dist = best_dist.min(strsim::levenshtein(first_word, &rule.primary_command.to_lowercase()));
            for alias in &rule.aliases {
                best_dist = best_dist.min(strsim::levenshtein(first_word, &alias.to_lowercase()));
            }

            let threshold = if first_word.len() < 4 { 0 } else if first_word.len() < 7 { 1 } else { 2 };

            if best_dist <= threshold {
                let context_boost = self.compute_context_boost(rule, context);
                let base_score = 1.0 - (best_dist as f32 / (first_word.len() as f32 + 1.0));
                candidates.push((
                    rule.clone(),
                    CommandMatch {
                        rule_id: rule.id.clone(),
                        score: (base_score + context_boost).clamp(0.0, 1.0),
                        is_exact: false,
                        context_boost,
                    },
                ));
            }
        }

        // 按加权分数排序
        candidates.sort_by(|a, b| {
            let a_weighted = a.1.score * a.0.weight;
            let b_weighted = b.1.score * b.0.weight;
            b_weighted
                .partial_cmp(&a_weighted)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        candidates.into_iter().next()
    }

    fn compute_context_boost(&self, rule: &DynamicCommandRule, context: &ContextSnapshot) -> f32 {
        let mut boost = 0.0f32;
        for tag in &rule.context_tags {
            if context.recent_topics.iter().any(|t| t.to_lowercase().contains(&tag.to_lowercase())) {
                boost += 0.15;
            }
            if let Some(ref last) = context.last_intent {
                if last.to_lowercase().contains(&tag.to_lowercase()) {
                    boost += 0.1;
                }
            }
        }
        boost.min(0.5)
    }

    /// 创建内置规则列表（外部调用者可批量注册）
    pub fn builtin_rules() -> Vec<DynamicCommandRule> {
        vec![
            DynamicCommandRule {
                id: "greeting".to_string(),
                primary_command: "hello".to_string(),
                aliases: vec![
                    "hi".to_string(), "hey".to_string(),
                    "你好".to_string(), "您好".to_string(),
                    "你好!".to_string(), "你好！".to_string(),
                ],
                description: "Say hello".to_string(),
                weight: 1.0,
                context_tags: vec!["onboarding".to_string()],
                condition: None,
                handler: Arc::new(|_: &str, _: &ContextSnapshot| -> String {
                    "你好！我是 Crablet，你的智能助手。".to_string()
                }),
            },
            DynamicCommandRule {
                id: "identity".to_string(),
                primary_command: "identity".to_string(),
                aliases: vec![
                    "who are you".to_string(), "你是谁".to_string(),
                    "what is your name".to_string(), "你叫什么".to_string(),
                ],
                description: "Identity".to_string(),
                weight: 1.0,
                context_tags: vec!["onboarding".to_string()],
                condition: None,
                handler: Arc::new(|_: &str, _: &ContextSnapshot| -> String {
                    "我是 Crablet，一个基于大模型的智能助手。我能够帮助你完成各种任务，比如搜索信息、执行命令、创建技能等。有什么我可以帮你的吗？".to_string()
                }),
            },
            DynamicCommandRule {
                id: "help".to_string(),
                primary_command: "help".to_string(),
                aliases: vec!["/help".to_string(), "帮助".to_string()],
                description: "Show help".to_string(),
                weight: 1.0,
                context_tags: vec![],
                condition: None,
                handler: Arc::new(|_: &str, _: &ContextSnapshot| -> String {
                    "Available commands:\n- /help: Show this message\n- /status: Check system status\n- /exit: Quit session".to_string()
                }),
            },
            DynamicCommandRule {
                id: "status".to_string(),
                primary_command: "status".to_string(),
                aliases: vec!["/status".to_string(), "stats".to_string(), "状态".to_string()],
                description: "Check status".to_string(),
                weight: 1.0,
                context_tags: vec![],
                condition: None,
                handler: Arc::new(|_: &str, _: &ContextSnapshot| -> String {
                    "System Status: ONLINE. All subsystems operational.".to_string()
                }),
            },
            DynamicCommandRule {
                id: "onboarding_tip".to_string(),
                primary_command: "start".to_string(),
                aliases: vec!["begin".to_string(), "开始".to_string()],
                description: "Onboarding tips for new users".to_string(),
                weight: 1.2,
                context_tags: vec!["onboarding".to_string()],
                condition: Some(Arc::new(|ctx: &ContextSnapshot| ctx.turn_count <= 3)),
                handler: Arc::new(|_: &str, ctx: &ContextSnapshot| -> String {
                    format!(
                        "欢迎新用户！这是你的第 {} 轮对话。尝试输入 '/help' 查看可用命令。",
                        ctx.turn_count
                    )
                }),
            },
            DynamicCommandRule {
                id: "code_review".to_string(),
                primary_command: "review".to_string(),
                aliases: vec!["cr".to_string(), "review code".to_string()],
                description: "Code review shortcut".to_string(),
                weight: 1.0,
                context_tags: vec!["coding".to_string(), "code".to_string()],
                condition: None,
                handler: Arc::new(|input: &str, _: &ContextSnapshot| -> String {
                    format!("进入代码审查模式。请提供代码：{}", input)
                }),
            },
        ]
    }
}

#[async_trait]
impl CognitiveSystem for System1Dynamic {
    fn name(&self) -> &str {
        "System 1 (Dynamic Intuitive)"
    }

    async fn process(&self, input: &str, context: &[Message]) -> Result<(String, Vec<TraceStep>)> {
        let input_trim = input.trim();
        let session_id = "default";

        // 获取并更新上下文
        let mut ctx = self.get_context(session_id).await;
        if let Some(last) = context.last() {
            if let Some(ref text) = last.text() {
                let lower = text.to_lowercase();
                if lower.contains("code") || lower.contains("rust") || lower.contains("python") {
                    ctx.recent_topics.push("coding".to_string());
                } else if lower.contains("help") || lower.contains("how to") {
                    ctx.recent_topics.push("help".to_string());
                }
                if ctx.recent_topics.len() > 5 {
                    ctx.recent_topics.remove(0);
                }
            }
        }

        // 1. 先尝试动态规则匹配
        if let Some((rule, matched)) = self.find_dynamic_match(input_trim, &ctx).await {
            let response = (rule.handler)(input_trim, &ctx);

            // 更新上下文
            let rule_id = rule.id.clone();
            self.update_context(session_id, |c| {
                c.last_intent = Some(rule_id);
            }).await;

            return Ok((
                response,
                vec![TraceStep {
                    step: 0,
                    thought: format!(
                        "System 1 {} Hit: {} (score: {:.2}, weight: {:.1}, context_boost: {:.2})",
                        if matched.is_exact { "Exact" } else { "Fuzzy" },
                        rule.id, matched.score, rule.weight, matched.context_boost
                    ),
                    action: Some("FastRespond".to_string()),
                    action_input: Some(input_trim.to_string()),
                    observation: Some(format!("Executed rule '{}' with context-aware scoring", rule.id)),
                }],
            ));
        }

        // 2. 回退到核心引擎 (System1Enhanced 的 20+ 内置命令)
        let core = self.core.read().await;
        match core.process(input_trim, context).await {
            Ok((response, traces)) => {
                drop(core);
                self.update_context(session_id, |c| {
                    c.last_intent = Some("core_match".to_string());
                }).await;
                Ok((response, traces))
            }
            Err(_) => {
                drop(core);
                self.update_context(session_id, |c| {
                    c.last_intent = Some("unknown".to_string());
                }).await;
                Err(CrabletError::NotFound("No intuitive match found".to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dynamic_register_and_match() {
        let system = System1Dynamic::new();
        system.register_rules(System1Dynamic::builtin_rules()).await;
        assert_eq!(system.rule_count().await, 6);

        let (response, _) = system.process("hello", &[]).await.unwrap();
        assert!(response.contains("你好"));
    }

    #[tokio::test]
    async fn test_fallback_to_core() {
        let system = System1Dynamic::new();
        // No dynamic rules registered, but core has 20+ commands
        let (response, _) = system.process("thank you", &[]).await.unwrap();
        assert!(!response.is_empty());
    }

    #[tokio::test]
    async fn test_unregister_and_fuzzy_match() {
        let system = System1Dynamic::new();
        system.register_rules(System1Dynamic::builtin_rules()).await;

        let removed = system.unregister_rule("greeting").await;
        assert!(removed);
        assert_eq!(system.rule_count().await, 5);

        // "halp" -> help (fuzzy)
        let (response, _) = system.process("halp", &[]).await.unwrap();
        assert!(response.contains("Available commands"));
    }

    #[tokio::test]
    async fn test_weighted_scoring() {
        let system = System1Dynamic::new();
        system.register_rules(System1Dynamic::builtin_rules()).await;

        system.set_rule_weight("code_review", 2.0).await;
        system.update_context("default", |c| {
            c.recent_topics = vec!["coding".to_string()];
        }).await;

        let (response, traces) = system.process("review", &[]).await.unwrap();
        assert!(response.contains("代码审查"));
        let trace = traces.first().unwrap();
        assert!(trace.thought.contains("context_boost"));
    }

    #[tokio::test]
    async fn test_context_aware_condition() {
        let system = System1Dynamic::new();
        system.register_rules(System1Dynamic::builtin_rules()).await;

        // New user (turn_count=0) triggers onboarding_tip
        system.update_context("default", |c| { c.turn_count = 0; }).await;
        let (response, _) = system.process("start", &[]).await.unwrap();
        assert!(response.contains("欢迎新用户"));
    }

    #[tokio::test]
    async fn test_empty_dynamic_falls_to_core() {
        let system = System1Dynamic::new();
        assert_eq!(system.rule_count().await, 0);

        // Core engine still handles greetings
        let (response, _) = system.process("hello", &[]).await.unwrap();
        assert!(!response.is_empty());
    }

    #[tokio::test]
    async fn test_reregister_after_unregister() {
        let system = System1Dynamic::new();
        system.register_rules(System1Dynamic::builtin_rules()).await;

        system.unregister_rule("greeting").await;
        assert_eq!(system.rule_count().await, 5);

        system.register_rules(vec![System1Dynamic::builtin_rules()
            .into_iter()
            .find(|r| r.id == "greeting")
            .unwrap()])
        .await;
        assert_eq!(system.rule_count().await, 6);

        let (resp, _) = system.process("hello", &[]).await.unwrap();
        assert!(resp.contains("你好"));
    }
}
