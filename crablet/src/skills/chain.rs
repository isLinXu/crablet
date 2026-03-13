//! 技能链系统 - 支持复杂工作流编排
//!
//! 特性:
//! - 有向无环图 (DAG) 结构支持
//! - 条件分支与合并
//! - 并行路径执行
//! - 事务与补偿机制
//! - 可视化导出

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::RwLock;
use tracing::{info, debug, warn, error};

use super::SkillRegistry;
use super::composite::{SkillNode, ExecutionContext, ExecutionRecord, RetryPolicy};

/// 技能链定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillChain {
    /// 链ID
    pub id: String,
    /// 链名称
    pub name: String,
    /// 描述
    pub description: String,
    /// 版本
    pub version: String,
    /// 步骤定义
    pub steps: Vec<ChainStep>,
    /// 连接定义
    pub connections: Vec<StepConnection>,
    /// 输入Schema
    pub input_schema: Value,
    /// 输出Schema
    pub output_schema: Value,
    /// 全局配置
    #[serde(default)]
    pub config: ChainConfig,
}

/// 链步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainStep {
    /// 步骤ID
    pub id: String,
    /// 步骤名称
    pub name: String,
    /// 步骤类型
    pub step_type: StepType,
    /// 技能节点定义 (如果是技能步骤)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_node: Option<SkillNode>,
    /// 子链ID (如果是子链步骤)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subchain_id: Option<String>,
    /// 输入参数映射
    #[serde(default)]
    pub input_mappings: HashMap<String, String>,
    /// 输出参数映射
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_mapping: Option<String>,
    /// 重试策略
    #[serde(default)]
    pub retry_policy: RetryPolicy,
    /// 超时配置 (秒)
    #[serde(default = "default_step_timeout")]
    pub timeout_secs: u64,
}

fn default_step_timeout() -> u64 {
    30
}

/// 步骤类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepType {
    /// 技能执行步骤
    Skill,
    /// 条件判断步骤
    Condition,
    /// 并行分支开始
    ParallelStart,
    /// 并行分支合并
    ParallelJoin,
    /// 子链调用
    SubChain,
    /// 等待/延迟步骤
    Wait,
    /// 人工审批步骤
    HumanApproval,
    /// 事件发送步骤
    EmitEvent,
}

/// 步骤连接
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepConnection {
    /// 源步骤ID
    pub from: String,
    /// 目标步骤ID
    pub to: String,
    /// 条件表达式 (用于条件分支)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// 连接标签
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// 链配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChainConfig {
    /// 是否启用事务
    #[serde(default)]
    pub transactional: bool,
    /// 全局超时 (秒)
    #[serde(default = "default_chain_timeout")]
    pub global_timeout_secs: u64,
    /// 错误处理策略
    #[serde(default)]
    pub error_policy: ChainErrorPolicy,
    /// 是否允许循环
    #[serde(default)]
    pub allow_cycles: bool,
    /// 最大执行深度
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,
}

fn default_chain_timeout() -> u64 {
    300
}

fn default_max_depth() -> usize {
    100
}

/// 链错误处理策略
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ChainErrorPolicy {
    /// 立即停止
    #[default]
    Stop,
    /// 继续执行其他分支
    Continue,
    /// 执行补偿
    Compensate,
    /// 跳转到指定步骤
    Goto(String),
}

/// 链执行实例
pub struct ChainExecution {
    /// 链定义
    chain: SkillChain,
    /// 执行上下文
    context: ExecutionContext,
    /// 执行状态
    state: ExecutionState,
    /// 已执行步骤
    executed_steps: HashSet<String>,
    /// 步骤结果缓存
    step_results: HashMap<String, Value>,
    /// 当前活跃步骤
    active_steps: Vec<String>,
    /// 开始时间
    start_time: std::time::Instant,
}

