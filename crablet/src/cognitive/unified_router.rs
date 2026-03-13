//! Unified Router - 统一路由架构
//!
//! 合并 router.rs / meta_router.rs / adaptive_router.rs 的功能
//! 提供单一、高效、可维护的路由入口

use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, debug, instrument};
use dashmap::DashMap;

use crate::cognitive::{
    intent_classifier::{Intent, IntentClassifier, ClassificationResult},
    system1::System1,
    system2::System2,
    system3::System3,
    system4::System4,
};
use crate::skills::{
    hybrid_matcher::{HybridMatcher, HybridMatch, ConversationContext, ConfidenceTier},
    registry::SkillRegistry,
};
use crate::memory::manager::MemoryManager;
use crate::types::TraceStep;

/// 统一路由决策
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedRoutingDecision {
    pub target: RoutingTarget,
    pub confidence: f32,
    pub reasoning: String,
    pub estimated_latency_ms: u32,
    pub fallback: Option<RoutingTarget>,
    pub requires_confirmation: bool,
    pub suggested_skills: Vec<String>,
}

/// 路由目标
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RoutingTarget {
    /// System1: 快速响应
    System1,
    /// System2: 平衡推理 (云端)
    System2,
    /// System2 Local: 本地推理
    System2Local,
    /// System3: 深度研究
    System3,
    /// 执行特定技能
    Skill(String),
    /// 多技能组合
    MultiSkill(Vec<String>),
    /// 需要澄清
    Clarification,
    /// 直接回答 (缓存)
    Cached(String),
}

/// 路由配置
#[derive(Clone, Debug)]
pub struct UnifiedRouterConfig {
    /// System1 置信度阈值
    pub system1_threshold: f32,
    /// System2 置信度阈值
    pub system2_threshold: f32,
    /// System3 置信度阈值
    pub system3_threshold: f32,
    /// 技能自动执行阈值
    pub skill_auto_execute_threshold: f32,
    /// 澄清阈值
    pub clarification_threshold: f32,
    /// 启用自适应路由
    pub enable_adaptive: bool,
    /// 探索率 (bandit)
    pub exploration_rate: f32,
    /// 最大历史记录
    pub max_history_size: usize,
}

impl Default for UnifiedRouterConfig {
    fn default() -> Self {
        Self {
            system1_threshold: 0.85,
            system2_threshold: 0.70,
            system3_threshold: 0.60,
            skill_auto_execute_threshold: 0.80,
            clarification_threshold: 0.40,
            enable_adaptive: true,
            exploration_rate: 0.15,
            max_history_size: 1000,
        }
    }
}

/// 性能指标
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub average_latency_ms: f32,
    pub user_satisfaction: f32,
    pub error_rate: f32,
}

/// 路由历史记录
#[derive(Debug, Clone)]
struct RoutingHistoryEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    query: String,
    decision: UnifiedRoutingDecision,
    actual_latency_ms: u64,
    success: bool,
    user_rating: Option<u8>,
}

/// 上下文特征 (用于 bandit)
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct ContextFeatures {
    intent: Intent,
    complexity_bucket: u8,
    has_code: bool,
    has_question: bool,
}

/// Bandit 臂统计
#[derive(Debug, Clone)]
struct BanditArm {
    count: u64,
    reward_sum: f32,
}

impl BanditArm {
    fn mean(&self) -> f32 {
        if self.count == 0 {
            0.5
        } else {
            self.reward_sum / self.count as f32
        }
    }
}

/// 上下文 bandit
#[derive(Debug)]
struct ContextualBandit {
    /// 每个上下文下的臂统计
    arms: DashMap<ContextFeatures, HashMap<RoutingTarget, BanditArm>>,
    /// 全局统计
    global_arms: RwLock<HashMap<RoutingTarget, BanditArm>>,
    /// 探索率
    exploration: f32,
}

impl ContextualBandit {
    fn new(exploration: f32) -> Self {
        Self {
            arms: DashMap::new(),
            global_arms: RwLock::new(HashMap::new()),
            exploration: exploration.clamp(0.05, 0.5),
        }
    }

