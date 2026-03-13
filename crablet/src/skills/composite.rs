//! 技能组合系统 - 支持多技能协同执行
//!
//! 提供两种组合模式:
//! 1. Sequential - 顺序执行，前一个技能的输出作为后一个的输入
//! 2. Parallel - 并行执行，多个技能同时运行，结果聚合

use std::collections::HashMap;
use std::sync::Arc;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::RwLock;
use tracing::{info, debug, warn, error};

use super::SkillRegistry;

/// 技能组合类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CompositionType {
    /// 顺序执行: A -> B -> C
    Sequential,
    /// 并行执行: A | B | C (同时运行)
    Parallel,
    /// 条件执行: if condition then A else B
    Conditional,
    /// 循环执行: while condition do A
    Loop,
    /// 映射执行: 对集合中的每个元素执行 A
    Map,
    /// 归约执行: 将多个结果归约为一个
    Reduce,
}

/// 技能节点 - 组合中的单个执行单元
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillNode {
    /// 节点ID
    pub id: String,
    /// 引用的技能名称
    pub skill_name: String,
    /// 输入参数映射 (从上下文到技能参数)
    pub input_mapping: HashMap<String, String>,
    /// 输出结果映射 (从技能输出到上下文)
    pub output_mapping: Option<String>,
    /// 执行条件 (可选，用于条件分支)
    pub condition: Option<String>,
    /// 重试配置
    #[serde(default)]
    pub retry_policy: RetryPolicy,
    /// 超时配置 (秒)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    30
}

/// 重试策略
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetryPolicy {
    /// 最大重试次数
    #[serde(default)]
    pub max_retries: u32,
    /// 重试间隔 (毫秒)
    #[serde(default = "default_retry_interval")]
    pub interval_ms: u64,
    /// 退避倍数
    #[serde(default = "default_backoff")]
    pub backoff_multiplier: f64,
}

fn default_retry_interval() -> u64 {
    1000
}

fn default_backoff() -> f64 {
    2.0
}

/// 技能组合定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompositeSkill {
    /// 组合名称
    pub name: String,
    /// 组合描述
    pub description: String,
    /// 版本
    pub version: String,
    /// 组合类型
    pub composition_type: CompositionType,
    /// 技能节点列表
    pub nodes: Vec<SkillNode>,
    /// 输入参数Schema
    pub input_schema: Value,
    /// 输出参数Schema
    pub output_schema: Value,
    /// 全局超时 (秒)
    #[serde(default = "default_global_timeout")]
    pub global_timeout_secs: u64,
    /// 错误处理策略
    #[serde(default)]
    pub error_policy: ErrorPolicy,
    /// 事务配置 (是否支持回滚)
    #[serde(default)]
    pub transactional: bool,
}

fn default_global_timeout() -> u64 {
    300
}

/// 错误处理策略
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum ErrorPolicy {
    /// 遇到错误立即停止
    #[default]
    FailFast,
    /// 忽略错误继续执行
    Continue,
    /// 执行回滚
    Rollback,
    /// 降级到备用技能
    Fallback(String),
}

/// 执行上下文 - 在技能链中传递状态
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// 原始输入
    pub input: Value,
    /// 中间结果存储
    pub variables: HashMap<String, Value>,
    /// 执行历史
    pub execution_log: Vec<ExecutionRecord>,
    /// 当前节点索引
    pub current_node: usize,
    /// 开始时间
    pub start_time: std::time::Instant,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            input: Value::Null,
            variables: HashMap::new(),
            execution_log: Vec::new(),
            current_node: 0,
            start_time: std::time::Instant::now(),
        }
    }
}

