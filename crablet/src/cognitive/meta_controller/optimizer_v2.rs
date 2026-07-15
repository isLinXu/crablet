//! Optimizer v2 — 可落地的元认知优化器
//!
//! 核心改进：
//! 1. 连接 ConfigManager，实际修改系统运行参数（timeout、并发度、模型选择等）
//! 2. 策略优化不仅记录评分，还下发到 CapabilityRouter、SmartTaskAllocator 等执行系统
//! 3. 错误预防策略触发实际的规则注入（如临时黑名单、降级策略）
//! 4. 所有改进都有持久化记录和回滚支持

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::cognitive::meta_controller::learner::LearnedKnowledge;
use crate::error::Result;

/// 运行时配置管理接口（需由外部系统实现并注入）
#[async_trait::async_trait]
pub trait ConfigManager: Send + Sync {
    /// 获取当前配置值
    async fn get(&self, key: &str) -> Option<String>;
    /// 设置配置值，返回旧值
    async fn set(&self, key: &str, value: String) -> Option<String>;
    /// 批量原子设置
    async fn set_batch(&self, values: HashMap<String, String>) -> Result<()>;
    /// 获取所有配置
    async fn snapshot(&self) -> HashMap<String, String>;
    /// 回滚到指定版本
    async fn rollback(&self, version: u64) -> Result<()>;
}

/// 策略执行接口 — 将优化结果下发到执行子系统
#[async_trait::async_trait]
pub trait StrategyExecutor: Send + Sync {
    /// 更新角色对应的超时、并发限制、工具权限等
    async fn apply_role_profile(
        &self,
        role: &str,
        timeout_ms: Option<u64>,
        concurrency_limit: Option<i64>,
        enable_reflection: Option<bool>,
    ) -> Result<()>;
    /// 临时降级某个工具/模型的使用频率
    async fn throttle_tool(&self, tool_name: &str, factor: f32) -> Result<()>;
    /// 提升某策略的权重
    async fn boost_strategy(&self, strategy_id: &str, boost: f32) -> Result<()>;
}

/// 优化器 v2
pub struct OptimizerV2 {
    applied_improvements: Arc<RwLock<Vec<AppliedImprovement>>>,
    strategy_scores: Arc<RwLock<HashMap<String, StrategyStats>>>,
    last_optimization: Arc<RwLock<Option<String>>>,
    config_manager: Arc<dyn ConfigManager>,
    strategy_executor: Arc<dyn StrategyExecutor>,
    // 可回滚的历史配置
    config_history: Arc<RwLock<Vec<ConfigSnapshot>>>,
}

/// 已应用的改进
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedImprovement {
    pub improvement_id: String,
    pub improvement_type: String,
    pub applied_at: String,
    pub effectiveness: f32,
    pub config_changes: Vec<ConfigChange>,
    pub rollback_version: Option<u64>,
}

/// 配置变更记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChange {
    pub key: String,
    pub old_value: Option<String>,
    pub new_value: String,
    pub reason: String,
}

/// 配置快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    pub version: u64,
    pub timestamp: String,
    pub configs: HashMap<String, String>,
}

/// 策略统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyStats {
    pub strategy: String,
    pub usage_count: u64,
    pub success_count: u64,
    pub avg_confidence: f32,
    pub avg_duration_ms: f64,
    pub last_applied_at: Option<String>,
}

impl StrategyStats {
    fn new(strategy: String) -> Self {
        Self {
            strategy,
            usage_count: 0,
            success_count: 0,
            avg_confidence: 0.5,
            avg_duration_ms: 0.0,
            last_applied_at: None,
        }
    }

    fn record_result(&mut self, success: bool, confidence: f32, duration_ms: u64) {
        self.usage_count += 1;
        if success {
            self.success_count += 1;
        }
        let n = self.usage_count as f32;
        self.avg_confidence = (self.avg_confidence * (n - 1.0) + confidence) / n;
        let n_f64 = self.usage_count as f64;
        self.avg_duration_ms = (self.avg_duration_ms * (n_f64 - 1.0) + duration_ms as f64) / n_f64;
    }

    fn success_rate(&self) -> f32 {
        if self.usage_count == 0 {
            0.0
        } else {
            self.success_count as f32 / self.usage_count as f32
        }
    }
}

