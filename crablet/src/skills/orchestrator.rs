//! 技能编排器 - 高级工作流编排
//!
//! 提供统一的API来管理和执行技能组合与技能链

use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::{SkillRegistry};
use super::composite::{CompositeExecutor, CompositeRegistry};
use super::chain::{SkillChainEngine, ChainResult};
use super::dsl::{WorkflowDefinition, WorkflowCompiler};
use super::visualization::{ExecutionTracer, GraphExporter, GraphFormat};

/// 编排器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// 最大并发执行数
    pub max_concurrent_executions: usize,
    /// 默认超时 (秒)
    pub default_timeout_secs: u64,
    /// 是否启用追踪
    pub enable_tracing: bool,
    /// 是否启用性能分析
    pub enable_profiling: bool,
    /// 工作流目录
    pub workflows_dir: Option<String>,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_concurrent_executions: 10,
            default_timeout_secs: 300,
            enable_tracing: true,
            enable_profiling: true,
            workflows_dir: Some("./workflows".to_string()),
        }
    }
}

/// 执行请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRequest {
    /// 工作流ID或名称
    pub workflow_id: String,
    /// 输入参数
    pub inputs: Value,
    /// 执行选项
    #[serde(default)]
    pub options: ExecutionOptions,
}

/// 执行选项
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionOptions {
    /// 自定义超时 (秒)
    pub timeout_secs: Option<u64>,
    /// 是否异步执行
    #[serde(default)]
    pub async_execution: bool,
    /// 执行标签
    #[serde(default)]
    pub tags: Vec<String>,
    /// 优先级 (1-10, 10最高)
    pub priority: Option<u8>,
}

/// 执行响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResponse {
    /// 执行ID
    pub execution_id: String,
    /// 工作流ID
    pub workflow_id: String,
    /// 执行状态
    pub status: ExecutionStatus,
    /// 输出结果
    pub output: Option<Value>,
    /// 执行统计
    pub stats: Option<ExecutionStats>,
    /// 错误信息
    pub error: Option<String>,
    /// 执行时长 (毫秒)
    pub duration_ms: Option<u64>,
}

/// 执行状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Timeout,
}

/// 执行统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    pub total_steps: usize,
    pub completed_steps: usize,
    pub failed_steps: usize,
    pub skipped_steps: usize,
    pub step_timings: HashMap<String, u64>,
}

/// 技能编排器
pub struct SkillOrchestrator {
    config: OrchestratorConfig,
    registry: Arc<RwLock<SkillRegistry>>,
    composite_registry: Arc<RwLock<CompositeRegistry>>,
    chain_engine: SkillChainEngine,
    tracer: Arc<RwLock<ExecutionTracer>>,
    active_executions: Arc<RwLock<HashMap<String, ExecutionHandle>>>,
}

/// 执行句柄
struct ExecutionHandle {
    workflow_id: String,
    status: ExecutionStatus,
    start_time: std::time::Instant,
    result: Option<ExecutionResponse>,
}

