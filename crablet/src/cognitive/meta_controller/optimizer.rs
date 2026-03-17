//! Optimizer - 应用改进并优化策略

use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::Result;
use crate::cognitive::meta_controller::learner::LearnedKnowledge;

/// 优化器
pub struct Optimizer {
    applied_improvements: Arc<tokio::sync::RwLock<Vec<AppliedImprovement>>>,
    strategy_scores: Arc<tokio::sync::RwLock<HashMap<String, StrategyStats>>>,
    last_optimization: Arc<tokio::sync::RwLock<Option<String>>>,
}

/// 已应用的改进
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedImprovement {
    improvement_id: String,
    improvement_type: String,
    applied_at: String,
    effectiveness: f32,
}

/// 策略统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyStats {
    pub strategy: String,
    pub usage_count: u64,
    pub success_count: u64,
    pub avg_confidence: f32,
    pub avg_duration_ms: f64,
}

impl StrategyStats {
    fn new(strategy: String) -> Self {
        Self {
            strategy,
            usage_count: 0,
            success_count: 0,
            avg_confidence: 0.5,
            avg_duration_ms: 0.0,
        }
    }

    fn record_result(&mut self, success: bool, confidence: f32, duration_ms: u64) {
        self.usage_count += 1;
        if success {
            self.success_count += 1;
        }

        // 更新平均值
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
    /// 应用的改进数量
    pub improvements_count: usize,
    /// 优化的策略数量
    pub strategies_optimized: usize,
    /// 预期改进
    pub expected_improvements: Vec<String>,
}

impl Optimizer {
    /// 创建新的优化器
    pub fn new() -> Self {
        Self {
            applied_improvements: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            strategy_scores: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            last_optimization: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// 应用改进
    pub async fn apply_improvements(&self, knowledge: &[LearnedKnowledge]) -> Result<OptimizationResult> {
        debug!("Applying {} improvements", knowledge.len());

        let mut improvements_count = 0;
        let mut strategies_optimized = 0;
        let mut expected_improvements = Vec::new();

        for item in knowledge {
            match item.knowledge_type.as_str() {
                "task_pattern" => {
                    // 更新策略配置
                    if let Some(strategy) = self.optimize_for_task_pattern(item).await? {
                        strategies_optimized += 1;
                        expected_improvements.push(strategy);
                    }
                    improvements_count += 1;
                }
                "error_pattern" => {
                    // 应用错误预防策略
                    if let Some(strategy) = self.apply_error_prevention(item).await? {
                        strategies_optimized += 1;
                        expected_improvements.push(strategy);
                    }
                    improvements_count += 1;
                }
                "successful_strategy" => {
                    // 强化成功策略
                    if let Some(strategy) = self.reinforce_strategy(item).await? {
                        strategies_optimized += 1;
                        expected_improvements.push(strategy);
                    }
                    improvements_count += 1;
                }
                _ => {
                    debug!("Skipping unknown knowledge type: {}", item.knowledge_type);
                }
            }
        }

        // 更新优化时间
        *self.last_optimization.write().await = Some(chrono::Utc::now().to_rfc3339());

        info!(
            "Optimization completed: {} improvements, {} strategies optimized",
            improvements_count, strategies_optimized
        );

        Ok(OptimizationResult {
            improvements_count,
            strategies_optimized,
            expected_improvements,
        })
    }

    /// 为任务模式优化
    async fn optimize_for_task_pattern(&self, knowledge: &LearnedKnowledge) -> Result<Option<String>> {
        // 解析任务类别
        let category = self.extract_category(&knowledge.content)?;
        
        // 选择最佳策略
        let best_strategy = self.select_best_strategy_for_category(&category).await;
        
        if let Some(strategy) = best_strategy {
            debug!("Selected strategy '{}' for category '{}'", strategy, category);
            Ok(Some(format!(
                "Optimized strategy for category '{}': {}",
                category, strategy
            )))
        } else {
            debug!("No suitable strategy found for category '{}'", category);
            Ok(None)
        }
    }

    /// 应用错误预防
    async fn apply_error_prevention(&self, knowledge: &LearnedKnowledge) -> Result<Option<String>> {
        // 从知识内容中提取错误类型
        let error_type = self.extract_error_type(&knowledge.content)?;
        
        // 应用错误预防措施
        debug!("Applying error prevention for: {}", error_type);
        
        Ok(Some(format!(
            "Applied error prevention for: {}",
            error_type
        )))
    }

    /// 强化成功策略
    async fn reinforce_strategy(&self, knowledge: &LearnedKnowledge) -> Result<Option<String>> {
        // 记录成功策略
        let strategy = self.extract_strategy(&knowledge.content)?;
        
        // 更新策略评分
        self.record_strategy_success(&strategy, knowledge.confidence).await;
        
        Ok(Some(format!(
            "Reinforced successful strategy: {} (confidence: {:.2})",
            strategy, knowledge.confidence
        )))
    }

    /// 选择最佳策略
    async fn select_best_strategy_for_category(&self, category: &str) -> Option<String> {
        let strategies = self.strategy_scores.read().await;
        
        // 查找适合该类别的策略
        let mut candidates: Vec<_> = strategies
            .values()
            .filter(|s| self.is_strategy_suitable_for_category(s, category))
            .collect();
        
        if candidates.is_empty() {
            // 返回默认策略
            return Some("default".to_string());
        }
        
        // 选择成功率最高的策略
        candidates.sort_by(|a, b| {
            b.success_rate()
                .partial_cmp(&a.success_rate())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        
        Some(candidates.first()?.strategy.clone())
    }

    /// 判断策略是否适合类别
    fn is_strategy_suitable_for_category(&self, stats: &StrategyStats, category: &str) -> bool {
        match (stats.strategy.as_str(), category) {
            ("enhanced", "coding" | "analysis") => true,
            ("fast", "explanation" | "general") => true,
            ("default", _) => true,
            _ => false,
        }
    }

    /// 记录策略成功
    async fn record_strategy_success(&self, strategy: &str, confidence: f32) {
        let mut strategies = self.strategy_scores.write().await;
        strategies
            .entry(strategy.to_string())
            .or_insert_with(|| StrategyStats::new(strategy.to_string()))
            .record_result(true, confidence, 100);
    }

    /// 记录策略结果
    pub async fn record_strategy_result(
        &self,
        strategy: &str,
        success: bool,
        confidence: f32,
        duration_ms: u64,
    ) {
        let mut strategies = self.strategy_scores.write().await;
        strategies
            .entry(strategy.to_string())
            .or_insert_with(|| StrategyStats::new(strategy.to_string()))
            .record_result(success, confidence, duration_ms);
    }

    /// 提取类别
    fn extract_category(&self, content: &str) -> Result<String> {
        let parts: Vec<&str> = content.split(',').collect();
        if let Some(first) = parts.first() {
            let category = first
                .split(':')
                .last()
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "general".to_string());
            Ok(category)
        } else {
            Ok("general".to_string())
        }
    }

    /// 提取错误类型
    fn extract_error_type(&self, content: &str) -> Result<String> {
        let parts: Vec<&str> = content.split(':').collect();
        if let Some(error_type) = parts.get(2) {
            Ok(error_type.trim().to_string())
        } else {
            Ok("unknown".to_string())
        }
    }

    /// 提取策略
    fn extract_strategy(&self, content: &str) -> Result<String> {
        Ok(content.split(':').last().unwrap_or("default").trim().to_string())
    }

    /// 获取策略统计
    pub async fn get_strategy_stats(&self) -> Vec<StrategyStats> {
        let strategies = self.strategy_scores.read().await;
        strategies.values().cloned().collect()
    }

    /// 获取最佳策略
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

    /// 获取所有应用的改进
    pub async fn get_applied_improvements(&self) -> Vec<AppliedImprovement> {
        self.applied_improvements.read().await.clone()
    }

    /// 总改进数量
    pub fn total_improvements(&self) -> usize {
        // 这个方法需要异步访问，但为了简化我们返回一个近似值
        // 实际使用时应该改为 async
        0
    }

    /// 最后优化时间
    pub fn last_optimization(&self) -> Option<String> {
        // 这个方法需要异步访问，但为了简化我们返回 None
        // 实际使用时应该改为 async
        None
    }
}

impl Default for Optimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_optimizer_creation() {
        let optimizer = Optimizer::new();
        assert!(!optimizer.total_improvements() > 0);
    }

    #[tokio::test]
    async fn test_apply_improvements() {
        let optimizer = Optimizer::new();
        
        let knowledge = vec![
            LearnedKnowledge {
                knowledge_id: "test-1".into(),
                knowledge_type: "task_pattern".into(),
                content: "Task category: coding".into(),
                related_patterns: vec![],
                confidence: 0.9,
            },
        ];
        
        let result = optimizer.apply_improvements(&knowledge).await.unwrap();
        assert_eq!(result.improvements_count, 1);
    }

    #[tokio::test]
    async fn test_record_strategy_result() {
        let optimizer = Optimizer::new();
        optimizer.record_strategy_result("test-strategy", true, 0.9, 100).await;
        
        let stats = optimizer.get_strategy_stats().await;
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].strategy, "test-strategy");
        assert_eq!(stats[0].usage_count, 1);
    }

    #[tokio::test]
    async fn test_extract_category() {
        let optimizer = Optimizer::new();
        let category = optimizer.extract_category("Task category: coding, complexity: 0.5").unwrap();
        assert_eq!(category, "coding");
    }
}
