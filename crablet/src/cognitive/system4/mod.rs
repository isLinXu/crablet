//! System 4 - Self-Evolving Cognitive Layer
//!
//! System4 是元认知层之上的自我进化系统，负责：
//! - 性能分析与瓶颈识别
//! - 技能自动发现与生成
//! - 知识蒸馏（将复杂执行转化为可复用知识）
//! - 系统架构自优化

pub mod performance_analyzer;
pub mod skill_discoverer;
pub mod knowledge_distiller;
pub mod evolution_engine;

use std::sync::Arc;
use async_trait::async_trait;
use chrono::Utc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::cognitive::CognitiveSystem;
use crate::cognitive::system4::skill_discoverer::{
    SkillDiscoverer, ExecutionRecord, SkillDiscoveryConfig
};
use crate::cognitive::system4::knowledge_distiller::{
    KnowledgeDistiller, DistillationTask, ExecutionTrace
};
use crate::cognitive::system4::evolution_engine::{
    EvolutionEngine, EvolutionConfig, ImprovementProposal, SystemConfiguration
};
use crate::cognitive::llm::LlmClient;
use crate::types::{Message, TraceStep};
use crate::error::{Result, CrabletError};

pub use performance_analyzer::{
    PerformanceAnalyzer, ExecutionMetrics, CognitiveSystemType as SystemType, PerformanceReport,
    PerformanceStats, BottleneckAnalysis, TrendDirection
};
pub use skill_discoverer::{SkillCandidate, ExecutionPattern};
pub use knowledge_distiller::{DistillationResult, DistilledKnowledge, ExtractedSkill};
pub use evolution_engine::{ProposalType, ChangeSeverity, ProposalStatus};

/// System4 配置
#[derive(Debug, Clone)]
pub struct System4Config {
    pub enabled: bool,
    pub auto_evolution: bool,
    pub evolution_interval_seconds: u64,
    pub min_confidence_for_skill_creation: f64,
    pub max_history_size: usize,
}

impl Default for System4Config {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_evolution: true,
            evolution_interval_seconds: 3600, // 1小时
            min_confidence_for_skill_creation: 0.75,
            max_history_size: 100000,
        }
    }
}

/// System4 - 自我进化认知系统
pub struct System4 {
    config: Arc<RwLock<System4Config>>,
    
    // 核心引擎
    performance_analyzer: Arc<PerformanceAnalyzer>,
    skill_discoverer: Arc<SkillDiscoverer>,
    knowledge_distiller: Arc<KnowledgeDistiller>,
    evolution_engine: Arc<EvolutionEngine>,
    
    // 执行历史（用于技能发现）
    execution_history: Arc<RwLock<Vec<ExecutionRecord>>>,
    
    // LLM 客户端
    llm: Arc<Box<dyn LlmClient>>,
}

impl System4 {
    /// 创建新的 System4 实例
    pub async fn new(llm: Arc<Box<dyn LlmClient>>) -> Self {
        let perf_analyzer = Arc::new(PerformanceAnalyzer::new());
        let skill_discoverer = Arc::new(SkillDiscoverer::new());
        let knowledge_distiller = Arc::new(KnowledgeDistiller::new(llm.clone()));
        let evolution_engine = Arc::new(EvolutionEngine::new(
            perf_analyzer.clone(),
            skill_discoverer.clone(),
        ));

        Self {
            config: Arc::new(RwLock::new(System4Config::default())),
            performance_analyzer: perf_analyzer,
            skill_discoverer,
            knowledge_distiller,
            evolution_engine,
            execution_history: Arc::new(RwLock::new(Vec::new())),
            llm,
        }
    }

    /// 使用配置创建
    pub async fn with_config(
        llm: Arc<Box<dyn LlmClient>>,
        config: System4Config,
    ) -> Self {
        let perf_analyzer = Arc::new(PerformanceAnalyzer::with_capacity(config.max_history_size));
        let skill_discoverer = Arc::new(SkillDiscoverer::with_config(SkillDiscoveryConfig::default()));
        let knowledge_distiller = Arc::new(KnowledgeDistiller::with_capacity(
            llm.clone(),
            config.max_history_size,
        ));
        let evolution_engine = Arc::new(EvolutionEngine::with_config(
            EvolutionConfig::default(),
            perf_analyzer.clone(),
            skill_discoverer.clone(),
        ));

        Self {
            config: Arc::new(RwLock::new(config)),
            performance_analyzer: perf_analyzer,
            skill_discoverer,
            knowledge_distiller,
            evolution_engine,
            execution_history: Arc::new(RwLock::new(Vec::new())),
            llm,
        }
    }

    /// 启动 System4 的后台任务
    pub async fn start(self: Arc<Self>) {
        let config = self.config.read().await;
        if !config.enabled {
            info!("System4 is disabled");
            return;
        }
        let auto_evolution = config.auto_evolution;
        drop(config);

        info!("Starting System4 self-evolution engine");

        if auto_evolution {
            let evolution_engine = self.evolution_engine.clone();
            tokio::spawn(async move {
                evolution_engine.start_evolution_loop().await;
            });
        }
    }

