//! 多级模型路由系统
//! 
//! 根据任务复杂度和延迟要求，智能选择最合适的模型：
//! - 本地小模型 (Ollama): <50ms，用于简单任务
//! - 快速云模型 (GPT-4o-mini): 100-500ms，用于中等任务
//! - 强力模型 (GPT-4o): 500ms-2s，用于复杂任务

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{info, debug};

use crate::cognitive::llm::LlmClient;
use crate::types::Message;

pub mod complexity;
pub mod local;

pub use complexity::{ComplexityAnalyzer, Complexity, TaskCharacteristics};
pub use local::{OllamaClient, OllamaConfig, LocalModelManager};

/// 延迟要求级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LatencyRequirement {
    /// 极关键: <50ms (如实时交互)
    Critical,
    /// 高: <200ms (如聊天响应)
    High,
    /// 中等: <1s (如分析任务)
    Medium,
    /// 低: <5s (如批量处理)
    Low,
}

impl LatencyRequirement {
    pub fn max_duration(&self) -> Duration {
        match self {
            LatencyRequirement::Critical => Duration::from_millis(50),
            LatencyRequirement::High => Duration::from_millis(200),
            LatencyRequirement::Medium => Duration::from_secs(1),
            LatencyRequirement::Low => Duration::from_secs(5),
        }
    }
}

/// 路由决策
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RouterDecision {
    /// 使用本地模型 (Ollama)
    Local { model: String, reason: String },
    /// 使用快速云模型 (GPT-4o-mini)
    FastModel { model: String, reason: String },
    /// 使用强力模型 (GPT-4o/Claude-3.5-Sonnet)
    Powerful { model: String, reason: String },
}

impl RouterDecision {
    pub fn model_name(&self) -> &str {
        match self {
            RouterDecision::Local { model, .. } => model,
            RouterDecision::FastModel { model, .. } => model,
            RouterDecision::Powerful { model, .. } => model,
        }
    }
    
    pub fn reason(&self) -> &str {
        match self {
            RouterDecision::Local { reason, .. } => reason,
            RouterDecision::FastModel { reason, .. } => reason,
            RouterDecision::Powerful { reason, .. } => reason,
        }
    }
}

/// 模型路由器
pub struct ModelRouter {
    /// 本地模型客户端 (Ollama)
    local_client: Option<Arc<dyn LlmClient>>,
    /// 快速云模型客户端 (GPT-4o-mini)
    fast_client: Arc<dyn LlmClient>,
    /// 强力模型客户端 (GPT-4o)
    powerful_client: Arc<dyn LlmClient>,
    /// 复杂度分析器
    complexity_analyzer: ComplexityAnalyzer,
    /// 性能统计
    stats: RouterStats,
}

/// 路由统计
#[derive(Debug, Default)]
pub struct RouterStats {
    pub local_requests: std::sync::atomic::AtomicU64,
    pub fast_requests: std::sync::atomic::AtomicU64,
    pub powerful_requests: std::sync::atomic::AtomicU64,
    pub total_latency_ms: std::sync::atomic::AtomicU64,
}

