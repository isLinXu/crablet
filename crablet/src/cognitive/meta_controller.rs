//! Meta-Cognitive Controller - 监控、反思、学习、优化的核心控制器
//!
//! # 概述
//!
//! 元认知控制器实现了完整的元认知循环：
//! 1. **监控** (Monitor) - 监控执行过程，收集性能指标
//! 2. **反思** (Reflector) - 分析问题，生成改进建议
//! 3. **学习** (Learner) - 提取模式，优化策略
//! 4. **优化** (Optimizer) - 应用改进，提升性能
//!
//! # 架构
//!
//! ```text
//! ┌─────────────┐
//! │   Monitor   │ ← 监控执行
//! └──────┬──────┘
//!        ↓
//! ┌─────────────┐
//! │  Reflector  │ ← 反思问题
//! └──────┬──────┘
//!        ↓
//! ┌─────────────┐
//! │   Learner   │ ← 学习模式
//! └──────┬──────┘
//!        ↓
//! ┌─────────────┐
//! │  Optimizer  │ ← 优化策略
//! └─────────────┘
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, debug, error};

use crate::error::{Result, CrabletError};
use crate::cognitive::llm::LlmClient;
use crate::types::Message;

// 导入子模块
pub mod monitor;
pub mod reflector;
pub mod learner;
pub mod optimizer;

// 重新导出核心类型
pub use monitor::{Monitor, ExecutionMetrics, QualityMetrics, ResourceMetrics};
pub use reflector::{Reflector, ProblemDiagnosis, ImprovementAction};
pub use learner::{Learner, Pattern, PatternType, LearnedKnowledge};
pub use optimizer::{Optimizer, OptimizationResult};

/// 元认知控制器
#[derive(Clone)]
pub struct MetaCognitiveController {
    monitor: Arc<RwLock<Monitor>>,
    reflector: Arc<RwLock<Reflector>>,
    learner: Arc<RwLock<Learner>>,
    optimizer: Arc<RwLock<Optimizer>>,
    llm: Arc<Box<dyn LlmClient>>,
    config: MetaConfig,
}

/// 元认知配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaConfig {
    /// 监控间隔
    pub monitor_interval: Duration,
    /// 最大反馈历史
    pub max_feedback_history: usize,
    /// 最大模式数量
    pub max_patterns: usize,
    /// 学习阈值
    pub learning_threshold: f32,
    /// 启用自动优化
    pub enable_auto_optimization: bool,
    /// 优化间隔
    pub optimization_interval: Duration,
}

impl Default for MetaConfig {
    fn default() -> Self {
        Self {
            monitor_interval: Duration::from_millis(100),
            max_feedback_history: 1000,
            max_patterns: 1000,
            learning_threshold: 0.6,
            enable_auto_optimization: true,
            optimization_interval: Duration::from_secs(60),
        }
    }
}

/// 执行请求
#[derive(Debug, Clone)]
pub struct ExecutionRequest {
    pub task_id: String,
    pub task: String,
    pub context: Vec<Message>,
    pub start_time: Instant,
}

/// 执行结果
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub task_id: String,
    pub success: bool,
    pub output: String,
    pub confidence: f32,
    pub duration: Duration,
    pub metrics: ExecutionMetrics,
}

/// 元认知统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaStatistics {
    pub total_tasks: u64,
    pub successful_tasks: u64,
    pub failed_tasks: u64,
    pub avg_confidence: f32,
    pub avg_duration_ms: f64,
    pub patterns_extracted: usize,
    pub improvements_applied: usize,
    pub last_optimization: Option<String>,
}

impl MetaCognitiveController {
    /// 创建新的元认知控制器
    pub async fn new(llm: Arc<Box<dyn LlmClient>>) -> Result<Self> {
        Self::with_config(llm, MetaConfig::default()).await
    }

    /// 使用自定义配置创建
    pub async fn with_config(llm: Arc<Box<dyn LlmClient>>, config: MetaConfig) -> Result<Self> {
        let monitor = Arc::new(RwLock::new(Monitor::new(config.max_feedback_history)));
        let reflector = Arc::new(RwLock::new(Reflector::new(llm.clone())));
        let learner = Arc::new(RwLock::new(Learner::new(config.max_patterns)));
        let optimizer = Arc::new(RwLock::new(Optimizer::new()));

        Ok(Self {
            monitor,
            reflector,
            learner,
            optimizer,
            llm,
            config,
        })
    }

    /// 执行任务（带元认知监控）
    pub async fn execute_with_meta(&self, request: ExecutionRequest, mut executor: impl FnMut(&ExecutionRequest) -> ExecutionResult) -> ExecutionResult {
        let start_time = Instant::now();

        // 开始监控
        {
            let monitor = self.monitor.read().await;
            monitor.start_execution(&request.task_id).await;
        }

        // 执行任务
        let mut result = executor(&request);

        // 更新结果中的持续时间
        result.duration = start_time.elapsed();

        // 完成监控并收集指标
        {
            let monitor = self.monitor.read().await;
            monitor.finish_execution(&request.task_id, &result).await;
        }

        // 如果失败，触发反思循环
        if !result.success || result.confidence < self.config.learning_threshold {
            if let Err(e) = self.trigger_reflection(&request, &result).await {
                warn!("Reflection failed: {}", e);
            }
        }

        result
    }