    /// 记录 System1 执行
    pub async fn record_system1_execution(
        &self,
        query: &str,
        latency_ms: u64,
        success: bool,
    ) {
        self.record_execution(SystemType::System1, query, None, latency_ms, success, 0, 0).await;
    }

    /// 记录 System2 执行
    pub async fn record_system2_execution(
        &self,
        query: &str,
        output: Option<&str>,
        latency_ms: u64,
        success: bool,
        token_count: usize,
        tool_calls: usize,
    ) {
        self.record_execution(
            SystemType::System2,
            query,
            output,
            latency_ms,
            success,
            token_count,
            tool_calls,
        ).await;
    }

    /// 记录 System3 执行
    pub async fn record_system3_execution(
        &self,
        query: &str,
        output: Option<&str>,
        latency_ms: u64,
        success: bool,
        agent_count: usize,
        execution_trace: Option<ExecutionTrace>,
    ) {
        self.record_execution(
            SystemType::System3,
            query,
            output,
            latency_ms,
            success,
            0,
            agent_count,
        ).await;

        // 对于 System3，还尝试进行知识蒸馏
        if let Some(trace) = execution_trace {
            if success {
                let task = DistillationTask {
                    id: format!("distill_{}", uuid::Uuid::new_v4().to_string()[..8].to_string()),
                    source_system: knowledge_distiller::DistillationSourceSystem::System3,
                    input: query.to_string(),
                    output: output.unwrap_or("").to_string(),
                    execution_trace: trace,
                    priority: knowledge_distiller::DistillationPriority::Medium,
                };

                // 在后台进行蒸馏
                let distiller = self.knowledge_distiller.clone();
                tokio::spawn(async move {
                    let result = distiller.distill(task).await;
                    if result.success {
                        debug!("Knowledge distillation completed for task {}", result.task_id);
                    }
                });
            }
        }
    }

    /// 通用执行记录
    async fn record_execution(
        &self,
        system_type: SystemType,
        query: &str,
        output: Option<&str>,
        latency_ms: u64,
        success: bool,
        token_count: usize,
        tool_or_agent_count: usize,
    ) {
        // 记录到性能分析器
        let metrics = ExecutionMetrics {
            system_type,
            query: query.to_string(),
            latency_ms,
            success,
            token_count,
            tool_calls: if system_type == SystemType::System2 { tool_or_agent_count } else { 0 },
            agent_count: if system_type == SystemType::System3 { tool_or_agent_count } else { 1 },
            timestamp: Utc::now(),
            context_length: query.len(),
            retry_count: 0,
        };

        self.performance_analyzer.record_metrics(metrics).await;

        // 记录到执行历史（用于技能发现）
        let record = ExecutionRecord {
            query: query.to_string(),
            output: output.map(|s| s.to_string()),
            tool_sequence: vec![], // 简化处理
            success,
            latency_ms,
            timestamp: Utc::now(),
        };

        let mut history = self.execution_history.write().await;
        history.push(record);

        // 限制历史大小
        if history.len() > 10000 {
            history.remove(0);
        }
    }

    /// 手动触发技能发现
    pub async fn discover_skills(&self) -> Vec<SkillCandidate> {
        let history = self.execution_history.read().await.clone();
        
        if history.len() < 10 {
            info!("Not enough execution history for skill discovery (need 10, have {})", history.len());
            return Vec::new();
        }

        info!("Running skill discovery on {} executions", history.len());
        self.skill_discoverer.discover_patterns(&history).await
    }

    /// 手动触发进化周期
    pub async fn trigger_evolution(&self) -> Result<()> {
        self.evolution_engine
            .run_evolution_cycle()
            .await
            .map_err(|e| CrabletError::Other(e.into()))
    }

    /// 生成性能报告
    pub async fn generate_performance_report(&self, window_hours: u32) -> PerformanceReport {
        self.performance_analyzer.generate_report(window_hours).await
    }

    /// 获取待处理的改进提案
    pub async fn get_pending_proposals(&self) -> Vec<ImprovementProposal> {
        self.evolution_engine.get_pending_proposals().await
    }

    /// 批准改进提案
    pub async fn approve_proposal(&self, proposal_id: &str) -> Result<()> {
        self.evolution_engine
            .approve_proposal(proposal_id)
            .await
            .map_err(|e| CrabletError::Other(e.into()))
    }

    /// 拒绝改进提案
    pub async fn reject_proposal(&self, proposal_id: &str) -> Result<()> {
        self.evolution_engine
            .reject_proposal(proposal_id)
            .await
            .map_err(|e| CrabletError::Other(e.into()))
    }

    /// 获取当前系统配置
    pub async fn get_system_configuration(&self) -> SystemConfiguration {
        self.evolution_engine.get_current_config().await
    }

    /// 查询已蒸馏的知识
    pub async fn query_knowledge(&self, topic: &str) -> Vec<DistilledKnowledge> {
        self.knowledge_distiller.query_knowledge(topic).await
    }