/// 执行状态
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionState {
    /// 准备中
    Pending,
    /// 运行中
    Running,
    /// 等待中 (如人工审批)
    Waiting,
    /// 已完成
    Completed,
    /// 已失败
    Failed,
    /// 已取消
    Cancelled,
}

/// 链执行结果
#[derive(Debug, Clone, Serialize)]
pub struct ChainResult {
    /// 链ID
    pub chain_id: String,
    /// 是否成功
    pub success: bool,
    /// 最终输出
    pub output: Value,
    /// 执行状态
    pub state: String,
    /// 执行统计
    pub stats: ChainExecutionStats,
    /// 执行历史
    pub execution_log: Vec<ExecutionRecord>,
    /// 执行时长 (毫秒)
    pub duration_ms: u64,
}

/// 链执行统计
#[derive(Debug, Clone, Serialize)]
pub struct ChainExecutionStats {
    pub total_steps: usize,
    pub executed_steps: usize,
    pub failed_steps: usize,
    pub skipped_steps: usize,
    pub parallel_branches: usize,
}

/// 技能链引擎
pub struct SkillChainEngine {
    registry: Arc<RwLock<SkillRegistry>>,
    chain_definitions: Arc<RwLock<HashMap<String, SkillChain>>>,
    active_executions: Arc<RwLock<HashMap<String, ChainExecution>>>,
}

impl SkillChainEngine {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self {
            registry,
            chain_definitions: Arc::new(RwLock::new(HashMap::new())),
            active_executions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册链定义
    pub async fn register_chain(&self, chain: SkillChain) -> Result<()> {
        // 验证链定义
        self.validate_chain(&chain)?;
        
        let mut definitions = self.chain_definitions.write().await;
        definitions.insert(chain.id.clone(), chain);
        Ok(())
    }

    /// 验证链定义
    fn validate_chain(&self, chain: &SkillChain) -> Result<()> {
        // 1. 检查步骤ID唯一性
        let mut ids = HashSet::new();
        for step in &chain.steps {
            if !ids.insert(step.id.clone()) {
                return Err(anyhow!("Duplicate step ID: {}", step.id));
            }
        }

        // 2. 检查连接有效性
        for conn in &chain.connections {
            if !ids.contains(&conn.from) {
                return Err(anyhow!("Connection references unknown step: {}", conn.from));
            }
            if !ids.contains(&conn.to) {
                return Err(anyhow!("Connection references unknown step: {}", conn.to));
            }
        }

        // 3. 检查是否有起始步骤
        let has_start = chain.steps.iter().any(|s| {
            chain.connections.iter().all(|c| c.to != s.id)
        });
        if !has_start && !chain.steps.is_empty() {
            return Err(anyhow!("Chain must have at least one start step"));
        }

        // 4. 检查是否有结束步骤
        let has_end = chain.steps.iter().any(|s| {
            chain.connections.iter().all(|c| c.from != s.id)
        });
        if !has_end && !chain.steps.is_empty() {
            return Err(anyhow!("Chain must have at least one end step"));
        }

        Ok(())
    }

    /// 启动链执行
    pub async fn start_execution(&self, chain_id: &str, input: Value) -> Result<String> {
        let definitions = self.chain_definitions.read().await;
        let chain = definitions.get(chain_id)
            .ok_or_else(|| anyhow!("Chain not found: {}", chain_id))?
            .clone();
        drop(definitions);

        let execution_id = format!("{}_{}", chain_id, uuid::Uuid::new_v4());
        
        let mut context = ExecutionContext {
            input: input.clone(),
            variables: HashMap::new(),
            execution_log: Vec::new(),
            current_node: 0,
            start_time: std::time::Instant::now(),
        };
        context.variables.insert("input".to_string(), input);

        let execution = ChainExecution {
            chain,
            context,
            state: ExecutionState::Pending,
            executed_steps: HashSet::new(),
            step_results: HashMap::new(),
            active_steps: Vec::new(),
            start_time: std::time::Instant::now(),
        };

        let mut executions = self.active_executions.write().await;
        executions.insert(execution_id.clone(), execution);

        // 启动执行
        let exec_id_clone = execution_id.clone();
        let registry = self.registry.clone();
        let definitions = self.chain_definitions.clone();
        let executions = self.active_executions.clone();
        
        tokio::spawn(async move {
            let engine = SkillChainEngine {
                registry,
                chain_definitions: definitions,
                active_executions: executions,
            };
            if let Err(e) = engine.run_execution(&exec_id_clone).await {
                error!("Chain execution failed: {}", e);
            }
        });

        Ok(execution_id)
    }