    /// 选择最佳目标
    async fn select(&self, features: &ContextFeatures, candidates: &[RoutingTarget]) -> RoutingTarget {
        if candidates.len() == 1 {
            return candidates[0].clone();
        }

        let context_arms = self.arms.get(features).map(|e| e.clone());
        let global_arms = self.global_arms.read().await.clone();

        let mut best_target = candidates[0].clone();
        let mut best_score = f32::MIN;
        let total_n = context_arms.as_ref()
            .map(|a| a.values().map(|arm| arm.count).sum::<u64>())
            .unwrap_or(0) as f32
            + global_arms.values().map(|arm| arm.count).sum::<u64>() as f32
            + 1.0;

        for target in candidates {
            let local_mean = context_arms
                .as_ref()
                .and_then(|a| a.get(target))
                .map(|arm| arm.mean())
                .unwrap_or(0.5);
            
            let global_mean = global_arms
                .get(target)
                .map(|arm| arm.mean())
                .unwrap_or(0.5);

            // 混合本地和全局估计
            let local_count = context_arms
                .as_ref()
                .and_then(|a| a.get(target))
                .map(|arm| arm.count)
                .unwrap_or(0);
            
            let blended = if local_count == 0 {
                0.3 * local_mean + 0.7 * global_mean
            } else {
                0.7 * local_mean + 0.3 * global_mean
            };

            // UCB 探索奖励
            let effective_n = local_count as f32 + 1.0;
            let bonus = self.exploration * (total_n.ln() / effective_n).sqrt();
            let score = blended + bonus;

            if score > best_score {
                best_score = score;
                best_target = target.clone();
            }
        }

        best_target
    }

    /// 更新奖励
    async fn update(&self, features: &ContextFeatures, target: &RoutingTarget, reward: f32) {
        // 更新上下文特定统计
        self.arms
            .entry(features.clone())
            .or_insert_with(HashMap::new)
            .entry(target.clone())
            .and_modify(|arm| {
                arm.count += 1;
                arm.reward_sum += reward;
            })
            .or_insert(BanditArm {
                count: 1,
                reward_sum: reward,
            });

        // 更新全局统计
        let mut global = self.global_arms.write().await;
        global
            .entry(target.clone())
            .and_modify(|arm| {
                arm.count += 1;
                arm.reward_sum += reward;
            })
            .or_insert(BanditArm {
                count: 1,
                reward_sum: reward,
            });
    }
}

/// 统一路由器
pub struct UnifiedRouter {
    /// 意图分类器
    intent_classifier: IntentClassifier,
    /// 混合匹配器
    hybrid_matcher: Arc<RwLock<HybridMatcher>>,
    /// Bandit 学习器
    bandit: Arc<ContextualBandit>,
    /// 配置
    config: Arc<RwLock<UnifiedRouterConfig>>,
    /// 性能指标
    metrics: Arc<RwLock<HashMap<RoutingTarget, PerformanceMetrics>>>,
    /// 历史记录
    history: Arc<RwLock<Vec<RoutingHistoryEntry>>>,
    /// 系统实例
    sys1: Arc<System1>,
    sys2: Arc<RwLock<System2>>,
    sys2_local: Arc<RwLock<System2>>,
    sys3: Arc<RwLock<System3>>,
    /// System4 - 自我进化层
    sys4: Option<Arc<System4>>,
    /// 技能注册表
    skill_registry: Arc<RwLock<SkillRegistry>>,
    /// 记忆管理器
    memory_mgr: Arc<MemoryManager>,
}