    /// 触发反思循环
    async fn trigger_reflection(&self, request: &ExecutionRequest, _result: &ExecutionResult) -> Result<()> {
        debug!("Triggering reflection for task {}", request.task);

        // 获取执行指标
        let metrics = {
            let monitor = self.monitor.read().await;
            monitor.get_metrics(&request.task_id).await
                .ok_or_else(|| CrabletError::Cognitive("Metrics not found".into()))?
        };

        // 反思问题
        let diagnosis = {
            let reflector = self.reflector.read().await;
            reflector.diagnose(&request.task, &metrics).await?
        };

        info!("Diagnosed problem: {:?}", diagnosis.problem_type);

        // 学习模式
        let learned = {
            let learner = self.learner.read().await;
            learner.learn_from_experience(&request.task, &metrics, &diagnosis).await?
        };

        info!("Learned {} new patterns", learned.len());

        // 优化策略
        if self.config.enable_auto_optimization {
            let optimizer = self.optimizer.write().await;
            let optimization = optimizer.apply_improvements(&learned).await?;

            info!("Applied {} optimizations", optimization.improvements_count);
        }

        Ok(())
    }

    /// 获取统计信息
    pub async fn get_statistics(&self) -> MetaStatistics {
        let monitor = self.monitor.read().await;
        let learner = self.learner.read().await;
        let optimizer = self.optimizer.read().await;

        let metrics = monitor.get_global_metrics().await;
        let patterns = learner.get_all_patterns().await;

        MetaStatistics {
            total_tasks: metrics.total_executions,
            successful_tasks: metrics.successful_executions,
            failed_tasks: metrics.failed_executions,
            avg_confidence: metrics.avg_confidence,
            avg_duration_ms: metrics.avg_duration_ms,
            patterns_extracted: patterns.len(),
            improvements_applied: optimizer.get_applied_improvements().await.len(),
            last_optimization: None, // TODO: Implement async version
        }
    }

    /// 导出学习到的知识
    pub async fn export_knowledge(&self) -> Result<Vec<LearnedKnowledge>> {
        let learner = self.learner.read().await;
        let patterns = learner.get_all_patterns().await;
        // 将 Pattern 转换为 LearnedKnowledge
        Ok(patterns.into_iter().map(|p| LearnedKnowledge {
            knowledge_id: p.id,
            knowledge_type: format!("{:?}", p.pattern_type),
            content: p.description,
            related_patterns: p.trigger_conditions,
            confidence: p.success_rate,
        }).collect())
    }

    /// 集成反馈
    pub async fn integrate_feedback(&self, task_id: &str, feedback: f32) -> Result<()> {
        let monitor = self.monitor.read().await;
        monitor.record_feedback(task_id, feedback).await;
        Ok(())
    }

    /// 启动后台优化
    pub async fn start_background_optimization(&self) -> Result<()> {
        if !self.config.enable_auto_optimization {
            return Ok(());
        }

        let controller = self.clone();
        let interval = self.config.optimization_interval;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;

                match controller.run_optimization_cycle().await {
                    Ok(()) => debug!("Optimization cycle completed"),
                    Err(e) => error!("Optimization cycle failed: {}", e),
                }
            }
        });

        Ok(())
    }

    /// 运行优化循环
    async fn run_optimization_cycle(&self) -> Result<()> {
        debug!("Starting optimization cycle");

        // 获取所有模式并转换为 LearnedKnowledge
        let patterns: Vec<LearnedKnowledge> = {
            let learner = self.learner.read().await;
            let raw_patterns = learner.get_all_patterns().await;
            // 将 Pattern 转换为 LearnedKnowledge
            raw_patterns.into_iter().map(|p| LearnedKnowledge {
                knowledge_id: p.id,
                knowledge_type: format!("{:?}", p.pattern_type),
                content: p.description,
                related_patterns: p.trigger_conditions,
                confidence: p.success_rate,
            }).collect()
        };

        if patterns.is_empty() {
            debug!("No patterns to optimize");
            return Ok(());
        }

        // 应用优化
        let optimizer = self.optimizer.read().await;
        let result = optimizer.apply_improvements(&patterns).await?;

        info!(
            "Optimization completed: {} improvements applied",
            result.improvements_count
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognitive::llm::MockClient;

    #[tokio::test]
    async fn test_controller_creation() {
        let llm = Arc::new(Box::new(MockClient) as Box<dyn LlmClient>);
        let controller = MetaCognitiveController::new(llm).await;
        assert!(controller.is_ok());
    }

    #[tokio::test]
    async fn test_execute_with_meta() {
        let llm = Arc::new(Box::new(MockClient) as Box<dyn LlmClient>);
        let controller = MetaCognitiveController::new(llm).await.unwrap();

        let request = ExecutionRequest {
            task_id: "test-1".into(),
            task: "Test task".into(),
            context: vec![],
            start_time: Instant::now(),
        };

        let result = controller.execute_with_meta(request, |req| ExecutionResult {
            task_id: req.task_id.clone(),
            success: true,
            output: "Test output".into(),
            confidence: 0.8,
            duration: Duration::from_millis(100),
            metrics: ExecutionMetrics::default(),
        }).await;

        assert!(result.success);
    }

    #[tokio::test]
    async fn test_statistics() {
        let llm = Arc::new(Box::new(MockClient) as Box<dyn LlmClient>);
        let controller = MetaCognitiveController::new(llm).await.unwrap();

        let stats = controller.get_statistics().await;
        assert_eq!(stats.total_tasks, 0);
    }
}