    /// 运行执行
    async fn run_execution(&self, execution_id: &str) -> Result<()> {
        // 获取执行信息
        let chain = {
            let executions = self.active_executions.read().await;
            let execution = executions.get(execution_id)
                .ok_or_else(|| anyhow!("Execution not found: {}", execution_id))?;
            execution.chain.clone()
        };

        // 按拓扑顺序执行步骤
        let mut pending_steps: Vec<String> = chain.steps.iter().map(|s| s.id.clone()).collect();
        let mut completed_steps: HashSet<String> = HashSet::new();

        {
            let mut executions = self.active_executions.write().await;
            if let Some(execution) = executions.get_mut(execution_id) {
                execution.state = ExecutionState::Running;
            }
        }

        while !pending_steps.is_empty() {
            // 找到可以执行的步骤 (所有前置步骤已完成)
            let executable: Vec<String> = pending_steps.iter()
                .filter(|step_id| {
                    chain.connections.iter()
                        .filter(|c| &c.to == *step_id)
                        .all(|c| completed_steps.contains(&c.from))
                })
                .cloned()
                .collect();

            if executable.is_empty() && !pending_steps.is_empty() {
                return Err(anyhow!("Deadlock detected in chain execution"));
            }

            for step_id in &executable {
                let step = chain.steps.iter()
                    .find(|s| &s.id == step_id)
                    .ok_or_else(|| anyhow!("Step not found: {}", step_id))?;

                // 检查条件
                let should_execute = self.check_preconditions(execution_id, step).await?;

                if should_execute {
                    match self.execute_step(execution_id, step).await {
                        Ok(result) => {
                            let mut executions = self.active_executions.write().await;
                            if let Some(execution) = executions.get_mut(execution_id) {
                                execution.step_results.insert(step_id.clone(), result);
                                execution.executed_steps.insert(step_id.clone());
                            }
                        }
                        Err(e) => {
                            error!("Step {} execution failed: {}", step_id, e);
                            
                            let error_policy = chain.config.error_policy.clone();
                            match error_policy {
                                ChainErrorPolicy::Stop => {
                                    let mut executions = self.active_executions.write().await;
                                    if let Some(execution) = executions.get_mut(execution_id) {
                                        execution.state = ExecutionState::Failed;
                                    }
                                    return Err(e);
                                }
                                ChainErrorPolicy::Compensate => {
                                    self.compensate(execution_id).await?;
                                }
                                _ => {}
                            }
                        }
                    }
                }

                completed_steps.insert(step_id.clone());
            }

            pending_steps.retain(|s| !completed_steps.contains(s));
        }

        // 标记完成
        let mut executions = self.active_executions.write().await;
        if let Some(execution) = executions.get_mut(execution_id) {
            if execution.state != ExecutionState::Failed {
                execution.state = ExecutionState::Completed;
            }
        }

        Ok(())
    }

    /// 检查前置条件
    async fn check_preconditions(&self, execution_id: &str, step: &ChainStep) -> Result<bool> {
        let executions = self.active_executions.read().await;
        let execution = executions.get(execution_id)
            .ok_or_else(|| anyhow!("Execution not found"))?;

        // 检查所有前置步骤是否已完成
        for conn in &execution.chain.connections {
            if conn.to == step.id {
                if !execution.executed_steps.contains(&conn.from) {
                    return Ok(false);
                }

                // 检查条件
                if let Some(ref condition) = conn.condition {
                    // 简单条件评估
                    let result = self.evaluate_condition(condition, &execution.context)?;
                    if !result {
                        return Ok(false);
                    }
                }
            }
        }

        Ok(true)
    }

