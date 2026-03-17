//! Monitor - 监控执行过程并收集性能指标

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tracing::debug;

/// 执行监控器
pub struct Monitor {
    executions: Arc<RwLock<HashMap<String, ExecutionState>>>,
    global_metrics: Arc<RwLock<GlobalMetrics>>,
    max_history: usize,
}

/// 执行状态
#[derive(Debug, Clone)]
struct ExecutionState {
    start_time: Instant,
    end_time: Option<Instant>,
    metrics: ExecutionMetrics,
}

/// 执行指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    /// 置信度
    pub confidence: f32,
    /// 输出质量
    pub quality_score: f32,
    /// 资源使用
    pub resources: ResourceMetrics,
    /// 成功状态
    pub success: bool,
    /// 错误信息
    pub error: Option<String>,
}

impl Default for ExecutionMetrics {
    fn default() -> Self {
        Self {
            confidence: 0.5,
            quality_score: 0.5,
            resources: ResourceMetrics::default(),
            success: false,
            error: None,
        }
    }
}

/// 资源指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetrics {
    /// 执行时间（毫秒）
    pub duration_ms: u64,
    /// 内存使用（字节）
    pub memory_bytes: u64,
    /// CPU 时间（毫秒）
    pub cpu_ms: u64,
    /// 令牌消耗
    pub tokens_used: u64,
}

impl Default for ResourceMetrics {
    fn default() -> Self {
        Self {
            duration_ms: 0,
            memory_bytes: 0,
            cpu_ms: 0,
            tokens_used: 0,
        }
    }
}

/// 质量指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// 完整性 (0-1)
    pub completeness: f32,
    /// 准确性 (0-1)
    pub accuracy: f32,
    /// 相关性 (0-1)
    pub relevance: f32,
    /// 清晰度 (0-1)
    pub clarity: f32,
}

impl Default for QualityMetrics {
    fn default() -> Self {
        Self {
            completeness: 0.5,
            accuracy: 0.5,
            relevance: 0.5,
            clarity: 0.5,
        }
    }
}

impl QualityMetrics {
    /// 计算综合质量分数
    pub fn overall_score(&self) -> f32 {
        (self.completeness + self.accuracy + self.relevance + self.clarity) / 4.0
    }
}

/// 全局指标
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GlobalMetrics {
    total_executions: u64,
    successful_executions: u64,
    failed_executions: u64,
    avg_confidence: f32,
    avg_duration_ms: f64,
    feedback_history: VecDeque<f32>,
}

impl Default for GlobalMetrics {
    fn default() -> Self {
        Self {
            total_executions: 0,
            successful_executions: 0,
            failed_executions: 0,
            avg_confidence: 0.5,
            avg_duration_ms: 0.0,
            feedback_history: VecDeque::new(),
        }
    }
}

impl Monitor {
    /// 创建新的监控器
    pub fn new(max_history: usize) -> Self {
        Self {
            executions: Arc::new(RwLock::new(HashMap::new())),
            global_metrics: Arc::new(RwLock::new(GlobalMetrics::default())),
            max_history,
        }
    }

    /// 开始监控执行
    pub async fn start_execution(&self, task_id: &str) {
        let mut executions = self.executions.write().await;
        executions.insert(
            task_id.to_string(),
            ExecutionState {
                start_time: Instant::now(),
                end_time: None,
                metrics: ExecutionMetrics::default(),
            },
        );
    }

    /// 完成执行并收集指标
    pub async fn finish_execution(&self, task_id: &str, result: &ExecutionResult) {
        let mut executions = self.executions.write().await;
        let mut global = self.global_metrics.write().await;
        
        if let Some(state) = executions.get_mut(task_id) {
            state.end_time = Some(Instant::now());
            state.metrics = result.metrics.clone();
            
            // 更新全局指标
            global.total_executions += 1;
            if result.success {
                global.successful_executions += 1;
            } else {
                global.failed_executions += 1;
            }
            
            // 更新平均置信度
            let n = global.total_executions as f32;
            global.avg_confidence =
                (global.avg_confidence * (n - 1.0) + result.confidence) / n;

            // 更新平均持续时间
            let n_f64 = global.total_executions as f64;
            let duration_ms = result.duration.as_millis() as f64;
            global.avg_duration_ms =
                (global.avg_duration_ms * (n_f64 - 1.0) + duration_ms) / n_f64;
        }
        
        debug!("Finished execution: {}", task_id);
    }