    /// 获取提取的技能
    pub async fn get_extracted_skills(&self) -> Vec<ExtractedSkill> {
        self.knowledge_distiller.get_extracted_skills().await
    }

    /// 获取进化历史
    pub async fn get_evolution_history(&self) -> Vec<evolution_engine::EvolutionRecord> {
        self.evolution_engine.get_evolution_history().await
    }

    /// 更新配置
    pub async fn update_config(&self, new_config: System4Config) {
        *self.config.write().await = new_config;
    }

    /// 获取 System4 状态报告
    pub async fn get_status_report(&self) -> System4StatusReport {
        let config = self.config.read().await;
        let history = self.execution_history.read().await;
        let proposals = self.evolution_engine.get_all_proposals().await;
        
        System4StatusReport {
            enabled: config.enabled,
            auto_evolution: config.auto_evolution,
            total_executions_recorded: history.len(),
            pending_proposals: proposals.iter().filter(|p| p.status == ProposalStatus::Pending).count(),
            applied_changes: proposals.iter().filter(|p| p.status == ProposalStatus::Applied).count(),
            extracted_skills_count: self.knowledge_distiller.get_extracted_skills().await.len(),
            distilled_knowledge_count: self.knowledge_distiller.query_knowledge("").await.len(),
        }
    }
}

#[async_trait]
impl CognitiveSystem for System4 {
    fn name(&self) -> &str {
        "System 4 (Self-Evolving)"
    }

    async fn process(&self, input: &str, _context: &[Message]) -> Result<(String, Vec<TraceStep>)> {
        // System4 主要作为元层运行，不直接处理用户输入
        // 但可以响应特定的管理命令
        
        let response = match input.to_lowercase().as_str() {
            cmd if cmd.contains("status") || cmd.contains("状态") => {
                let report = self.get_status_report().await;
                format!(
                    "System4 Status:\n\
                    - Enabled: {}\n\
                    - Auto Evolution: {}\n\
                    - Executions Recorded: {}\n\
                    - Pending Proposals: {}\n\
                    - Applied Changes: {}\n\
                    - Extracted Skills: {}\n\
                    - Distilled Knowledge: {}",
                    report.enabled,
                    report.auto_evolution,
                    report.total_executions_recorded,
                    report.pending_proposals,
                    report.applied_changes,
                    report.extracted_skills_count,
                    report.distilled_knowledge_count
                )
            }
            cmd if cmd.contains("report") || cmd.contains("报告") => {
                let perf_report = self.generate_performance_report(24).await;
                format!(
                    "Performance Report (24h):\n\
                    - Generated: {}\n\
                    - Insights: {}\n\
                    - Bottlenecks: {}\n\
                    - Trends: {}",
                    perf_report.generated_at,
                    perf_report.insights.join("; "),
                    perf_report.bottlenecks.len(),
                    perf_report.trends.len()
                )
            }
            cmd if cmd.contains("proposals") || cmd.contains("提案") => {
                let proposals = self.get_pending_proposals().await;
                if proposals.is_empty() {
                    "No pending improvement proposals.".to_string()
                } else {
                    let mut response = "Pending Proposals:\n".to_string();
                    for (i, p) in proposals.iter().take(5).enumerate() {
                        response.push_str(&format!(
                            "{}. {} ({:?}) - {}\n",
                            i + 1,
                            p.title,
                            p.severity,
                            p.description.chars().take(50).collect::<String>()
                        ));
                    }
                    response
                }
            }
            _ => {
                "System4 is the self-evolution layer. Available commands:\n\
                - status: Show System4 status\n\
                - report: Show performance report\n\
                - proposals: List pending proposals".to_string()
            }
        };

        let traces = vec![TraceStep {
            step: 1,
            thought: "System4 meta-layer processing".to_string(),
            action: Some("meta_query".to_string()),
            action_input: Some(input.to_string()),
            observation: Some("Returned System4 status".to_string()),
        }];

        Ok((response, traces))
    }
}

/// System4 状态报告
#[derive(Debug, Clone)]
pub struct System4StatusReport {
    pub enabled: bool,
    pub auto_evolution: bool,
    pub total_executions_recorded: usize,
    pub pending_proposals: usize,
    pub applied_changes: usize,
    pub extracted_skills_count: usize,
    pub distilled_knowledge_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mocks::MockLlmClient;

    #[tokio::test]
    async fn test_system4_creation() {
        let llm = Arc::new(Box::new(MockLlmClient::new()) as Box<dyn LlmClient>);
        let system4 = System4::new(llm).await;

        assert_eq!(system4.name(), "System 4 (Self-Evolving)");
    }

    #[tokio::test]
    async fn test_execution_recording() {
        let llm = Arc::new(Box::new(MockLlmClient::new()) as Box<dyn LlmClient>);
        let system4 = System4::new(llm).await;

        // 记录一些执行
        for i in 0..5 {
            system4.record_system2_execution(
                &format!("test query {}", i),
                Some("test output"),
                1000,
                true,
                100,
                2,
            ).await;
        }

        let report = system4.get_status_report().await;
        assert_eq!(report.total_executions_recorded, 5);
    }
}