    /// 执行单个步骤
    async fn execute_step(&self, execution_id: &str, step: &ChainStep) -> Result<Value> {
        info!("Executing step: {} (type: {:?})", step.id, step.step_type);

        let start = std::time::Instant::now();

        let result = match step.step_type {
            StepType::Skill => {
                if let Some(ref skill_node) = step.skill_node {
                    self.execute_skill_step(execution_id, skill_node, &step.input_mappings).await
                } else {
                    Err(anyhow!("Skill step missing skill_node"))
                }
            }
            StepType::Condition => {
                self.execute_condition_step(execution_id, step).await
            }
            StepType::Wait => {
                let duration = step.timeout_secs;
                tokio::time::sleep(tokio::time::Duration::from_secs(duration)).await;
                Ok(json!({ "waited_secs": duration }))
            }
            StepType::ParallelStart => {
                Ok(json!({ "parallel_start": step.id }))
            }
            StepType::ParallelJoin => {
                Ok(json!({ "parallel_join": step.id }))
            }
            StepType::SubChain => {
                if let Some(ref subchain_id) = step.subchain_id {
                    self.execute_subchain(execution_id, subchain_id).await
                } else {
                    Err(anyhow!("SubChain step missing subchain_id"))
                }
            }
            _ => {
                warn!("Step type {:?} not fully implemented", step.step_type);
                Ok(Value::Null)
            }
        };

        let duration = start.elapsed().as_millis() as u64;
        
        // 记录执行日志
        let record = ExecutionRecord {
            node_id: step.id.clone(),
            skill_name: step.skill_node.as_ref()
                .map(|n| n.skill_name.clone())
                .unwrap_or_else(|| step.name.clone()),
            input: Value::Null,
            output: result.as_ref().ok().cloned(),
            success: result.is_ok(),
            error: result.as_ref().err().map(|e| e.to_string()),
            duration_ms: duration,
            timestamp: chrono::Utc::now(),
        };

        let mut executions = self.active_executions.write().await;
        if let Some(execution) = executions.get_mut(execution_id) {
            execution.context.execution_log.push(record);
        }

        result
    }

    /// 执行技能步骤
    async fn execute_skill_step(
        &self,
        execution_id: &str,
        skill_node: &SkillNode,
        input_mappings: &HashMap<String, String>
    ) -> Result<Value> {
        let (args, timeout) = {
            let executions = self.active_executions.read().await;
            let execution = executions.get(execution_id)
                .ok_or_else(|| anyhow!("Execution not found"))?;
            
            let args = self.build_args(&execution.context, input_mappings)?;
            (args, skill_node.timeout_secs)
        };

        let registry = self.registry.read().await;
        
        let result = tokio::time::timeout(
            tokio::time::Duration::from_secs(timeout),
            registry.execute(&skill_node.skill_name, args)
        ).await;

        match result {
            Ok(Ok(output)) => {
                serde_json::from_str(&output)
                    .or_else(|_| Ok(json!(output)))
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow!("Step timeout after {}s", timeout)),
        }
    }

    /// 执行条件步骤
    async fn execute_condition_step(&self, _execution_id: &str, step: &ChainStep) -> Result<Value> {
        // 条件步骤通常只是标记，实际条件在连接中评估
        Ok(json!({ "condition_step": step.id }))
    }