impl SkillOrchestrator {
    /// 创建新的编排器
    pub fn new(registry: Arc<RwLock<SkillRegistry>>, config: Option<OrchestratorConfig>) -> Self {
        let config = config.unwrap_or_default();
        let chain_engine = SkillChainEngine::new(registry.clone());
        
        Self {
            config: config.clone(),
            registry: registry.clone(),
            composite_registry: Arc::new(RwLock::new(CompositeRegistry::new())),
            chain_engine,
            tracer: Arc::new(RwLock::new(ExecutionTracer::new())),
            active_executions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册工作流
    pub async fn register_workflow(&self, workflow: WorkflowDefinition) -> Result<()> {
        info!("Registering workflow: {}", workflow.name);

        // 编译为技能链
        let chain = WorkflowCompiler::compile_to_chain(&workflow)?;
        
        // 注册到链引擎
        self.chain_engine.register_chain(chain).await?;

        // 同时编译为组合技能并注册
        let composite = WorkflowCompiler::compile_to_composite(&workflow)?;
        let mut registry = self.composite_registry.write().await;
        registry.register(composite);

        info!("Workflow '{}' registered successfully", workflow.name);
        Ok(())
    }

    /// 从文件加载工作流
    pub async fn load_workflow_from_file(&self, path: &std::path::Path) -> Result<()> {
        info!("Loading workflow from: {:?}", path);

        let content = tokio::fs::read_to_string(path).await?;
        
        let workflow = if path.extension().and_then(|s| s.to_str()) == Some("json") {
            WorkflowCompiler::from_json(&content)?
        } else {
            WorkflowCompiler::from_yaml(&content)?
        };

        self.register_workflow(workflow).await
    }

    /// 从目录加载所有工作流
    pub async fn load_workflows_from_dir(&self, dir: &std::path::Path) -> Result<usize> {
        let mut count = 0;
        
        if !dir.exists() {
            warn!("Workflows directory not found: {:?}", dir);
            return Ok(0);
        }

        let mut entries = tokio::fs::read_dir(dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() {
                let ext = path.extension().and_then(|s| s.to_str());
                if ext == Some("yaml") || ext == Some("yml") || ext == Some("json") {
                    match self.load_workflow_from_file(&path).await {
                        Ok(_) => count += 1,
                        Err(e) => warn!("Failed to load workflow from {:?}: {}", path, e),
                    }
                }
            }
        }

        info!("Loaded {} workflows from {:?}", count, dir);
        Ok(count)
    }

    /// 执行工作流
    pub async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionResponse> {
        let execution_id = format!("exec_{}", uuid::Uuid::new_v4());
        
        info!("Starting execution {} for workflow {}", execution_id, request.workflow_id);

        // 创建执行句柄
        {
            let mut executions = self.active_executions.write().await;
            executions.insert(execution_id.clone(), ExecutionHandle {
                workflow_id: request.workflow_id.clone(),
                status: ExecutionStatus::Pending,
                start_time: std::time::Instant::now(),
                result: None,
            });
        }

        // 开始追踪
        let trace_id = if self.config.enable_tracing {
            let mut tracer = self.tracer.write().await;
            Some(tracer.start_trace(&request.workflow_id))
        } else {
            None
        };

        // 执行工作流
        let result = self.run_execution(&execution_id, &request, trace_id.as_deref()).await;

        // 更新执行状态
        let duration = {
            let mut executions = self.active_executions.write().await;
            if let Some(handle) = executions.get_mut(&execution_id) {
                handle.status = match &result {
                    Ok(resp) if resp.status == ExecutionStatus::Completed => ExecutionStatus::Completed,
                    Ok(resp) => resp.status.clone(),
                    Err(_) => ExecutionStatus::Failed,
                };
                handle.result = result.as_ref().ok().cloned();
                handle.start_time.elapsed().as_millis() as u64
            } else {
                0
            }
        };

        // 结束追踪
        if let Some(tid) = trace_id {
            let mut tracer = self.tracer.write().await;
            let _ = tracer.end_trace(&tid);
        }

        match result {
            Ok(mut resp) => {
                resp.duration_ms = Some(duration);
                Ok(resp)
            }
            Err(e) => {
                Ok(ExecutionResponse {
                    execution_id,
                    workflow_id: request.workflow_id,
                    status: ExecutionStatus::Failed,
                    output: None,
                    stats: None,
                    error: Some(e.to_string()),
                    duration_ms: Some(duration),
                })
            }
        }
    }

    /// 运行执行
    async fn run_execution(
        &self,
        execution_id: &str,
        request: &ExecutionRequest,
        trace_id: Option<&str>,
    ) -> Result<ExecutionResponse> {
        // 更新状态为运行中
        {
            let mut executions = self.active_executions.write().await;
            if let Some(handle) = executions.get_mut(execution_id) {
                handle.status = ExecutionStatus::Running;
            }
        }

        // 尝试作为技能链执行
        let chain_result = self.chain_engine.start_execution(
            &request.workflow_id,
            request.inputs.clone()
        ).await;

        match chain_result {
            Ok(chain_exec_id) => {
                // 等待链执行完成
                let timeout = request.options.timeout_secs
                    .unwrap_or(self.config.default_timeout_secs);
                
                let result = tokio::time::timeout(
                    tokio::time::Duration::from_secs(timeout),
                    self.wait_for_chain_completion(&chain_exec_id)
                ).await;

                match result {
                    Ok(Ok(chain_result)) => {
                        Ok(ExecutionResponse {
                            execution_id: execution_id.to_string(),
                            workflow_id: request.workflow_id.clone(),
                            status: if chain_result.success { 
                                ExecutionStatus::Completed 
                            } else { 
                                ExecutionStatus::Failed 
                            },
                            output: Some(chain_result.output),
                            stats: Some(ExecutionStats {
                                total_steps: chain_result.stats.total_steps,
                                completed_steps: chain_result.stats.executed_steps,
                                failed_steps: chain_result.stats.failed_steps,
                                skipped_steps: chain_result.stats.skipped_steps,
                                step_timings: HashMap::new(), // 简化实现
                            }),
                            error: None,
                            duration_ms: Some(chain_result.duration_ms),
                        })
                    }
                    Ok(Err(e)) => Err(e),
                    Err(_) => {
                        // 超时
                        let _ = self.chain_engine.cancel_execution(&chain_exec_id).await;
                        Ok(ExecutionResponse {
                            execution_id: execution_id.to_string(),
                            workflow_id: request.workflow_id.clone(),
                            status: ExecutionStatus::Timeout,
                            output: None,
                            stats: None,
                            error: Some(format!("Execution timeout after {}s", timeout)),
                            duration_ms: Some(timeout * 1000),
                        })
                    }
                }
            }
            Err(_) => {
                // 尝试作为组合技能执行
                self.execute_as_composite(execution_id, request, trace_id).await
            }
        }
    }

    /// 作为组合技能执行
    async fn execute_as_composite(
        &self,
        execution_id: &str,
        request: &ExecutionRequest,
        _trace_id: Option<&str>,
    ) -> Result<ExecutionResponse> {
        let composite = {
            let registry = self.composite_registry.read().await;
            registry.get(&request.workflow_id)
                .ok_or_else(|| anyhow!("Workflow not found: {}", request.workflow_id))?
                .clone()
        };

        let executor = CompositeExecutor::new(self.registry.clone());
        let result = executor.execute(&composite, request.inputs.clone()).await?;

        Ok(ExecutionResponse {
            execution_id: execution_id.to_string(),
            workflow_id: request.workflow_id.clone(),
            status: if result.success { ExecutionStatus::Completed } else { ExecutionStatus::Failed },
            output: Some(result.output),
            stats: Some(ExecutionStats {
                total_steps: result.stats.total_nodes,
                completed_steps: result.stats.executed_nodes,
                failed_steps: result.stats.failed_nodes,
                skipped_steps: 0,
                step_timings: result.stats.node_durations.clone(),
            }),
            error: None,
            duration_ms: Some(result.stats.total_duration_ms),
        })
    }

    /// 等待链执行完成
    async fn wait_for_chain_completion(&self, chain_exec_id: &str) -> Result<ChainResult> {
        loop {
            if let Some(result) = self.chain_engine.get_result(chain_exec_id).await {
                return Ok(result);
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    /// 获取执行状态
    pub async fn get_execution_status(&self, execution_id: &str) -> Option<ExecutionResponse> {
        let executions = self.active_executions.read().await;
        executions.get(execution_id).and_then(|h| h.result.clone())
    }

    /// 取消执行
    pub async fn cancel_execution(&self, execution_id: &str) -> Result<()> {
        info!("Cancelling execution: {}", execution_id);
        
        let mut executions = self.active_executions.write().await;
        if let Some(handle) = executions.get_mut(execution_id) {
            handle.status = ExecutionStatus::Cancelled;
        }

        Ok(())
    }

    /// 列出所有已注册的工作流
    pub async fn list_workflows(&self) -> Vec<String> {
        let registry = self.composite_registry.read().await;
        registry.list().iter().map(|c| c.name.clone()).collect()
    }

    /// 导出工作流可视化
    pub async fn export_visualization(
        &self,
        workflow_id: &str,
        format: GraphFormat,
    ) -> Result<String> {
        // 尝试从组合注册表获取
        let registry = self.composite_registry.read().await;
        if let Some(composite) = registry.get(workflow_id) {
            return GraphExporter::export_composite(composite, format);
        }

        Err(anyhow!("Workflow not found: {}", workflow_id))
    }

    /// 获取执行追踪
    pub async fn get_execution_trace(&self, trace_id: &str) -> Result<String> {
        let tracer = self.tracer.read().await;
        tracer.export_trace(trace_id)
    }

    /// 获取性能报告
    pub async fn get_performance_report(&self, workflow_id: &str) -> Result<String> {
        if !self.config.enable_profiling {
            return Err(anyhow!("Profiling is not enabled"));
        }

        // 收集该工作流的所有追踪
        let _tracer = self.tracer.read().await;
        // 简化实现 - 实际应该过滤特定工作流的追踪
        
        Ok(json!({
            "workflow_id": workflow_id,
            "profiling_enabled": true,
            "message": "Performance report would be generated here"
        }).to_string())
    }

    /// 获取编排器统计
    pub async fn get_stats(&self) -> OrchestratorStats {
        let executions = self.active_executions.read().await;
        
        let total = executions.len();
        let completed = executions.values().filter(|h| h.status == ExecutionStatus::Completed).count();
        let failed = executions.values().filter(|h| h.status == ExecutionStatus::Failed).count();
        let running = executions.values().filter(|h| h.status == ExecutionStatus::Running).count();

        OrchestratorStats {
            total_executions: total,
            completed_executions: completed,
            failed_executions: failed,
            running_executions: running,
            registered_workflows: self.list_workflows().await.len(),
        }
    }
}

/// 编排器统计
#[derive(Debug, Clone, Serialize)]
pub struct OrchestratorStats {
    pub total_executions: usize,
    pub completed_executions: usize,
    pub failed_executions: usize,
    pub running_executions: usize,
    pub registered_workflows: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_creation() {
        let registry = Arc::new(RwLock::new(SkillRegistry::new()));
        let orchestrator = SkillOrchestrator::new(registry, None);
        
        let stats = orchestrator.get_stats().await;
        assert_eq!(stats.total_executions, 0);
        assert_eq!(stats.registered_workflows, 0);
    }

    #[test]
    fn test_execution_request_serialization() {
        let request = ExecutionRequest {
            workflow_id: "test_workflow".to_string(),
            inputs: json!({"key": "value"}),
            options: ExecutionOptions {
                timeout_secs: Some(60),
                async_execution: false,
                tags: vec!["test".to_string()],
                priority: Some(5),
            },
        };

        let json_str = serde_json::to_string(&request).unwrap();
        assert!(json_str.contains("test_workflow"));
    }
}