    /// 记录反馈
    pub async fn record_feedback(&self, task_id: &str, feedback: f32) {
        let mut global = self.global_metrics.write().await;
        
        // 添加反馈历史
        if global.feedback_history.len() >= self.max_history {
            global.feedback_history.pop_front();
        }
        global.feedback_history.push_back(feedback.clamp(0.0, 1.0));
        
        debug!("Recorded feedback for {}: {:.2}", task_id, feedback);
    }

    /// 获取执行指标
    pub async fn get_metrics(&self, task_id: &str) -> Option<ExecutionMetrics> {
        let executions = self.executions.read().await;
        executions.get(task_id).map(|state| state.metrics.clone())
    }

    /// 获取全局指标
    pub async fn get_global_metrics(&self) -> GlobalMetricsView {
        let global = self.global_metrics.read().await;
        GlobalMetricsView {
            total_executions: global.total_executions,
            successful_executions: global.successful_executions,
            failed_executions: global.failed_executions,
            avg_confidence: global.avg_confidence,
            avg_duration_ms: global.avg_duration_ms,
            avg_feedback: if global.feedback_history.is_empty() {
                0.0
            } else {
                let sum: f32 = global.feedback_history.iter().sum();
                sum / global.feedback_history.len() as f32
            },
        }
    }

    /// 计算执行成功率
    pub async fn success_rate(&self) -> f32 {
        let global = self.global_metrics.read().await;
        if global.total_executions == 0 {
            0.0
        } else {
            global.successful_executions as f32 / global.total_executions as f32
        }
    }

    /// 计算平均反馈分数
    pub async fn avg_feedback(&self) -> f32 {
        let global = self.global_metrics.read().await;
        if global.feedback_history.is_empty() {
            0.0
        } else {
            let sum: f32 = global.feedback_history.iter().sum();
            sum / global.feedback_history.len() as f32
        }
    }
}

/// 全局指标视图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalMetricsView {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub avg_confidence: f32,
    pub avg_duration_ms: f64,
    pub avg_feedback: f32,
}

// 导入 ExecutionResult 类型
use crate::cognitive::meta_controller::ExecutionResult;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_monitor_creation() {
        let monitor = Monitor::new(100);
        assert_eq!(monitor.max_history, 100);
    }

    #[tokio::test]
    async fn test_start_and_finish_execution() {
        let monitor = Monitor::new(100);
        monitor.start_execution("test-1");
        
        let result = ExecutionResult {
            task_id: "test-1".into(),
            success: true,
            output: "Test".into(),
            confidence: 0.9,
            duration: Duration::from_millis(100),
            metrics: ExecutionMetrics::default(),
        };
        
        monitor.finish_execution("test-1", &result);
        
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        let metrics = monitor.get_metrics("test-1").await;
        assert!(metrics.is_some());
    }

    #[tokio::test]
    async fn test_global_metrics() {
        let monitor = Monitor::new(100);
        
        let result = ExecutionResult {
            task_id: "test-1".into(),
            success: true,
            output: "Test".into(),
            confidence: 0.9,
            duration: Duration::from_millis(100),
            metrics: ExecutionMetrics::default(),
        };
        
        monitor.start_execution("test-1");
        monitor.finish_execution("test-1", &result);
        
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        let global = monitor.get_global_metrics().await;
        assert_eq!(global.total_executions, 1);
        assert_eq!(global.successful_executions, 1);
    }
}