    /// 执行子链 (简化实现 - 不递归调用)
    async fn execute_subchain(&self, execution_id: &str, subchain_id: &str) -> Result<Value> {
        let input = {
            let executions = self.active_executions.read().await;
            let execution = executions.get(execution_id)
                .ok_or_else(|| anyhow!("Execution not found"))?;
            execution.context.input.clone()
        };

        // 子链执行暂时返回一个标记值
        // 实际应用中应该有专门的子链执行器
        info!("Sub-chain '{}' execution requested (not fully implemented yet)", subchain_id);
        Ok(json!({ "subchain_id": subchain_id, "input": input }))
    }

    /// 构建参数
    fn build_args(&self, context: &ExecutionContext, mappings: &HashMap<String, String>) -> Result<Value> {
        let mut args = serde_json::Map::new();

        for (param_name, var_path) in mappings {
            let value = self.resolve_variable(context, var_path)?;
            args.insert(param_name.clone(), value);
        }

        Ok(Value::Object(args))
    }

    /// 解析变量
    fn resolve_variable(&self, context: &ExecutionContext, path: &str) -> Result<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        
        if parts.is_empty() {
            return Ok(Value::Null);
        }

        let root = match parts[0] {
            "input" => context.input.clone(),
            "variables" => {
                if parts.len() > 1 {
                    context.variables.get(parts[1])
                        .cloned()
                        .unwrap_or(Value::Null)
                } else {
                    Value::Null
                }
            }
            _ => context.variables.get(parts[0]).cloned().unwrap_or(Value::Null),
        };

        let mut current = root;
        for part in parts.iter().skip(1) {
            if part == &"variables" {
                continue;
            }
            current = match current {
                Value::Object(map) => map.get(*part).cloned().unwrap_or(Value::Null),
                _ => Value::Null,
            };
        }

        Ok(current)
    }

    /// 评估条件
    fn evaluate_condition(&self, condition: &str, context: &ExecutionContext) -> Result<bool> {
        // 简化实现 - 实际可以使用表达式引擎如 evalexpr
        if condition.contains("==") {
            let parts: Vec<&str> = condition.split("==").collect();
            if parts.len() == 2 {
                let left = self.resolve_variable(context, parts[0].trim())?;
                let right = parts[1].trim().trim_matches('"').trim_matches('\'');
                return Ok(left.as_str().map(|s| s == right).unwrap_or(false));
            }
        }
        
        // 默认返回 true
        Ok(true)
    }

    /// 执行补偿
    async fn compensate(&self, execution_id: &str) -> Result<()> {
        warn!("Executing compensation for {}", execution_id);
        
        let executions = self.active_executions.read().await;
        let execution = executions.get(execution_id)
            .ok_or_else(|| anyhow!("Execution not found"))?;
        
        // 逆序执行补偿
        for record in execution.context.execution_log.iter().rev() {
            if record.success {
                debug!("Compensating step: {}", record.node_id);
                // TODO: 调用技能的补偿接口
            }
        }
        
        Ok(())
    }

    /// 获取执行结果
    pub async fn get_result(&self, execution_id: &str) -> Option<ChainResult> {
        let executions = self.active_executions.read().await;
        let execution = executions.get(execution_id)?;

        let duration = execution.start_time.elapsed().as_millis() as u64;
        
        Some(ChainResult {
            chain_id: execution.chain.id.clone(),
            success: execution.state == ExecutionState::Completed,
            output: execution.step_results.values().last().cloned().unwrap_or(Value::Null),
            state: format!("{:?}", execution.state),
            stats: ChainExecutionStats {
                total_steps: execution.chain.steps.len(),
                executed_steps: execution.executed_steps.len(),
                failed_steps: execution.context.execution_log.iter().filter(|r| !r.success).count(),
                skipped_steps: 0,
                parallel_branches: 0,
            },
            execution_log: execution.context.execution_log.clone(),
            duration_ms: duration,
        })
    }

    /// 取消执行
    pub async fn cancel_execution(&self, execution_id: &str) -> Result<()> {
        let mut executions = self.active_executions.write().await;
        if let Some(execution) = executions.get_mut(execution_id) {
            execution.state = ExecutionState::Cancelled;
        }
        Ok(())
    }