/// 优化结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationResult {
    pub improvements_count: usize,
    pub strategies_optimized: usize,
    pub expected_improvements: Vec<String>,
    pub config_changes_applied: Vec<ConfigChange>,
    pub rollback_version: u64,
}

impl OptimizerV2 {
    pub fn new(
        config_manager: Arc<dyn ConfigManager>,
        strategy_executor: Arc<dyn StrategyExecutor>,
    ) -> Self {
        Self {
            applied_improvements: Arc::new(RwLock::new(Vec::new())),
            strategy_scores: Arc::new(RwLock::new(HashMap::new())),
            last_optimization: Arc::new(RwLock::new(None)),
            config_manager,
            strategy_executor,
            config_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 应用改进 — 实际修改系统配置
    pub async fn apply_improvements(
        &self,
        knowledge: &[LearnedKnowledge],
    ) -> Result<OptimizationResult> {
        debug!("Applying {} improvements (v2)", knowledge.len());

        // 1. 创建配置快照用于回滚
        let snapshot = self.create_config_snapshot().await;
        let rollback_version = snapshot.version;
        {
            let mut history = self.config_history.write().await;
            history.push(snapshot);
            // 保留最近 20 个快照
            if history.len() > 20 {
                history.remove(0);
            }
        }

        let mut improvements_count = 0;
        let mut strategies_optimized = 0;
        let mut expected_improvements = Vec::new();
        let mut all_changes: Vec<ConfigChange> = Vec::new();

        for item in knowledge {
            let changes = match item.knowledge_type.as_str() {
                "task_pattern" => {
                    if let Some(strategy) = self.optimize_for_task_pattern(item).await? {
                        strategies_optimized += 1;
                        expected_improvements.push(strategy.clone());
                    }
                    improvements_count += 1;
                    vec![]
                }
                "error_pattern" => {
                    let changes = self.apply_error_prevention(item).await?;
                    if !changes.is_empty() {
                        expected_improvements.push(format!(
                            "Error prevention applied: {} changes",
                            changes.len()
                        ));
                    }
                    improvements_count += 1;
                    changes
                }
                "successful_strategy" => {
                    if let Some(strategy) = self.reinforce_strategy(item).await? {
                        strategies_optimized += 1;
                        expected_improvements.push(strategy.clone());
                    }
                    improvements_count += 1;
                    vec![]
                }
                "performance_bottleneck" => {
                    let changes = self.optimize_performance(item).await?;
                    improvements_count += 1;
                    changes
                }
                _ => {
                    debug!("Skipping unknown knowledge type: {}", item.knowledge_type);
                    vec![]
                }
            };
            all_changes.extend(changes);
        }

        // 2. 批量原子应用所有配置变更
        if !all_changes.is_empty() {
            let mut batch = HashMap::new();
            for change in &all_changes {
                batch.insert(change.key.clone(), change.new_value.clone());
            }
            self.config_manager.set_batch(batch).await?;
        }

        // 3. 记录已应用的改进
        let improvement_record = AppliedImprovement {
            improvement_id: format!("imp-{}", uuid::Uuid::new_v4()),
            improvement_type: "batch_optimization".to_string(),
            applied_at: chrono::Utc::now().to_rfc3339(),
            effectiveness: 0.0, // 待后续评估
            config_changes: all_changes.clone(),
            rollback_version: Some(rollback_version),
        };
        {
            let mut improvements = self.applied_improvements.write().await;
            improvements.push(improvement_record);
        }

        *self.last_optimization.write().await = Some(chrono::Utc::now().to_rfc3339());

        info!(
            "Optimization v2 completed: {} improvements, {} strategies optimized, {} config changes",
            improvements_count, strategies_optimized, all_changes.len()
        );

        Ok(OptimizationResult {
            improvements_count,
            strategies_optimized,
            expected_improvements,
            config_changes_applied: all_changes,
            rollback_version,
        })
    }

    /// 为任务模式优化 — 实际修改角色配置和策略权重
    async fn optimize_for_task_pattern(
        &self,
        knowledge: &LearnedKnowledge,
    ) -> Result<Option<String>> {
        let category = self.extract_category(&knowledge.content)?;
        let best_strategy = self.select_best_strategy_for_category(&category).await;

        if let Some(strategy) = best_strategy {
            debug!(
                "Selected strategy '{}' for category '{}'",
                strategy, category
            );

            // 实际下发策略到执行系统
            let boost = (knowledge.confidence * 0.5).clamp(0.1, 1.0);
            self.strategy_executor
                .boost_strategy(&strategy, boost)
                .await?;

            // 根据类别动态调整角色配置
            match category.as_str() {
                "coding" => {
                    self.strategy_executor
                        .apply_role_profile(
                            "coder",
                            Some(90_000), // 提升超时到 90s
                            Some(3),      // 提升并发
                            Some(true),
                        )
                        .await?;
                }
                "analysis" => {
                    self.strategy_executor
                        .apply_role_profile("analyst", Some(120_000), Some(5), Some(true))
                        .await?;
                }
                "explanation" | "general" => {
                    self.strategy_executor
                        .apply_role_profile("drafter", Some(30_000), Some(4), Some(false))
                        .await?;
                }
                _ => {}
            }

            Ok(Some(format!(
                "Optimized strategy for category '{}': {}, applied config changes",
                category, strategy
            )))
        } else {
            debug!("No suitable strategy found for category '{}'", category);
            Ok(None)
        }
    }

    /// 应用错误预防 — 实际修改系统运行参数
    async fn apply_error_prevention(
        &self,
        knowledge: &LearnedKnowledge,
    ) -> Result<Vec<ConfigChange>> {
        let error_type = self.extract_error_type(&knowledge.content)?;
        debug!("Applying error prevention for: {}", error_type);

        let mut changes = Vec::new();
        let now = chrono::Utc::now().to_rfc3339();

        match error_type.as_str() {
            "timeout" | "llm_timeout" => {
                // 提升超时阈值，减少并发
                let old_timeout = self
                    .config_manager
                    .get("swarm.default_timeout_ms")
                    .await
                    .unwrap_or_else(|| "30000".to_string());
                let new_timeout = (old_timeout.parse::<u64>().unwrap_or(30000) * 12 / 10)
                    .clamp(30000, 300000)
                    .to_string();

                changes.push(ConfigChange {
                    key: "swarm.default_timeout_ms".to_string(),
                    old_value: Some(old_timeout.clone()),
                    new_value: new_timeout.clone(),
                    reason: format!(
                        "Timeout error prevention: {} -> {}",
                        old_timeout, new_timeout
                    ),
                });

                // 临时限流高频出错的工具
                self.strategy_executor
                    .throttle_tool("web_search", 0.5)
                    .await?;
            }
            "rate_limit" | "api_error" => {
                // 降低并发，增加重试间隔
                let old_concurrency = self
                    .config_manager
                    .get("swarm.max_concurrent")
                    .await
                    .unwrap_or_else(|| "20".to_string());
                let new_concurrency = (old_concurrency.parse::<u64>().unwrap_or(20) / 2)
                    .max(1)
                    .to_string();

                changes.push(ConfigChange {
                    key: "swarm.max_concurrent".to_string(),
                    old_value: Some(old_concurrency.clone()),
                    new_value: new_concurrency.clone(),
                    reason: format!(
                        "Rate limit prevention: concurrency {} -> {}",
                        old_concurrency, new_concurrency
                    ),
                });
            }
            "hallucination" | "incorrect_output" => {
                // 启用更严格的验证层
                changes.push(ConfigChange {
                    key: "cognitive.validation_strictness".to_string(),
                    old_value: self
                        .config_manager
                        .get("cognitive.validation_strictness")
                        .await,
                    new_value: "high".to_string(),
                    reason: "Hallucination prevention: strict validation".to_string(),
                });
            }
            _ => {
                // 通用错误预防：记录错误模式到黑名单
                changes.push(ConfigChange {
                    key: format!("error_pattern.blacklist.{}", error_type.replace(' ', "_")),
                    old_value: None,
                    new_value: now.clone(),
                    reason: format!("Generic error prevention for: {}", error_type),
                });
            }
        }

        Ok(changes)
    }

    /// 强化成功策略 — 不仅记录评分，还提升策略权重和资源配置
    async fn reinforce_strategy(&self, knowledge: &LearnedKnowledge) -> Result<Option<String>> {
        let strategy = self.extract_strategy(&knowledge.content)?;

        // 更新内部评分
        self.record_strategy_success(&strategy, knowledge.confidence)
            .await;

        // 实际提升策略在执行系统中的权重
        let boost = (knowledge.confidence * 0.3).clamp(0.05, 0.5);
        self.strategy_executor
            .boost_strategy(&strategy, boost)
            .await?;

        // 如果成功率高，尝试提升相关角色的资源配置
        let stats = {
            let strategies = self.strategy_scores.read().await;
            strategies.get(&strategy).cloned()
        };
        if let Some(stats) = stats {
            if stats.success_rate() > 0.85 && stats.usage_count > 5 {
                // 这是一个经过验证的好策略，提升其角色的并发限制
                let role = if strategy.contains("coding") {
                    "coder"
                } else if strategy.contains("analysis") {
                    "analyst"
                } else {
                    "researcher"
                };
                self.strategy_executor
                    .apply_role_profile(role, None, Some(2), None)
                    .await?;
            }
        }

        Ok(Some(format!(
            "Reinforced successful strategy: {} (confidence: {:.2}), applied weight boost",
            strategy, knowledge.confidence
        )))
    }

    /// 性能瓶颈优化
    async fn optimize_performance(
        &self,
        knowledge: &LearnedKnowledge,
    ) -> Result<Vec<ConfigChange>> {
        let mut changes = Vec::new();
        // 解析性能指标
        if knowledge.content.contains("slow") || knowledge.content.contains("timeout") {
            let old_batch = self
                .config_manager
                .get("tools.batch_size")
                .await
                .unwrap_or_else(|| "1".to_string());
            let new_batch = (old_batch.parse::<usize>().unwrap_or(1) * 2)
                .max(5)
                .to_string();
            changes.push(ConfigChange {
                key: "tools.batch_size".to_string(),
                old_value: Some(old_batch),
                new_value: new_batch,
                reason: "Performance optimization: increase batch size".to_string(),
            });
        }
        Ok(changes)
    }

    /// 选择最佳策略
    async fn select_best_strategy_for_category(&self, category: &str) -> Option<String> {
        let strategies = self.strategy_scores.read().await;
        let mut candidates: Vec<_> = strategies
            .values()
            .filter(|s| self.is_strategy_suitable_for_category(s, category))
            .collect();

        if candidates.is_empty() {
            return Some("default".to_string());
        }

        candidates.sort_by(|a, b| {
            b.success_rate()
                .partial_cmp(&a.success_rate())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Some(candidates.first()?.strategy.clone())
    }

    fn is_strategy_suitable_for_category(&self, stats: &StrategyStats, category: &str) -> bool {
        matches!(
            (stats.strategy.as_str(), category),
            ("enhanced", "coding" | "analysis")
                | ("fast", "explanation" | "general")
                | ("default", _)
        )
    }

    async fn record_strategy_success(&self, strategy: &str, confidence: f32) {
        let mut strategies = self.strategy_scores.write().await;
        strategies
            .entry(strategy.to_string())
            .or_insert_with(|| StrategyStats::new(strategy.to_string()))
            .record_result(true, confidence, 100);
    }

    pub async fn record_strategy_result(
        &self,
        strategy: &str,
        success: bool,
        confidence: f32,
        duration_ms: u64,
    ) {
        let mut strategies = self.strategy_scores.write().await;
        let stats = strategies
            .entry(strategy.to_string())
            .or_insert_with(|| StrategyStats::new(strategy.to_string()));
        stats.record_result(success, confidence, duration_ms);
        stats.last_applied_at = Some(chrono::Utc::now().to_rfc3339());
    }

    fn extract_category(&self, content: &str) -> Result<String> {
        let parts: Vec<&str> = content.split(',').collect();
        if let Some(first) = parts.first() {
            let category = first
                .split(':')
                .next_back()
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "general".to_string());
            Ok(category)
        } else {
            Ok("general".to_string())
        }
    }

    fn extract_error_type(&self, content: &str) -> Result<String> {
        let parts: Vec<&str> = content.split(':').collect();
        if let Some(error_type) = parts.get(2) {
            Ok(error_type.trim().to_string())
        } else {
            Ok("unknown".to_string())
        }
    }

    fn extract_strategy(&self, content: &str) -> Result<String> {
        Ok(content
            .split(':')
            .next_back()
            .unwrap_or("default")
            .trim()
            .to_string())
    }

    pub async fn get_strategy_stats(&self) -> Vec<StrategyStats> {
        self.strategy_scores
            .read()
            .await
            .values()
            .cloned()
            .collect()
    }

    pub async fn get_best_strategy(&self) -> Option<String> {
        let strategies = self.strategy_scores.read().await;
        if strategies.is_empty() {
            return Some("default".to_string());
        }
        strategies
            .values()
            .max_by(|a, b| {
                b.success_rate()
                    .partial_cmp(&a.success_rate())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|s| s.strategy.clone())
    }

    pub async fn get_applied_improvements(&self) -> Vec<AppliedImprovement> {
        self.applied_improvements.read().await.clone()
    }

    pub async fn total_improvements_async(&self) -> usize {
        self.applied_improvements.read().await.len()
    }

    pub async fn last_optimization_async(&self) -> Option<String> {
        self.last_optimization.read().await.clone()
    }

    pub fn last_optimization(&self) -> Option<String> {
        self.last_optimization
            .try_read()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// 回滚到指定版本
    pub async fn rollback(&self, version: u64) -> Result<()> {
        warn!("Rolling back optimizer changes to version {}", version);
        self.config_manager.rollback(version).await?;

        // 标记相关改进为已回滚
        let mut improvements = self.applied_improvements.write().await;
        for imp in improvements.iter_mut() {
            if imp.rollback_version == Some(version) {
                imp.effectiveness = -1.0; // 标记为已回滚
            }
        }
        Ok(())
    }

    /// 创建配置快照
    async fn create_config_snapshot(&self) -> ConfigSnapshot {
        let configs = self.config_manager.snapshot().await;
        ConfigSnapshot {
            version: chrono::Utc::now().timestamp_millis() as u64,
            timestamp: chrono::Utc::now().to_rfc3339(),
            configs,
        }
    }
}

impl Default for OptimizerV2 {
    fn default() -> Self {
        // NOTE: Default impl requires dummy implementations or is for testing only
        panic!("OptimizerV2 requires ConfigManager and StrategyExecutor to be constructed")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockConfigManager {
        store: Arc<RwLock<HashMap<String, String>>>,
    }

    #[async_trait::async_trait]
    impl ConfigManager for MockConfigManager {
        async fn get(&self, key: &str) -> Option<String> {
            self.store.read().await.get(key).cloned()
        }
        async fn set(&self, key: &str, value: String) -> Option<String> {
            self.store.write().await.insert(key.to_string(), value)
        }
        async fn set_batch(&self, values: HashMap<String, String>) -> Result<()> {
            let mut store = self.store.write().await;
            for (k, v) in values {
                store.insert(k, v);
            }
            Ok(())
        }
        async fn snapshot(&self) -> HashMap<String, String> {
            self.store.read().await.clone()
        }
        async fn rollback(&self, _version: u64) -> Result<()> {
            Ok(())
        }
    }

    struct MockStrategyExecutor;

    #[async_trait::async_trait]
    impl StrategyExecutor for MockStrategyExecutor {
        async fn apply_role_profile(
            &self,
            _role: &str,
            _timeout_ms: Option<u64>,
            _concurrency_limit: Option<i64>,
            _enable_reflection: Option<bool>,
        ) -> Result<()> {
            Ok(())
        }
        async fn throttle_tool(&self, _tool_name: &str, _factor: f32) -> Result<()> {
            Ok(())
        }
        async fn boost_strategy(&self, _strategy_id: &str, _boost: f32) -> Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_optimizer_v2_apply_improvements() {
        let config = Arc::new(MockConfigManager {
            store: Arc::new(RwLock::new(HashMap::new())),
        });
        let executor = Arc::new(MockStrategyExecutor);
        let optimizer = OptimizerV2::new(config, executor);

        let knowledge = vec![LearnedKnowledge {
            knowledge_id: "test-1".into(),
            knowledge_type: "error_pattern".into(),
            content: "Error pattern: timeout, severity: 0.8, root_cause: slow_llm".into(),
            related_patterns: vec![],
            confidence: 0.9,
        }];

        let result = optimizer.apply_improvements(&knowledge).await.unwrap();
        assert_eq!(result.improvements_count, 1);
        assert!(!result.config_changes_applied.is_empty());
        assert!(result.rollback_version > 0);
    }
}
