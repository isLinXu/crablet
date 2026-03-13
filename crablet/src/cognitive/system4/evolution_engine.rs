//! Evolution Engine - 进化引擎
//!
//! 基于性能分析和技能发现，自动优化系统配置和架构

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{info, warn};

use crate::cognitive::system4::performance_analyzer::{PerformanceAnalyzer, PerformanceReport, CognitiveSystemType};
use crate::cognitive::system4::skill_discoverer::SkillDiscoverer;

/// 进化配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionConfig {
    pub enabled: bool,
    pub analysis_interval_seconds: u64,
    pub min_samples_for_evolution: usize,
    pub confidence_threshold: f64,
    pub auto_apply_minor_changes: bool,
    pub require_approval_for_major_changes: bool,
    pub max_changes_per_cycle: usize,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            analysis_interval_seconds: 3600, // 1小时
            min_samples_for_evolution: 50,
            confidence_threshold: 0.75,
            auto_apply_minor_changes: true,
            require_approval_for_major_changes: true,
            max_changes_per_cycle: 5,
        }
    }
}

/// 改进提案
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementProposal {
    pub id: String,
    pub title: String,
    pub description: String,
    pub proposal_type: ProposalType,
    pub target_system: CognitiveSystemType,
    pub severity: ChangeSeverity,
    pub expected_impact: ExpectedImpact,
    pub confidence: f64,
    pub proposed_changes: Vec<ProposedChange>,
    pub created_at: DateTime<Utc>,
    pub status: ProposalStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalType {
    RoutingOptimization,
    ThresholdAdjustment,
    SkillIntegration,
    ParameterTuning,
    ArchitectureChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeSeverity {
    Minor,      // 自动应用
    Moderate,   // 建议但需确认
    Major,      // 必须确认
    Critical,   // 需要人工审核
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedImpact {
    pub latency_improvement_percent: f64,
    pub accuracy_improvement_percent: f64,
    pub resource_usage_change_percent: f64,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedChange {
    pub target: String,
    pub current_value: serde_json::Value,
    pub proposed_value: serde_json::Value,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    Pending,
    Approved,
    Rejected,
    Applied,
    Failed,
}

/// 系统配置快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfiguration {
    pub version: String,
    pub timestamp: DateTime<Utc>,
    pub routing_thresholds: RoutingThresholds,
    pub system_parameters: HashMap<String, serde_json::Value>,
}

impl Default for SystemConfiguration {
    fn default() -> Self {
        Self {
            version: "1.0.0".to_string(),
            timestamp: Utc::now(),
            routing_thresholds: RoutingThresholds::default(),
            system_parameters: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingThresholds {
    pub system1_confidence: f32,
    pub system2_confidence: f32,
    pub system3_confidence: f32,
    pub skill_auto_execute: f32,
    pub clarification_threshold: f32,
}

impl Default for RoutingThresholds {
    fn default() -> Self {
        Self {
            system1_confidence: 0.85,
            system2_confidence: 0.70,
            system3_confidence: 0.60,
            skill_auto_execute: 0.80,
            clarification_threshold: 0.40,
        }
    }
}

/// 进化历史
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionRecord {
    pub timestamp: DateTime<Utc>,
    pub proposal_id: String,
    pub changes_applied: Vec<ProposedChange>,
    pub before_config: SystemConfiguration,
    pub after_config: SystemConfiguration,
    pub result: EvolutionResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionResult {
    pub success: bool,
    pub actual_latency_change_percent: f64,
    pub actual_accuracy_change_percent: f64,
    pub error_message: Option<String>,
}

/// 进化引擎
pub struct EvolutionEngine {
    config: Arc<RwLock<EvolutionConfig>>,
    performance_analyzer: Arc<PerformanceAnalyzer>,
    skill_discoverer: Arc<SkillDiscoverer>,
    proposals: Arc<RwLock<Vec<ImprovementProposal>>>,
    evolution_history: Arc<RwLock<Vec<EvolutionRecord>>>,
    current_config: Arc<RwLock<SystemConfiguration>>,
}

impl EvolutionEngine {
    pub fn new(
        performance_analyzer: Arc<PerformanceAnalyzer>,
        skill_discoverer: Arc<SkillDiscoverer>,
    ) -> Self {
        Self {
            config: Arc::new(RwLock::new(EvolutionConfig::default())),
            performance_analyzer,
            skill_discoverer,
            proposals: Arc::new(RwLock::new(Vec::new())),
            evolution_history: Arc::new(RwLock::new(Vec::new())),
            current_config: Arc::new(RwLock::new(SystemConfiguration {
                version: "1.0.0".to_string(),
                timestamp: Utc::now(),
                routing_thresholds: RoutingThresholds::default(),
                system_parameters: HashMap::new(),
            })),
        }
    }

    pub fn with_config(
        config: EvolutionConfig,
        performance_analyzer: Arc<PerformanceAnalyzer>,
        skill_discoverer: Arc<SkillDiscoverer>,
    ) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            performance_analyzer,
            skill_discoverer,
            proposals: Arc::new(RwLock::new(Vec::new())),
            evolution_history: Arc::new(RwLock::new(Vec::new())),
            current_config: Arc::new(RwLock::new(SystemConfiguration {
                version: "1.0.0".to_string(),
                timestamp: Utc::now(),
                routing_thresholds: RoutingThresholds::default(),
                system_parameters: HashMap::new(),
            })),
        }
    }

    /// 启动进化循环
    pub async fn start_evolution_loop(self: Arc<Self>) {
        let config = self.config.read().await;
        if !config.enabled {
            info!("Evolution engine is disabled");
            return;
        }
        let interval_seconds = config.analysis_interval_seconds;
        drop(config);

        info!("Starting evolution loop with {}s interval", interval_seconds);

        let mut ticker = interval(Duration::from_secs(interval_seconds));

        loop {
            ticker.tick().await;
            
            if let Err(e) = self.run_evolution_cycle().await {
                warn!("Evolution cycle failed: {}", e);
            }
        }
    }

    /// 运行一次进化周期
    pub async fn run_evolution_cycle(&self) -> anyhow::Result<()> {
        info!("Running evolution cycle...");

        // 1. 生成性能报告
        let report = self.performance_analyzer.generate_report(24).await;
        
        // 2. 基于性能报告生成改进提案
        let new_proposals = self.generate_proposals_from_report(&report).await;
        
        // 3. 存储提案
        {
            let mut proposals = self.proposals.write().await;
            for proposal in new_proposals {
                info!("Generated proposal: {} ({:?})", proposal.title, proposal.proposal_type);
                proposals.push(proposal);
            }
        }

        // 4. 应用符合条件的提案
        self.apply_eligible_proposals().await?;

        info!("Evolution cycle completed");
        Ok(())
    }

    /// 从性能报告生成改进提案
    async fn generate_proposals_from_report(&self, report: &PerformanceReport) -> Vec<ImprovementProposal> {
        let mut proposals = Vec::new();

        // 分析每个系统的性能
        for (system_type, stats) in &report.overall_stats {
            // 检查错误率
            if stats.error_rate > 0.15 {
                proposals.push(self.create_threshold_adjustment_proposal(*system_type, stats));
            }

            // 检查延迟
            if stats.p95_latency_ms > 3000.0 {
                proposals.push(self.create_routing_optimization_proposal(*system_type, stats));
            }

            // 检查成功率
            if (stats.successful_requests as f64) / (stats.total_requests as f64) < 0.9 {
                proposals.push(self.create_fallback_proposal(*system_type, stats));
            }
        }

        // 分析趋势
        for trend in &report.trends {
            if matches!(trend.direction, super::performance_analyzer::TrendDirection::Degrading) {
                proposals.push(self.create_trend_response_proposal(trend));
            }
        }

        proposals
    }

    fn create_threshold_adjustment_proposal(&self, system_type: CognitiveSystemType, stats: &super::performance_analyzer::PerformanceStats) -> ImprovementProposal {
        let current_config = self
            .current_config
            .try_read()
            .map(|cfg| cfg.clone())
            .unwrap_or_else(|_| SystemConfiguration::default());
        let current_threshold = match system_type {
            CognitiveSystemType::System1 => current_config.routing_thresholds.system1_confidence,
            CognitiveSystemType::System2 => current_config.routing_thresholds.system2_confidence,
            CognitiveSystemType::System3 => current_config.routing_thresholds.system3_confidence,
        };

        // 根据错误率调整阈值
        let adjustment = if stats.error_rate > 0.3 {
            0.1 // 大幅提高阈值
        } else if stats.error_rate > 0.2 {
            0.05 // 适度提高
        } else {
            0.02 // 小幅提高
        };

        let new_threshold = (current_threshold + adjustment).min(0.95);

        ImprovementProposal {
            id: format!("prop_{}", uuid::Uuid::new_v4().to_string()[..8].to_string()),
            title: format!("Adjust {:?} confidence threshold", system_type),
            description: format!(
                "Error rate for {:?} is {:.1}%. Increasing threshold from {:.2} to {:.2} to improve quality.",
                system_type, stats.error_rate * 100.0, current_threshold, new_threshold
            ),
            proposal_type: ProposalType::ThresholdAdjustment,
            target_system: system_type,
            severity: if stats.error_rate > 0.3 { ChangeSeverity::Major } else { ChangeSeverity::Minor },
            expected_impact: ExpectedImpact {
                latency_improvement_percent: -5.0, // 可能略微增加延迟
                accuracy_improvement_percent: stats.error_rate * 50.0, // 预期减少错误
                resource_usage_change_percent: 0.0,
                description: "Reduce error rate by being more selective".to_string(),
            },
            confidence: 0.8,
            proposed_changes: vec![ProposedChange {
                target: format!("{:?}_confidence_threshold", system_type).to_lowercase(),
                current_value: serde_json::json!(current_threshold),
                proposed_value: serde_json::json!(new_threshold),
                rationale: format!("Error rate of {:.1}% exceeds acceptable threshold", stats.error_rate * 100.0),
            }],
            created_at: Utc::now(),
            status: ProposalStatus::Pending,
        }
    }

    fn create_routing_optimization_proposal(&self, system_type: CognitiveSystemType, stats: &super::performance_analyzer::PerformanceStats) -> ImprovementProposal {
        // 建议路由到更快的系统
        let target_system = match system_type {
            CognitiveSystemType::System3 => CognitiveSystemType::System2,
            CognitiveSystemType::System2 => CognitiveSystemType::System1,
            CognitiveSystemType::System1 => CognitiveSystemType::System1, // System1 已经是最快的
        };

        ImprovementProposal {
            id: format!("prop_{}", uuid::Uuid::new_v4().to_string()[..8].to_string()),
            title: format!("Optimize routing from {:?} to {:?}", system_type, target_system),
            description: format!(
                "{:?} P95 latency is {:.0}ms. Consider routing more queries to {:?} for better performance.",
                system_type, stats.p95_latency_ms, target_system
            ),
            proposal_type: ProposalType::RoutingOptimization,
            target_system: system_type,
            severity: ChangeSeverity::Moderate,
            expected_impact: ExpectedImpact {
                latency_improvement_percent: 30.0,
                accuracy_improvement_percent: -2.0, // 可能略微降低准确性
                resource_usage_change_percent: -10.0,
                description: "Improve latency by using faster system".to_string(),
            },
            confidence: 0.75,
            proposed_changes: vec![ProposedChange {
                target: "routing_strategy".to_string(),
                current_value: serde_json::json!("default"),
                proposed_value: serde_json::json!("latency_optimized"),
                rationale: format!("High P95 latency of {:.0}ms", stats.p95_latency_ms),
            }],
            created_at: Utc::now(),
            status: ProposalStatus::Pending,
        }
    }

    fn create_fallback_proposal(&self, system_type: CognitiveSystemType, stats: &super::performance_analyzer::PerformanceStats) -> ImprovementProposal {
        ImprovementProposal {
            id: format!("prop_{}", uuid::Uuid::new_v4().to_string()[..8].to_string()),
            title: format!("Add fallback mechanism for {:?}", system_type),
            description: format!(
                "{:?} success rate is {:.1}%. Adding fallback to alternative system could improve reliability.",
                system_type,
                (stats.successful_requests as f64 / stats.total_requests as f64) * 100.0
            ),
            proposal_type: ProposalType::ArchitectureChange,
            target_system: system_type,
            severity: ChangeSeverity::Major,
            expected_impact: ExpectedImpact {
                latency_improvement_percent: 0.0,
                accuracy_improvement_percent: 5.0,
                resource_usage_change_percent: 10.0,
                description: "Improve reliability with fallback".to_string(),
            },
            confidence: 0.7,
            proposed_changes: vec![ProposedChange {
                target: "fallback_enabled".to_string(),
                current_value: serde_json::json!(false),
                proposed_value: serde_json::json!(true),
                rationale: "Low success rate requires fallback mechanism".to_string(),
            }],
            created_at: Utc::now(),
            status: ProposalStatus::Pending,
        }
    }

    fn create_trend_response_proposal(&self, trend: &super::performance_analyzer::PerformanceTrend) -> ImprovementProposal {
        ImprovementProposal {
            id: format!("prop_{}", uuid::Uuid::new_v4().to_string()[..8].to_string()),
            title: format!("Address degrading trend in {:?}", trend.system_type),
            description: format!(
                "{:?} {} has degraded by {:.1}% over the past {} hours.",
                trend.system_type, trend.metric, trend.change_percent.abs(), trend.window_hours
            ),
            proposal_type: ProposalType::ParameterTuning,
            target_system: trend.system_type,
            severity: ChangeSeverity::Moderate,
            expected_impact: ExpectedImpact {
                latency_improvement_percent: trend.change_percent.abs(),
                accuracy_improvement_percent: 0.0,
                resource_usage_change_percent: 0.0,
                description: "Reverse degrading trend".to_string(),
            },
            confidence: 0.65,
            proposed_changes: vec![ProposedChange {
                target: "trend_monitoring".to_string(),
                current_value: serde_json::json!("normal"),
                proposed_value: serde_json::json!("enhanced"),
                rationale: format!("Degrading trend detected: {:.1}%", trend.change_percent),
            }],
            created_at: Utc::now(),
            status: ProposalStatus::Pending,
        }
    }

    /// 应用符合条件的提案
    async fn apply_eligible_proposals(&self) -> anyhow::Result<()> {
        let config = self.config.read().await;
        let max_changes = config.max_changes_per_cycle;
        let auto_apply_minor = config.auto_apply_minor_changes;
        let require_approval = config.require_approval_for_major_changes;
        drop(config);

        let mut proposals_to_apply = Vec::new();

        {
            let proposals = self.proposals.read().await;
            for proposal in proposals.iter().filter(|p| p.status == ProposalStatus::Pending) {
                let should_apply = match proposal.severity {
                    ChangeSeverity::Minor => auto_apply_minor,
                    ChangeSeverity::Moderate => auto_apply_minor && proposal.confidence > 0.8,
                    ChangeSeverity::Major => !require_approval && proposal.confidence > 0.9,
                    ChangeSeverity::Critical => false, // 永远需要人工审核
                };

                if should_apply && proposals_to_apply.len() < max_changes {
                    proposals_to_apply.push(proposal.clone());
                }
            }
        }

        for proposal in proposals_to_apply {
            if let Err(e) = self.apply_proposal(&proposal).await {
                warn!("Failed to apply proposal {}: {}", proposal.id, e);
            }
        }

        Ok(())
    }

    /// 应用单个提案
    pub async fn apply_proposal(&self, proposal: &ImprovementProposal) -> anyhow::Result<()> {
        info!("Applying proposal: {}", proposal.title);

        // 保存当前配置
        let before_config = self.current_config.read().await.clone();

        // 应用变更
        {
            let mut config = self.current_config.write().await;
            
            for change in &proposal.proposed_changes {
                self.apply_change(&mut config, change).await?;
            }

            config.version = format!("{}+evolved", config.version);
            config.timestamp = Utc::now();
        }

        // 记录进化历史
        let after_config = self.current_config.read().await.clone();
        let record = EvolutionRecord {
            timestamp: Utc::now(),
            proposal_id: proposal.id.clone(),
            changes_applied: proposal.proposed_changes.clone(),
            before_config,
            after_config,
            result: EvolutionResult {
                success: true,
                actual_latency_change_percent: 0.0, // 将在后续测量
                actual_accuracy_change_percent: 0.0,
                error_message: None,
            },
        };

        {
            let mut history = self.evolution_history.write().await;
            history.push(record);
        }

        // 更新提案状态
        {
            let mut proposals = self.proposals.write().await;
            if let Some(p) = proposals.iter_mut().find(|p| p.id == proposal.id) {
                p.status = ProposalStatus::Applied;
            }
        }

        info!("Successfully applied proposal: {}", proposal.title);
        Ok(())
    }

    async fn apply_change(&self, config: &mut SystemConfiguration, change: &ProposedChange) -> anyhow::Result<()> {
        match change.target.as_str() {
            "system1_confidence_threshold" => {
                if let Some(val) = change.proposed_value.as_f64() {
                    config.routing_thresholds.system1_confidence = val as f32;
                }
            }
            "system2_confidence_threshold" => {
                if let Some(val) = change.proposed_value.as_f64() {
                    config.routing_thresholds.system2_confidence = val as f32;
                }
            }
            "system3_confidence_threshold" => {
                if let Some(val) = change.proposed_value.as_f64() {
                    config.routing_thresholds.system3_confidence = val as f32;
                }
            }
            _ => {
                // 通用参数更新
                config.system_parameters.insert(change.target.clone(), change.proposed_value.clone());
            }
        }
        Ok(())
    }

    /// 获取待处理的提案
    pub async fn get_pending_proposals(&self) -> Vec<ImprovementProposal> {
        self.proposals
            .read()
            .await
            .iter()
            .filter(|p| p.status == ProposalStatus::Pending)
            .cloned()
            .collect()
    }

    /// 获取所有提案
    pub async fn get_all_proposals(&self) -> Vec<ImprovementProposal> {
        self.proposals.read().await.clone()
    }

    /// 获取进化历史
    pub async fn get_evolution_history(&self) -> Vec<EvolutionRecord> {
        self.evolution_history.read().await.clone()
    }

    /// 获取当前配置
    pub async fn get_current_config(&self) -> SystemConfiguration {
        self.current_config.read().await.clone()
    }

    /// 更新配置
    pub async fn update_config(&self, new_config: EvolutionConfig) {
        *self.config.write().await = new_config;
    }

    /// 批准提案（人工审核）
    pub async fn approve_proposal(&self, proposal_id: &str) -> anyhow::Result<()> {
        // 先找到提案并克隆
        let proposal = {
            let mut proposals = self.proposals.write().await;
            if let Some(p) = proposals.iter_mut().find(|p| p.id == proposal_id) {
                p.status = ProposalStatus::Approved;
                p.clone()
            } else {
                return Err(anyhow::anyhow!("Proposal not found: {}", proposal_id));
            }
        };
        
        // 立即应用
        self.apply_proposal(&proposal).await?;
        Ok(())
    }

    /// 拒绝提案
    pub async fn reject_proposal(&self, proposal_id: &str) -> anyhow::Result<()> {
        let mut proposals = self.proposals.write().await;
        
        if let Some(proposal) = proposals.iter_mut().find(|p| p.id == proposal_id) {
            proposal.status = ProposalStatus::Rejected;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Proposal not found: {}", proposal_id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognitive::system4::performance_analyzer::{PerformanceAnalyzer, ExecutionMetrics};
    use crate::cognitive::system4::skill_discoverer::SkillDiscoverer;
    use crate::cognitive::system4::performance_analyzer::CognitiveSystemType as PerfCognitiveSystemType;

    #[tokio::test]
    async fn test_proposal_generation() {
        let perf_analyzer = Arc::new(PerformanceAnalyzer::new());
        let skill_discoverer = Arc::new(SkillDiscoverer::new());
        
        let engine = EvolutionEngine::new(perf_analyzer.clone(), skill_discoverer);

        // 记录一些性能数据（高错误率）
        for i in 0..20 {
            perf_analyzer.record_metrics(ExecutionMetrics {
                system_type: PerfCognitiveSystemType::System2,
                query: format!("test {}", i),
                latency_ms: 2000,
                success: i < 12, // 40% 失败率
                token_count: 100,
                tool_calls: 2,
                agent_count: 1,
                timestamp: Utc::now(),
                context_length: 500,
                retry_count: 0,
            }).await;
        }

        // 运行进化周期
        engine.run_evolution_cycle().await.unwrap();

        // 检查是否生成了提案
        let proposals = engine.get_pending_proposals().await;
        assert!(!proposals.is_empty());
        
        // 应该有一个关于错误率的提案
        let error_proposal = proposals.iter()
            .find(|p| matches!(p.proposal_type, ProposalType::ThresholdAdjustment));
        assert!(error_proposal.is_some());
    }
}