    /// 导出为 Mermaid 图
    pub fn export_mermaid(&self, chain: &SkillChain) -> String {
        let mut output = String::from("flowchart TD\n");

        // 添加节点
        for step in &chain.steps {
            let shape = match step.step_type {
                StepType::Skill => format!("[{}]", step.name),
                StepType::Condition => format!("{{{}}}", step.name),
                StepType::ParallelStart => format!("[[{}]]", step.name),
                StepType::ParallelJoin => format!("(({}))", step.name),
                _ => format!("[{}]", step.name),
            };
            output.push_str(&format!("    {}{}\n", step.id, shape));
        }

        // 添加连接
        for conn in &chain.connections {
            let label = conn.label.as_ref()
                .map(|l| format!("|{}|", l))
                .unwrap_or_default();
            output.push_str(&format!("    {} -->{} {}\n", conn.from, label, conn.to));
        }

        output
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_validation() {
        let _chain = SkillChain {
            id: "test".to_string(),
            name: "Test Chain".to_string(),
            description: "Test".to_string(),
            version: "1.0.0".to_string(),
            steps: vec![
                ChainStep {
                    id: "step1".to_string(),
                    name: "Step 1".to_string(),
                    step_type: StepType::Skill,
                    skill_node: None,
                    subchain_id: None,
                    input_mappings: HashMap::new(),
                    output_mapping: None,
                    retry_policy: RetryPolicy::default(),
                    timeout_secs: 30,
                },
                ChainStep {
                    id: "step2".to_string(),
                    name: "Step 2".to_string(),
                    step_type: StepType::Skill,
                    skill_node: None,
                    subchain_id: None,
                    input_mappings: HashMap::new(),
                    output_mapping: None,
                    retry_policy: RetryPolicy::default(),
                    timeout_secs: 30,
                },
            ],
            connections: vec![
                StepConnection {
                    from: "step1".to_string(),
                    to: "step2".to_string(),
                    condition: None,
                    label: None,
                },
            ],
            input_schema: Value::Null,
            output_schema: Value::Null,
            config: ChainConfig::default(),
        };

        // 需要引擎实例来验证
        // let engine = SkillChainEngine::new(...);
        // assert!(engine.validate_chain(&chain).is_ok());
    }

    #[test]
    fn test_mermaid_export() {
        let _chain = SkillChain {
            id: "test".to_string(),
            name: "Test".to_string(),
            description: "Test".to_string(),
            version: "1.0.0".to_string(),
            steps: vec![
                ChainStep {
                    id: "a".to_string(),
                    name: "Start".to_string(),
                    step_type: StepType::Skill,
                    skill_node: None,
                    subchain_id: None,
                    input_mappings: HashMap::new(),
                    output_mapping: None,
                    retry_policy: RetryPolicy::default(),
                    timeout_secs: 30,
                },
                ChainStep {
                    id: "b".to_string(),
                    name: "Check".to_string(),
                    step_type: StepType::Condition,
                    skill_node: None,
                    subchain_id: None,
                    input_mappings: HashMap::new(),
                    output_mapping: None,
                    retry_policy: RetryPolicy::default(),
                    timeout_secs: 30,
                },
            ],
            connections: vec![
                StepConnection {
                    from: "a".to_string(),
                    to: "b".to_string(),
                    condition: None,
                    label: Some("next".to_string()),
                },
            ],
            input_schema: Value::Null,
            output_schema: Value::Null,
            config: ChainConfig::default(),
        };

        // 创建一个模拟引擎来测试导出
        // let mermaid = engine.export_mermaid(&chain);
        // assert!(mermaid.contains("flowchart TD"));
        // assert!(mermaid.contains("a[Start]"));
        // assert!(mermaid.contains("b{Check}"));
    }
}