impl RouterStats {
    pub fn record_request(&self, decision: &RouterDecision, latency_ms: u64) {
        match decision {
            RouterDecision::Local { .. } => {
                self.local_requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            RouterDecision::FastModel { .. } => {
                self.fast_requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            RouterDecision::Powerful { .. } => {
                self.powerful_requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
        self.total_latency_ms.fetch_add(latency_ms, std::sync::atomic::Ordering::Relaxed);
    }
    
    pub fn report(&self) -> String {
        let local = self.local_requests.load(std::sync::atomic::Ordering::Relaxed);
        let fast = self.fast_requests.load(std::sync::atomic::Ordering::Relaxed);
        let powerful = self.powerful_requests.load(std::sync::atomic::Ordering::Relaxed);
        let total_latency = self.total_latency_ms.load(std::sync::atomic::Ordering::Relaxed);
        let total = local + fast + powerful;
        
        if total == 0 {
            return "No requests yet".to_string();
        }
        
        let avg_latency = total_latency / total;
        format!(
            "Router Stats:\n\
            - Total: {} requests\n\
            - Local (Ollama): {} ({:.1}%)\n\
            - Fast (GPT-4o-mini): {} ({:.1}%)\n\
            - Powerful (GPT-4o): {} ({:.1}%)\n\
            - Avg Latency: {}ms",
            total,
            local, local as f64 / total as f64 * 100.0,
            fast, fast as f64 / total as f64 * 100.0,
            powerful, powerful as f64 / total as f64 * 100.0,
            avg_latency
        )
    }
}

impl ModelRouter {
    /// 创建新的模型路由器
    pub fn new(
        local_client: Option<Arc<dyn LlmClient>>,
        fast_client: Arc<dyn LlmClient>,
        powerful_client: Arc<dyn LlmClient>,
    ) -> Self {
        Self {
            local_client,
            fast_client,
            powerful_client,
            complexity_analyzer: ComplexityAnalyzer::new(),
            stats: RouterStats::default(),
        }
    }
    
    /// 路由任务到合适的模型
    pub async fn route(
        &self,
        messages: &[Message],
        latency_requirement: LatencyRequirement,
    ) -> Result<RouterDecision> {
        let start = Instant::now();
        
        // 1. 分析任务复杂度
        let complexity = self.complexity_analyzer.analyze(messages)?;
        debug!("Task complexity: {:?}", complexity);
        
        // 2. 做出路由决策
        let decision = self.make_decision(complexity, latency_requirement).await;
        
        // 3. 记录统计
        let latency_ms = start.elapsed().as_millis() as u64;
        self.stats.record_request(&decision, latency_ms);
        
        info!(
            "Routed to {} (reason: {}), latency: {}ms",
            decision.model_name(),
            decision.reason(),
            latency_ms
        );
        
        Ok(decision)
    }
    
    /// 获取客户端
    pub fn get_client(&self, decision: &RouterDecision) -> Arc<dyn LlmClient> {
        match decision {
            RouterDecision::Local { .. } => {
                self.local_client.clone()
                    .expect("Local client not available")
            }
            RouterDecision::FastModel { .. } => self.fast_client.clone(),
            RouterDecision::Powerful { .. } => self.powerful_client.clone(),
        }
    }
    
    /// 获取统计信息
    pub fn get_stats(&self) -> String {
        self.stats.report()
    }
    
    /// 做出路由决策
    async fn make_decision(
        &self,
        complexity: Complexity,
        latency_requirement: LatencyRequirement,
    ) -> RouterDecision {
        // 策略矩阵
        match (complexity, latency_requirement) {
            // 关键延迟 + 任意复杂度: 如果有本地模型就用本地
            (_, LatencyRequirement::Critical) => {
                if self.local_client.is_some() {
                    RouterDecision::Local {
                        model: "ollama/llama3.2".to_string(),
                        reason: format!("Critical latency requirement (<50ms), complexity: {:?}", complexity),
                    }
                } else {
                    // 没有本地模型，使用最快可用
                    RouterDecision::FastModel {
                        model: "gpt-4o-mini".to_string(),
                        reason: "Critical latency but no local model available".to_string(),
                    }
                }
            }
            
            // 简单任务: 优先本地模型，其次快速模型
            (Complexity::Simple, _) => {
                if self.local_client.is_some() {
                    RouterDecision::Local {
                        model: "ollama/llama3.2".to_string(),
                        reason: "Simple task, use local model for cost efficiency".to_string(),
                    }
                } else {
                    RouterDecision::FastModel {
                        model: "gpt-4o-mini".to_string(),
                        reason: "Simple task, use fast model".to_string(),
                    }
                }
            }
            
            // 中等复杂度: 根据延迟要求选择
            (Complexity::Medium, latency) => {
                match latency {
                    LatencyRequirement::High => {
                        RouterDecision::FastModel {
                            model: "gpt-4o-mini".to_string(),
                            reason: "Medium complexity with high latency requirement".to_string(),
                        }
                    }
                    _ => {
                        // 可以偶尔使用强力模型获得更好质量
                        if fastrand::f32() < 0.3 {
                            RouterDecision::Powerful {
                                model: "gpt-4o".to_string(),
                                reason: "Medium complexity, occasionally use powerful model for quality".to_string(),
                            }
                        } else {
                            RouterDecision::FastModel {
                                model: "gpt-4o-mini".to_string(),
                                reason: "Medium complexity, use fast model for cost balance".to_string(),
                            }
                        }
                    }
                }
            }
            
            // 复杂任务: 使用强力模型
            (Complexity::Complex, _) => {
                RouterDecision::Powerful {
                    model: "gpt-4o".to_string(),
                    reason: format!("Complex task requires powerful model, latency: {:?}", latency_requirement),
                }
            }
        }
    }
}