/// 执行记录
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionRecord {
    pub node_id: String,
    pub skill_name: String,
    pub input: Value,
    pub output: Option<Value>,
    pub success: bool,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// 执行结果
#[derive(Debug, Clone, Serialize)]
pub struct CompositeResult {
    /// 是否成功
    pub success: bool,
    /// 最终结果
    pub output: Value,
    /// 执行统计
    pub stats: ExecutionStats,
}

/// 执行统计
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionStats {
    pub total_nodes: usize,
    pub executed_nodes: usize,
    pub failed_nodes: usize,
    pub total_duration_ms: u64,
    pub node_durations: HashMap<String, u64>,
}

/// 技能组合执行器
pub struct CompositeExecutor {
    registry: Arc<RwLock<SkillRegistry>>,
}

impl CompositeExecutor {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>) -> Self {
        Self { registry }
    }

    /// 执行技能组合
    pub async fn execute(&self, composite: &CompositeSkill, input: Value) -> Result<CompositeResult> {
        info!("Executing composite skill: {} (type: {:?})", composite.name, composite.composition_type);
        
        let mut context = ExecutionContext {
            input: input.clone(),
            variables: HashMap::new(),
            execution_log: Vec::new(),
            current_node: 0,
            start_time: std::time::Instant::now(),
        };

        // 将输入放入变量上下文
        context.variables.insert("input".to_string(), input);

        let result = match composite.composition_type {
            CompositionType::Sequential => {
                self.execute_sequential(composite, &mut context).await
            }
            CompositionType::Parallel => {
                self.execute_parallel(composite, &mut context).await
            }
            CompositionType::Conditional => {
                self.execute_conditional(composite, &mut context).await
            }
            CompositionType::Loop => {
                self.execute_loop(composite, &mut context).await
            }
            CompositionType::Map => {
                self.execute_map(composite, &mut context).await
            }
            CompositionType::Reduce => {
                self.execute_reduce(composite, &mut context).await
            }
        };

        let total_duration = context.start_time.elapsed().as_millis() as u64;
        
        let stats = ExecutionStats {
            total_nodes: composite.nodes.len(),
            executed_nodes: context.execution_log.len(),
            failed_nodes: context.execution_log.iter().filter(|r| !r.success).count(),
            total_duration_ms: total_duration,
            node_durations: context.execution_log.iter()
                .map(|r| (r.node_id.clone(), r.duration_ms))
                .collect(),
        };

        match result {
            Ok(output) => {
                Ok(CompositeResult {
                    success: true,
                    output,
                    stats,
                })
            }
            Err(_) => {
                if composite.transactional && composite.error_policy == ErrorPolicy::Rollback {
                    self.rollback(&context).await;
                }
                
                Ok(CompositeResult {
                    success: false,
                    output: Value::Null,
                    stats,
                })
            }
        }
    }

    /// 顺序执行
    async fn execute_sequential(&self, composite: &CompositeSkill, context: &mut ExecutionContext) -> Result<Value> {
        let mut last_output = Value::Null;

        for (idx, node) in composite.nodes.iter().enumerate() {
            context.current_node = idx;
            
            match self.execute_node(node, context).await {
                Ok(output) => {
                    last_output = output.clone();
                    // 将输出映射到上下文变量
                    if let Some(ref mapping) = node.output_mapping {
                        context.variables.insert(mapping.clone(), output);
                    }
                }
                Err(e) => {
                    error!("Node {} execution failed: {}", node.id, e);
                    
                    match composite.error_policy {
                        ErrorPolicy::FailFast => return Err(e),
                        ErrorPolicy::Continue => continue,
                        ErrorPolicy::Rollback => {
                            return Err(e);
                        }
                        ErrorPolicy::Fallback(ref fallback_skill) => {
                            // 执行降级技能
                            let fallback_node = SkillNode {
                                id: format!("{}_fallback", node.id),
                                skill_name: fallback_skill.clone(),
                                input_mapping: node.input_mapping.clone(),
                                output_mapping: node.output_mapping.clone(),
                                condition: None,
                                retry_policy: RetryPolicy::default(),
                                timeout_secs: node.timeout_secs,
                            };
                            last_output = self.execute_node(&fallback_node, context).await?;
                        }
                    }
                }
            }
        }

        Ok(last_output)
    }

    /// 并行执行
    async fn execute_parallel(&self, composite: &CompositeSkill, context: &mut ExecutionContext) -> Result<Value> {
        use futures::future::join_all;

        let mut handles = Vec::new();
        let context_arc = Arc::new(RwLock::new(context.clone()));

        for (idx, node) in composite.nodes.iter().enumerate() {
            let node = node.clone();
            let ctx = context_arc.clone();
            let registry = self.registry.clone();
            
            let handle = tokio::spawn(async move {
                let mut ctx_write = ctx.write().await;
                ctx_write.current_node = idx;
                drop(ctx_write);
                
                // 读取上下文执行节点
                let ctx_read = ctx.read().await;
                let node_result = Self::execute_node_with_registry(
                    &node, 
                    &ctx_read,
                    &registry
                ).await;
                
                (node.id.clone(), node_result)
            });
            
            handles.push(handle);
        }

        let results = join_all(handles).await;
        let mut outputs = Vec::new();

        for result in results {
            match result {
                Ok((node_id, Ok(output))) => {
                    outputs.push(json!({
                        "node_id": node_id,
                        "output": output
                    }));
                }
                Ok((node_id, Err(e))) => {
                    warn!("Parallel node {} failed: {}", node_id, e);
                    if composite.error_policy == ErrorPolicy::FailFast {
                        return Err(e);
                    }
                }
                Err(e) => {
                    error!("Task panicked: {}", e);
                    if composite.error_policy == ErrorPolicy::FailFast {
                        return Err(anyhow!("Task panicked: {}", e));
                    }
                }
            }
        }

        Ok(Value::Array(outputs))
    }

    /// 条件执行
    async fn execute_conditional(&self, composite: &CompositeSkill, context: &mut ExecutionContext) -> Result<Value> {
        if composite.nodes.len() < 2 {
            return Err(anyhow!("Conditional composition requires at least 2 nodes"));
        }

        // 第一个节点是条件判断
        let condition_node = &composite.nodes[0];
        let condition_result = self.execute_node(condition_node, context).await?;
        
        // 根据条件结果选择分支
        let condition_met = condition_result.as_bool().unwrap_or(false);
        
        let target_node = if condition_met {
            composite.nodes.get(1) // then 分支
        } else {
            composite.nodes.get(2) // else 分支
        };

        if let Some(node) = target_node {
            self.execute_node(node, context).await
        } else {
            Ok(Value::Null)
        }
    }

    /// 循环执行
    async fn execute_loop(&self, composite: &CompositeSkill, context: &mut ExecutionContext) -> Result<Value> {
        if composite.nodes.is_empty() {
            return Ok(Value::Null);
        }

        let condition_node = &composite.nodes[0];
        let body_node = composite.nodes.get(1);
        
        let mut loop_count = 0;
        let max_iterations = 1000; // 防止无限循环

        loop {
            if loop_count >= max_iterations {
                return Err(anyhow!("Loop exceeded maximum iterations"));
            }

            // 检查条件
            let condition_result = self.execute_node(condition_node, context).await?;
            if !condition_result.as_bool().unwrap_or(false) {
                break;
            }

            // 执行循环体
            if let Some(node) = body_node {
                match self.execute_node(node, context).await {
                    Ok(output) => {
                        context.variables.insert(format!("loop_{}", loop_count), output);
                    }
                    Err(e) => {
                        if composite.error_policy == ErrorPolicy::FailFast {
                            return Err(e);
                        }
                    }
                }
            }

            loop_count += 1;
        }

        Ok(json!({ "iterations": loop_count }))
    }

    /// 映射执行 - 对集合中的每个元素执行技能
    async fn execute_map(&self, composite: &CompositeSkill, context: &mut ExecutionContext) -> Result<Value> {
        if composite.nodes.is_empty() {
            return Ok(Value::Null);
        }

        let node = &composite.nodes[0];
        
        // 从上下文中获取输入集合
        let input_array = context.input.as_array()
            .ok_or_else(|| anyhow!("Map composition requires input to be an array"))?;

        let mut results = Vec::new();

        for (idx, item) in input_array.iter().enumerate() {
            // 为每个元素创建临时上下文
            let mut item_context = context.clone();
            item_context.variables.insert("item".to_string(), item.clone());
            item_context.variables.insert("index".to_string(), json!(idx));

            match self.execute_node_with_context(node, &item_context).await {
                Ok(output) => results.push(output),
                Err(e) => {
                    if composite.error_policy == ErrorPolicy::FailFast {
                        return Err(e);
                    }
                    results.push(Value::Null);
                }
            }
        }

        Ok(Value::Array(results))
    }

    /// 归约执行 - 将多个结果归约为一个
    async fn execute_reduce(&self, composite: &CompositeSkill, context: &mut ExecutionContext) -> Result<Value> {
        if composite.nodes.len() < 2 {
            return Err(anyhow!("Reduce composition requires at least 2 nodes: mapper and reducer"));
        }

        let mapper_node = &composite.nodes[0];
        let reducer_node = &composite.nodes[1];

        // 首先执行映射
        let input_array = context.input.as_array()
            .ok_or_else(|| anyhow!("Map composition requires input to be an array"))?;

        let mut mapped_results = Vec::new();
        for (idx, item) in input_array.iter().enumerate() {
            let mut item_context = context.clone();
            item_context.variables.insert("item".to_string(), item.clone());
            item_context.variables.insert("index".to_string(), json!(idx));

            match self.execute_node_with_context(mapper_node, &item_context).await {
                Ok(output) => mapped_results.push(output),
                Err(e) => {
                    if composite.error_policy == ErrorPolicy::FailFast {
                        return Err(e);
                    }
                    mapped_results.push(Value::Null);
                }
            }
        }

        // 然后执行归约
        let mut accumulator = json!(null);
        for item in mapped_results {
            let mut reduce_context = context.clone();
            reduce_context.variables.insert("accumulator".to_string(), accumulator);
            reduce_context.variables.insert("current".to_string(), item);

            accumulator = self.execute_node_with_context(reducer_node, &reduce_context).await?;
        }

        Ok(accumulator)
    }

    /// 执行单个节点
    async fn execute_node(&self, node: &SkillNode, context: &ExecutionContext) -> Result<Value> {
        Self::execute_node_with_registry(node, context, &self.registry).await
    }

    /// 使用指定的注册表执行节点
    async fn execute_node_with_registry(
        node: &SkillNode,
        context: &ExecutionContext,
        registry: &Arc<RwLock<SkillRegistry>>
    ) -> Result<Value> {
        let _start = std::time::Instant::now();
        
        // 构建技能输入参数
        let skill_args = Self::build_skill_args(node, context)?;
        
        // 获取技能
        let reg = registry.read().await;
        
        // 执行技能 (带重试逻辑)
        let mut last_error = None;
        for attempt in 0..=node.retry_policy.max_retries {
            if attempt > 0 {
                let delay = node.retry_policy.interval_ms as f64 
                    * node.retry_policy.backoff_multiplier.powi(attempt as i32 - 1);
                tokio::time::sleep(tokio::time::Duration::from_millis(delay as u64)).await;
                debug!("Retrying node {} (attempt {})", node.id, attempt);
            }

            match tokio::time::timeout(
                tokio::time::Duration::from_secs(node.timeout_secs),
                reg.execute(&node.skill_name, skill_args.clone())
            ).await {
                Ok(Ok(result)) => {
                    // 尝试解析结果为 JSON
                    let output = serde_json::from_str(&result).unwrap_or_else(|_| json!(result));
                    return Ok(output);
                }
                Ok(Err(e)) => {
                    last_error = Some(e);
                }
                Err(_) => {
                    last_error = Some(anyhow!("Timeout after {}s", node.timeout_secs));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("All retries failed")))
    }

    /// 使用上下文执行节点
    async fn execute_node_with_context(&self, node: &SkillNode, context: &ExecutionContext) -> Result<Value> {
        Self::execute_node_with_registry(node, context, &self.registry).await
    }

    /// 构建技能参数
    fn build_skill_args(node: &SkillNode, context: &ExecutionContext) -> Result<Value> {
        let mut args = serde_json::Map::new();

        for (param_name, var_path) in &node.input_mapping {
            let value = Self::resolve_variable(context, var_path)?;
            args.insert(param_name.clone(), value);
        }

        Ok(Value::Object(args))
    }

    /// 解析变量路径
    fn resolve_variable(context: &ExecutionContext, path: &str) -> Result<Value> {
        // 支持简单的路径解析: input.name, variables.result, etc.
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

        // 处理嵌套路径
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

    /// 回滚执行
    async fn rollback(&self, context: &ExecutionContext) {
        warn!("Rolling back composite execution");
        
        // 逆序遍历执行记录，调用回滚
        for record in context.execution_log.iter().rev() {
            if record.success {
                debug!("Rolling back node: {}", record.node_id);
                // TODO: 实现具体的回滚逻辑
            }
        }
    }
}

/// 组合技能注册表
pub struct CompositeRegistry {
    composites: HashMap<String, CompositeSkill>,
}

impl CompositeRegistry {
    pub fn new() -> Self {
        Self {
            composites: HashMap::new(),
        }
    }

    pub fn register(&mut self, composite: CompositeSkill) {
        self.composites.insert(composite.name.clone(), composite);
    }

    pub fn get(&self, name: &str) -> Option<&CompositeSkill> {
        self.composites.get(name)
    }

    pub fn unregister(&mut self, name: &str) -> bool {
        self.composites.remove(name).is_some()
    }

    pub fn list(&self) -> Vec<&CompositeSkill> {
        self.composites.values().collect()
    }

    /// 从 YAML/JSON 文件加载组合定义
    pub async fn load_from_file(&mut self, path: &std::path::Path) -> Result<()> {
        let content = tokio::fs::read_to_string(path).await?;
        
        let composite: CompositeSkill = if path.extension().and_then(|s| s.to_str()) == Some("json") {
            serde_json::from_str(&content)?
        } else {
            serde_yaml::from_str(&content)?
        };

        self.register(composite);
        Ok(())
    }
}

impl Default for CompositeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_composite_skill_serialization() {
        let composite = CompositeSkill {
            name: "test_pipeline".to_string(),
            description: "Test sequential pipeline".to_string(),
            version: "1.0.0".to_string(),
            composition_type: CompositionType::Sequential,
            nodes: vec![
                SkillNode {
                    id: "step1".to_string(),
                    skill_name: "extract_data".to_string(),
                    input_mapping: [("url".to_string(), "input.url".to_string())].into(),
                    output_mapping: Some("extracted".to_string()),
                    condition: None,
                    retry_policy: RetryPolicy::default(),
                    timeout_secs: 30,
                },
                SkillNode {
                    id: "step2".to_string(),
                    skill_name: "transform".to_string(),
                    input_mapping: [("data".to_string(), "variables.extracted".to_string())].into(),
                    output_mapping: Some("result".to_string()),
                    condition: None,
                    retry_policy: RetryPolicy::default(),
                    timeout_secs: 30,
                },
            ],
            input_schema: json!({"type": "object"}),
            output_schema: json!({"type": "object"}),
            global_timeout_secs: 120,
            error_policy: ErrorPolicy::FailFast,
            transactional: false,
        };

        let yaml = serde_yaml::to_string(&composite).unwrap();
        assert!(yaml.contains("test_pipeline"));
        assert!(yaml.contains("Sequential"));
    }
}