impl UnifiedRouter {
    /// 创建新的统一路由器
    pub async fn new(
        config: UnifiedRouterConfig,
        sys1: System1,
        sys2: System2,
        sys2_local: System2,
        sys3: System3,
        _skill_registry: SkillRegistry,
        memory_mgr: MemoryManager,
    ) -> Result<Self> {
        let hybrid_matcher = HybridMatcher::new();

        Ok(Self {
            intent_classifier: IntentClassifier::new(),
            hybrid_matcher: Arc::new(RwLock::new(hybrid_matcher)),
            bandit: Arc::new(ContextualBandit::new(config.exploration_rate)),
            config: Arc::new(RwLock::new(config)),
            metrics: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(Vec::new())),
            sys1: Arc::new(sys1),
            sys2: Arc::new(RwLock::new(sys2)),
            sys2_local: Arc::new(RwLock::new(sys2_local)),
            sys3: Arc::new(RwLock::new(sys3)),
            sys4: None, // System4 需要单独初始化
            skill_registry: Arc::new(RwLock::new(_skill_registry)),
            memory_mgr: Arc::new(memory_mgr),
        })
    }

    /// 路由请求
    #[instrument(skip(self, query, conversation_ctx))]
    pub async fn route(
        &self,
        query: &str,
        conversation_ctx: Option<&ConversationContext>,
    ) -> Result<UnifiedRoutingDecision> {
        let start_time = std::time::Instant::now();
        
        // 1. 意图分类
        let intent_result = self.intent_classifier.classify(query);
        debug!("Intent classified: {:?} with confidence {:.2}",
            intent_result.intent, intent_result.confidence);

        // 2. 技能匹配
        let ctx = conversation_ctx.cloned().unwrap_or_default();
        let skill_matches = self.hybrid_matcher.read().await.find_matches(query, &ctx, 10).await?;

        // 3. 确定候选目标
        let candidates = self.determine_candidates(&intent_result, &skill_matches).await;
        
        // 4. 使用 bandit 选择最佳目标
        let features = self.extract_features(&intent_result, query);
        let selected_target = if self.config.read().await.enable_adaptive {
            self.bandit.select(&features, &candidates).await
        } else {
            candidates[0].clone()
        };

        // 5. 构建决策
        let decision = self.build_decision(
            selected_target,
            &intent_result,
            &skill_matches,
            &candidates,
        ).await;

        // 6. 记录历史
        let latency = start_time.elapsed().as_millis() as u64;
        self.record_history(query, decision.clone(), latency).await;

        info!("Routing decision: {:?} (confidence: {:.2}, latency: {}ms)",
            decision.target, decision.confidence, latency);

        Ok(decision)
    }

    /// 确定候选目标
    async fn determine_candidates(
        &self,
        intent_result: &ClassificationResult,
        skill_matches: &[HybridMatch],
    ) -> Vec<RoutingTarget> {
        let config = self.config.read().await;
        let mut candidates = Vec::new();

        // 检查技能匹配
        let high_confidence_skills: Vec<_> = skill_matches
            .iter()
            .filter(|m| m.confidence_tier == ConfidenceTier::High)
            .take(3)
            .collect();

        if !high_confidence_skills.is_empty() {
            if high_confidence_skills.len() == 1 {
                candidates.push(RoutingTarget::Skill(
                    high_confidence_skills[0].skill_name.clone()
                ));
            } else {
                candidates.push(RoutingTarget::MultiSkill(
                    high_confidence_skills.iter().map(|s| s.skill_name.clone()).collect()
                ));
            }
        }

        // 根据意图选择系统
        match intent_result.intent {
            Intent::Greeting | Intent::Help | Intent::Status => {
                if intent_result.confidence >= config.system1_threshold {
                    candidates.push(RoutingTarget::System1);
                }
                candidates.push(RoutingTarget::System2);
            }
            Intent::Coding | Intent::Analysis | Intent::Math => {
                if intent_result.confidence >= config.system2_threshold {
                    candidates.push(RoutingTarget::System2);
                    candidates.push(RoutingTarget::System2Local);
                } else {
                    candidates.push(RoutingTarget::System2Local);
                }
            }
            Intent::DeepResearch | Intent::Creative | Intent::MultiStep => {
                if intent_result.confidence >= config.system3_threshold {
                    candidates.push(RoutingTarget::System3);
                }
                candidates.push(RoutingTarget::System2);
            }
            Intent::General | Intent::Unknown => {
                candidates.push(RoutingTarget::System2);
            }
            Intent::SkillExecution(_) => {
                candidates.push(RoutingTarget::System2);
            }
        }

        // 如果置信度太低，添加澄清选项
        if intent_result.confidence < config.clarification_threshold {
            candidates.push(RoutingTarget::Clarification);
        }

        candidates
    }

    /// 提取特征
    fn extract_features(&self, intent_result: &ClassificationResult, query: &str) -> ContextFeatures {
        let query_lower = query.to_lowercase();
        ContextFeatures {
            intent: intent_result.intent.clone(),
            complexity_bucket: self.estimate_complexity(query),
            has_code: query_lower.contains("```") || query_lower.contains("fn ") || query_lower.contains("class "),
            has_question: query.contains('?'),
        }
    }

    /// 估计复杂度
    fn estimate_complexity(&self, query: &str) -> u8 {
        let word_count = query.split_whitespace().count();
        match word_count {
            0..=5 => 1,
            6..=15 => 2,
            16..=30 => 3,
            31..=50 => 4,
            _ => 5,
        }
    }

    /// 构建决策
    async fn build_decision(
        &self,
        target: RoutingTarget,
        intent_result: &ClassificationResult,
        skill_matches: &[HybridMatch],
        candidates: &[RoutingTarget],
    ) -> UnifiedRoutingDecision {
        let config = self.config.read().await;
        
        let (estimated_latency, reasoning): (u32, String) = match &target {
            RoutingTarget::System1 => (200, "Simple query, using fast System1".to_string()),
            RoutingTarget::System2 => (1500, "Balanced reasoning with System2".to_string()),
            RoutingTarget::System2Local => (2000, "Local processing with System2".to_string()),
            RoutingTarget::System3 => (5000, "Deep research with System3".to_string()),
            RoutingTarget::Skill(name) => (800, format!("Executing skill: {}", name)),
            RoutingTarget::MultiSkill(_) => (1200, "Executing multiple skills".to_string()),
            RoutingTarget::Clarification => (100, "Need clarification from user".to_string()),
            RoutingTarget::Cached(_) => (50, "Returning cached response".to_string()),
        };

        let fallback = if candidates.len() > 1 {
            Some(candidates[1].clone())
        } else {
            None
        };

        let suggested_skills = skill_matches
            .iter()
            .take(3)
            .map(|m| m.skill_name.clone())
            .collect();

        let requires_confirmation = intent_result.confidence < config.skill_auto_execute_threshold
            && matches!(target, RoutingTarget::Skill(_) | RoutingTarget::MultiSkill(_));

        UnifiedRoutingDecision {
            target,
            confidence: intent_result.confidence,
            reasoning,
            estimated_latency_ms: estimated_latency,
            fallback,
            requires_confirmation,
            suggested_skills,
        }
    }

    /// 记录历史
    async fn record_history(&self, query: &str, decision: UnifiedRoutingDecision, latency: u64) {
        let entry = RoutingHistoryEntry {
            timestamp: chrono::Utc::now(),
            query: query.to_string(),
            decision,
            actual_latency_ms: latency,
            success: true,
            user_rating: None,
        };

        let mut history = self.history.write().await;
        history.push(entry);
        
        // 限制历史大小
        let max_size = self.config.read().await.max_history_size;
        let history_len = history.len();
        if history_len > max_size {
            // 移除最旧的条目
            history.drain(0..history_len - max_size);
        }
    }

    /// 处理请求
    pub async fn process(&self, query: &str, session_id: &str) -> Result<(String, Vec<TraceStep>)> {
        let conversation_ctx = self.build_conversation_context(session_id).await?;
        let decision = self.route(query, Some(&conversation_ctx)).await?;

        // 根据决策执行
        match &decision.target {
            RoutingTarget::System1 => {
                self.execute_system1(query).await
            }
            RoutingTarget::System2 => {
                self.execute_system2(query, false).await
            }
            RoutingTarget::System2Local => {
                self.execute_system2(query, true).await
            }
            RoutingTarget::System3 => {
                self.execute_system3(query).await
            }
            RoutingTarget::Skill(skill_name) => {
                self.execute_skill(skill_name, query).await
            }
            RoutingTarget::MultiSkill(skills) => {
                self.execute_multi_skill(skills, query).await
            }
            RoutingTarget::Clarification => {
                Ok(("我需要更多信息来回答您的问题。您能详细说明一下吗？".to_string(), vec![]))
            }
            RoutingTarget::Cached(response) => {
                Ok((response.clone(), vec![]))
            }
        }
    }

    /// 构建对话上下文
    async fn build_conversation_context(&self, _session_id: &str) -> Result<ConversationContext> {
        // 简化实现 - 实际应该从记忆管理器获取历史
        Ok(ConversationContext {
            current_topic: None,
            recent_skills: Vec::new(),
            conversation_history: Vec::new(),
            user_preferences: HashMap::new(),
        })
    }

    /// 执行 System1
    async fn execute_system1(&self, query: &str) -> Result<(String, Vec<TraceStep>)> {
        // System1 快速响应
        let response = format!("System1 快速响应: {}", query);
        Ok((response, vec![]))
    }

    /// 执行 System2
    async fn execute_system2(&self, _query: &str, use_local: bool) -> Result<(String, Vec<TraceStep>)> {
        if use_local {
            let _sys2 = self.sys2_local.read().await;
            // 调用 System2 处理
            Ok(("System2 Local 响应".to_string(), vec![]))
        } else {
            let _sys2 = self.sys2.read().await;
            Ok(("System2 Cloud 响应".to_string(), vec![]))
        }
    }

    /// 执行 System3
    async fn execute_system3(&self, _query: &str) -> Result<(String, Vec<TraceStep>)> {
        let _sys3 = self.sys3.read().await;
        Ok(("System3 深度研究响应".to_string(), vec![]))
    }

    /// 执行技能
    async fn execute_skill(&self, skill_name: &str, query: &str) -> Result<(String, Vec<TraceStep>)> {
        let _registry = self.skill_registry.read().await;
        // 执行技能逻辑
        Ok((format!("执行技能 {}: {}", skill_name, query), vec![]))
    }

    /// 执行多技能
    async fn execute_multi_skill(&self, skills: &[String], query: &str) -> Result<(String, Vec<TraceStep>)> {
        let mut results = Vec::new();
        for skill in skills {
            let (result, _) = self.execute_skill(skill, query).await?;
            results.push(result);
        }
        Ok((results.join("\n"), vec![]))
    }

    /// 反馈更新
    pub async fn provide_feedback(
        &self,
        query: &str,
        target: &RoutingTarget,
        success: bool,
        latency_ms: u64,
        user_rating: Option<u8>,
    ) -> Result<()> {
        // 计算奖励
        let reward = if success {
            let base = 1.0;
            let latency_penalty = (latency_ms as f32 / 5000.0).min(0.5);
            let rating_bonus = user_rating.map(|r| (r as f32 - 3.0) / 10.0).unwrap_or(0.0);
            base - latency_penalty + rating_bonus
        } else {
            0.0
        };

        // 提取特征并更新 bandit
        let intent_result = self.intent_classifier.classify(query);
        let features = self.extract_features(&intent_result, query);
        self.bandit.update(&features, target, reward).await;

        // 更新指标
        let mut metrics = self.metrics.write().await;
        let entry = metrics.entry(target.clone()).or_default();
        entry.total_requests += 1;
        if success {
            entry.successful_requests += 1;
        }
        
        // 更新平均延迟
        let old_avg = entry.average_latency_ms;
        let count = entry.total_requests as f32;
        entry.average_latency_ms = (old_avg * (count - 1.0) + latency_ms as f32) / count;

        if let Some(rating) = user_rating {
            entry.user_satisfaction = (entry.user_satisfaction * (count - 1.0) + rating as f32) / count;
        }

        info!("Feedback recorded for {:?}: reward={:.2}", target, reward);
        Ok(())
    }

    /// 获取性能报告
    pub async fn get_performance_report(&self) -> HashMap<RoutingTarget, PerformanceMetrics> {
        self.metrics.read().await.clone()
    }

    /// 设置 System4
    pub fn with_system4(mut self, sys4: Arc<System4>) -> Self {
        self.sys4 = Some(sys4);
        self
    }

    /// 记录执行到 System4
    async fn record_to_system4(&self, target: &RoutingTarget, query: &str, result: &(String, Vec<TraceStep>), latency_ms: u64, success: bool) {
        if let Some(ref sys4) = self.sys4 {
            match target {
                RoutingTarget::System1 => {
                    sys4.record_system1_execution(query, latency_ms, success).await;
                }
                RoutingTarget::System2 | RoutingTarget::System2Local => {
                    sys4.record_system2_execution(
                        query,
                        Some(&result.0),
                        latency_ms,
                        success,
                        result.0.len(),
                        0,
                    ).await;
                }
                RoutingTarget::System3 => {
                    sys4.record_system3_execution(
                        query,
                        Some(&result.0),
                        latency_ms,
                        success,
                        1,
                        None,
                    ).await;
                }
                _ => {}
            }
        }
    }

    /// 获取 System4 状态（如果已启用）
    pub async fn get_system4_status(&self) -> Option<String> {
        if let Some(ref sys4) = self.sys4 {
            let report = sys4.get_status_report().await;
            Some(format!(
                "System4 Enabled | Executions: {} | Pending Proposals: {} | Applied Changes: {}",
                report.total_executions_recorded,
                report.pending_proposals,
                report.applied_changes
            ))
        } else {
            None
        }
    }

    /// 更新配置
    pub async fn update_config(&self, config: UnifiedRouterConfig) {
        *self.config.write().await = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_target_serialization() {
        let target = RoutingTarget::Skill("test".to_string());
        let json = serde_json::to_string(&target).unwrap();
        assert!(json.contains("Skill"));
    }

    #[test]
    fn test_bandit_arm() {
        let arm = BanditArm {
            count: 10,
            reward_sum: 8.5,
        };
        assert_eq!(arm.mean(), 0.85);
    }
}
