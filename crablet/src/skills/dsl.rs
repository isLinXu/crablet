//! 技能编排DSL - 声明式定义复杂工作流
//!
//! 提供人类可读的语法来定义技能链和组合:
//! - YAML/JSON 格式支持
//! - 内联表达式
//! - 模板和参数化
//! - 版本控制友好

use std::collections::HashMap;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::composite::{CompositeSkill, CompositionType, SkillNode, ErrorPolicy, RetryPolicy};
use super::chain::{SkillChain, ChainStep, StepType, StepConnection, ChainConfig, ChainErrorPolicy};

/// DSL 版本
pub const DSL_VERSION: &str = "1.0";

/// 工作流定义 (DSL根结构)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    /// DSL版本
    #[serde(default = "default_dsl_version")]
    pub dsl_version: String,
    /// 工作流名称
    pub name: String,
    /// 描述
    pub description: Option<String>,
    /// 版本
    pub version: String,
    /// 作者
    pub author: Option<String>,
    /// 标签
    #[serde(default)]
    pub tags: Vec<String>,
    /// 输入定义
    #[serde(default)]
    pub inputs: HashMap<String, InputDef>,
    /// 输出定义
    #[serde(default)]
    pub outputs: HashMap<String, OutputDef>,
    /// 变量定义
    #[serde(default)]
    pub variables: HashMap<String, VariableDef>,
    /// 步骤定义
    pub steps: Vec<StepDef>,
    /// 配置
    #[serde(default)]
    pub config: WorkflowConfig,
}

fn default_dsl_version() -> String {
    DSL_VERSION.to_string()
}

/// 输入定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDef {
    /// 类型
    #[serde(rename = "type")]
    pub input_type: String,
    /// 描述
    pub description: Option<String>,
    /// 是否必需
    #[serde(default)]
    pub required: bool,
    /// 默认值
    pub default: Option<Value>,
    /// 验证规则
    pub validation: Option<ValidationRule>,
}

/// 输出定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDef {
    /// 类型
    #[serde(rename = "type")]
    pub output_type: String,
    /// 描述
    pub description: Option<String>,
    /// 来源 (表达式)
    pub source: String,
}

/// 变量定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDef {
    /// 类型
    #[serde(rename = "type")]
    pub var_type: String,
    /// 初始值
    pub value: Option<Value>,
    /// 描述
    pub description: Option<String>,
}

/// 验证规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    /// 最小值/长度
    pub min: Option<f64>,
    /// 最大值/长度
    pub max: Option<f64>,
    /// 正则模式
    pub pattern: Option<String>,
    /// 枚举值
    pub enum_values: Option<Vec<Value>>,
}

/// 步骤定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepDef {
    /// 步骤ID
    pub id: String,
    /// 步骤名称
    pub name: Option<String>,
    /// 步骤类型
    #[serde(rename = "type", default = "default_step_type")]
    pub step_type: String,
    /// 技能名称 (如果是技能步骤)
    pub skill: Option<String>,
    /// 子工作流名称 (如果是子流程)
    pub workflow: Option<String>,
    /// 条件表达式
    pub condition: Option<String>,
    /// 输入映射
    #[serde(default)]
    pub inputs: HashMap<String, String>,
    /// 输出映射
    pub output: Option<String>,
    /// 重试配置
    #[serde(default)]
    pub retry: RetryConfig,
    /// 超时
    pub timeout: Option<u64>,
    /// 错误处理
    #[serde(default)]
    pub on_error: ErrorHandler,
    /// 并行配置
    pub parallel: Option<ParallelConfig>,
    /// 循环配置
    pub loop_config: Option<LoopConfig>,
    /// 下一步 (简单顺序)
    pub next: Option<String>,
    /// 分支 (条件分支)
    pub branches: Option<Vec<BranchDef>>,
}

fn default_step_type() -> String {
    "skill".to_string()
}

/// 重试配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetryConfig {
    /// 最大重试次数
    #[serde(default)]
    pub count: u32,
    /// 间隔 (秒)
    #[serde(default = "default_retry_interval_secs")]
    pub interval: u64,
    /// 指数退避
    #[serde(default)]
    pub exponential: bool,
}

fn default_retry_interval_secs() -> u64 {
    1
}

/// 错误处理
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ErrorHandler {
    /// 策略: fail, continue, retry, fallback
    #[serde(default = "default_error_strategy")]
    pub strategy: String,
    /// 降级技能
    pub fallback: Option<String>,
    /// 跳转步骤
    pub goto: Option<String>,
}

fn default_error_strategy() -> String {
    "fail".to_string()
}

/// 并行配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelConfig {
    /// 分支步骤ID列表
    pub branches: Vec<String>,
    /// 聚合策略: all, any, race
    #[serde(default = "default_aggregate")]
    pub aggregate: String,
}

fn default_aggregate() -> String {
    "all".to_string()
}

/// 循环配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopConfig {
    /// 循环条件
    pub condition: String,
    /// 最大迭代次数
    pub max_iterations: Option<usize>,
    /// 迭代变量名
    pub iterator: Option<String>,
    /// 集合表达式 (用于for-each)
    pub collection: Option<String>,
}

/// 分支定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchDef {
    /// 分支名称
    pub name: String,
    /// 条件表达式
    pub condition: String,
    /// 目标步骤
    pub target: String,
}

/// 工作流配置
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowConfig {
    /// 是否启用事务
    #[serde(default)]
    pub transactional: bool,
    /// 全局超时 (秒)
    pub timeout: Option<u64>,
    /// 并发限制
    pub concurrency: Option<usize>,
    /// 是否允许循环
    #[serde(default)]
    pub allow_cycles: bool,
}

/// DSL 编译器
pub struct WorkflowCompiler;

impl WorkflowCompiler {
    /// 从 YAML 编译工作流
    pub fn from_yaml(yaml_str: &str) -> Result<WorkflowDefinition> {
        let workflow: WorkflowDefinition = serde_yaml::from_str(yaml_str)
            .map_err(|e| anyhow!("Failed to parse workflow YAML: {}", e))?;
        
        Self::validate(&workflow)?;
        Ok(workflow)
    }

    /// 从 JSON 编译工作流
    pub fn from_json(json_str: &str) -> Result<WorkflowDefinition> {
        let workflow: WorkflowDefinition = serde_json::from_str(json_str)
            .map_err(|e| anyhow!("Failed to parse workflow JSON: {}", e))?;
        
        Self::validate(&workflow)?;
        Ok(workflow)
    }

    /// 验证工作流定义
    fn validate(workflow: &WorkflowDefinition) -> Result<()> {
        // 1. 检查步骤ID唯一性
        let mut ids = std::collections::HashSet::new();
        for step in &workflow.steps {
            if !ids.insert(&step.id) {
                return Err(anyhow!("Duplicate step ID: {}", step.id));
            }
        }

        // 2. 检查引用的步骤存在
        for step in &workflow.steps {
            if let Some(ref next) = step.next {
                if !ids.contains(next) {
                    return Err(anyhow!("Step '{}' references unknown step: {}", step.id, next));
                }
            }

            if let Some(ref branches) = step.branches {
                for branch in branches {
                    if !ids.contains(&branch.target) {
                        return Err(anyhow!("Branch references unknown step: {}", branch.target));
                    }
                }
            }
        }

        // 3. 检查输入引用有效性
        for step in &workflow.steps {
            for (_key, expr) in &step.inputs {
                Self::validate_expression(expr, workflow)?;
            }
        }

        Ok(())
    }

    /// 验证表达式
    fn validate_expression(expr: &str, workflow: &WorkflowDefinition) -> Result<()> {
        // 简单验证: 检查变量引用
        if expr.starts_with("$") {
            let var_name = &expr[1..];
            if !workflow.inputs.contains_key(var_name) 
                && !workflow.variables.contains_key(var_name) 
                && var_name != "input" {
                // 可能是步骤输出，不做严格检查
            }
        }
        Ok(())
    }

    /// 编译为 CompositeSkill
    pub fn compile_to_composite(workflow: &WorkflowDefinition) -> Result<CompositeSkill> {
        let composition_type = Self::detect_composition_type(workflow)?;
        
        let nodes: Vec<SkillNode> = workflow.steps.iter()
            .filter(|s| s.step_type == "skill")
            .map(|step| SkillNode {
                id: step.id.clone(),
                skill_name: step.skill.clone()
                    .unwrap_or_else(|| "default".to_string()),
                input_mapping: step.inputs.clone(),
                output_mapping: step.output.clone(),
                condition: step.condition.clone(),
                retry_policy: RetryPolicy {
                    max_retries: step.retry.count,
                    interval_ms: step.retry.interval * 1000,
                    backoff_multiplier: if step.retry.exponential { 2.0 } else { 1.0 },
                },
                timeout_secs: step.timeout.unwrap_or(30),
            })
            .collect();

        let error_policy = if workflow.steps.iter().any(|s| s.on_error.strategy == "continue") {
            ErrorPolicy::Continue
        } else {
            ErrorPolicy::FailFast
        };

        Ok(CompositeSkill {
            name: workflow.name.clone(),
            description: workflow.description.clone().unwrap_or_default(),
            version: workflow.version.clone(),
            composition_type,
            nodes,
            input_schema: Self::build_input_schema(workflow),
            output_schema: Self::build_output_schema(workflow),
            global_timeout_secs: workflow.config.timeout.unwrap_or(300),
            error_policy,
            transactional: workflow.config.transactional,
        })
    }

    /// 编译为 SkillChain
    pub fn compile_to_chain(workflow: &WorkflowDefinition) -> Result<SkillChain> {
        let mut steps: Vec<ChainStep> = Vec::new();
        let mut connections: Vec<StepConnection> = Vec::new();

        for (idx, step_def) in workflow.steps.iter().enumerate() {
            let step_type = match step_def.step_type.as_str() {
                "skill" => StepType::Skill,
                "condition" => StepType::Condition,
                "parallel" => StepType::ParallelStart,
                "wait" => StepType::Wait,
                "subworkflow" => StepType::SubChain,
                _ => StepType::Skill,
            };

            let skill_node = if step_type == StepType::Skill {
                step_def.skill.as_ref().map(|skill_name| SkillNode {
                    id: step_def.id.clone(),
                    skill_name: skill_name.clone(),
                    input_mapping: step_def.inputs.clone(),
                    output_mapping: step_def.output.clone(),
                    condition: step_def.condition.clone(),
                    retry_policy: RetryPolicy {
                        max_retries: step_def.retry.count,
                        interval_ms: step_def.retry.interval * 1000,
                        backoff_multiplier: if step_def.retry.exponential { 2.0 } else { 1.0 },
                    },
                    timeout_secs: step_def.timeout.unwrap_or(30),
                })
            } else {
                None
            };

            steps.push(ChainStep {
                id: step_def.id.clone(),
                name: step_def.name.clone().unwrap_or_else(|| step_def.id.clone()),
                step_type,
                skill_node,
                subchain_id: step_def.workflow.clone(),
                input_mappings: step_def.inputs.clone(),
                output_mapping: step_def.output.clone(),
                retry_policy: RetryPolicy {
                    max_retries: step_def.retry.count,
                    interval_ms: step_def.retry.interval * 1000,
                    backoff_multiplier: if step_def.retry.exponential { 2.0 } else { 1.0 },
                },
                timeout_secs: step_def.timeout.unwrap_or(30),
            });

            // 处理连接
            if let Some(ref next_id) = step_def.next {
                connections.push(StepConnection {
                    from: step_def.id.clone(),
                    to: next_id.clone(),
                    condition: step_def.condition.clone(),
                    label: None,
                });
            }

            // 处理分支
            if let Some(ref branches) = step_def.branches {
                for branch in branches {
                    connections.push(StepConnection {
                        from: step_def.id.clone(),
                        to: branch.target.clone(),
                        condition: Some(branch.condition.clone()),
                        label: Some(branch.name.clone()),
                    });
                }
            }

            // 处理并行
            if let Some(ref parallel) = step_def.parallel {
                for branch_id in &parallel.branches {
                    connections.push(StepConnection {
                        from: step_def.id.clone(),
                        to: branch_id.clone(),
                        condition: None,
                        label: None,
                    });
                }
            }

            // 如果没有显式连接，自动连接下一步
            if step_def.next.is_none() && step_def.branches.is_none() && step_def.parallel.is_none() {
                if let Some(next_step) = workflow.steps.get(idx + 1) {
                    connections.push(StepConnection {
                        from: step_def.id.clone(),
                        to: next_step.id.clone(),
                        condition: step_def.condition.clone(),
                        label: None,
                    });
                }
            }
        }

        let error_policy = if workflow.steps.iter().any(|s| s.on_error.strategy == "continue") {
            ChainErrorPolicy::Continue
        } else if workflow.steps.iter().any(|s| s.on_error.strategy == "compensate") {
            ChainErrorPolicy::Compensate
        } else {
            ChainErrorPolicy::Stop
        };

        Ok(SkillChain {
            id: format!("{}_{}", workflow.name, workflow.version),
            name: workflow.name.clone(),
            description: workflow.description.clone().unwrap_or_default(),
            version: workflow.version.clone(),
            steps,
            connections,
            input_schema: Self::build_input_schema(workflow),
            output_schema: Self::build_output_schema(workflow),
            config: ChainConfig {
                transactional: workflow.config.transactional,
                global_timeout_secs: workflow.config.timeout.unwrap_or(300),
                error_policy,
                allow_cycles: workflow.config.allow_cycles,
                max_depth: 100,
            },
        })
    }

    /// 检测组合类型
    fn detect_composition_type(workflow: &WorkflowDefinition) -> Result<CompositionType> {
        // 检查是否有并行步骤
        let has_parallel = workflow.steps.iter().any(|s| s.step_type == "parallel");
        // 检查是否有条件步骤
        let has_condition = workflow.steps.iter().any(|s| s.condition.is_some() || s.branches.is_some());
        // 检查是否有循环
        let has_loop = workflow.steps.iter().any(|s| s.loop_config.is_some());
        // 检查是否有map
        let has_map = workflow.steps.iter().any(|s| s.loop_config.as_ref().map(|l| l.collection.is_some()).unwrap_or(false));

        if has_map {
            Ok(CompositionType::Map)
        } else if has_loop {
            Ok(CompositionType::Loop)
        } else if has_parallel {
            Ok(CompositionType::Parallel)
        } else if has_condition {
            Ok(CompositionType::Conditional)
        } else {
            Ok(CompositionType::Sequential)
        }
    }

    /// 构建输入Schema
    fn build_input_schema(workflow: &WorkflowDefinition) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for (name, def) in &workflow.inputs {
            let mut prop = serde_json::Map::new();
            prop.insert("type".to_string(), json!(def.input_type));
            if let Some(ref desc) = def.description {
                prop.insert("description".to_string(), json!(desc));
            }
            if let Some(ref validation) = def.validation {
                if let Some(min) = validation.min {
                    prop.insert("minimum".to_string(), json!(min));
                }
                if let Some(max) = validation.max {
                    prop.insert("maximum".to_string(), json!(max));
                }
            }
            properties.insert(name.clone(), Value::Object(prop));

            if def.required {
                required.push(name.clone());
            }
        }

        json!({
            "type": "object",
            "properties": properties,
            "required": required
        })
    }

    /// 构建输出Schema
    fn build_output_schema(workflow: &WorkflowDefinition) -> Value {
        let mut properties = serde_json::Map::new();

        for (name, def) in &workflow.outputs {
            let mut prop = serde_json::Map::new();
            prop.insert("type".to_string(), json!(def.output_type));
            if let Some(ref desc) = def.description {
                prop.insert("description".to_string(), json!(desc));
            }
            properties.insert(name.clone(), Value::Object(prop));
        }

        json!({
            "type": "object",
            "properties": properties
        })
    }

    /// 导出为 YAML
    pub fn to_yaml(workflow: &WorkflowDefinition) -> Result<String> {
        serde_yaml::to_string(workflow)
            .map_err(|e| anyhow!("Failed to serialize workflow: {}", e))
    }

    /// 导出为 JSON
    pub fn to_json(workflow: &WorkflowDefinition) -> Result<String> {
        serde_json::to_string_pretty(workflow)
            .map_err(|e| anyhow!("Failed to serialize workflow: {}", e))
    }
}

/// 工作流模板引擎
pub struct WorkflowTemplateEngine;

impl WorkflowTemplateEngine {
    /// 渲染模板
    pub fn render(template: &str, params: &HashMap<String, Value>) -> Result<String> {
        let mut result = template.to_string();
        
        for (key, value) in params {
            let placeholder = format!("{{{{{}}}}}", key);
            let replacement = value.as_str()
                .map(|s| s.to_string())
                .unwrap_or_else(|| value.to_string());
            result = result.replace(&placeholder, &replacement);
        }
        
        Ok(result)
    }

    /// 参数化工作流
    pub fn parameterize(workflow: &mut WorkflowDefinition, params: &HashMap<String, Value>) -> Result<()> {
        // 替换名称
        workflow.name = Self::render(&workflow.name, params)?;
        
        // 替换描述
        if let Some(ref mut desc) = workflow.description {
            *desc = Self::render(desc, params)?;
        }

        // 替换步骤中的字符串值
        for step in &mut workflow.steps {
            if let Some(ref mut skill) = step.skill {
                *skill = Self::render(skill, params)?;
            }
            
            for (_, expr) in &mut step.inputs {
                *expr = Self::render(expr, params)?;
            }
        }

        Ok(())
    }
}

/// 预定义模板
pub mod templates {
    use super::*;

    /// 顺序处理模板
    pub fn sequential_pipeline(name: &str, skills: Vec<&str>) -> WorkflowDefinition {
        let steps: Vec<StepDef> = skills.iter().enumerate().map(|(idx, skill)| {
            StepDef {
                id: format!("step_{}", idx),
                name: Some(format!("Step {}", idx)),
                step_type: "skill".to_string(),
                skill: Some(skill.to_string()),
                workflow: None,
                condition: None,
                inputs: HashMap::new(),
                output: Some(format!("result_{}", idx)),
                retry: RetryConfig::default(),
                timeout: None,
                on_error: ErrorHandler::default(),
                parallel: None,
                loop_config: None,
                next: None,
                branches: None,
            }
        }).collect();

        WorkflowDefinition {
            dsl_version: DSL_VERSION.to_string(),
            name: name.to_string(),
            description: Some(format!("Sequential pipeline with {} steps", skills.len())),
            version: "1.0.0".to_string(),
            author: None,
            tags: vec!["pipeline".to_string()],
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            variables: HashMap::new(),
            steps,
            config: WorkflowConfig::default(),
        }
    }

    /// 条件分支模板
    pub fn conditional_workflow(name: &str, condition: &str, then_skill: &str, else_skill: &str) -> WorkflowDefinition {
        WorkflowDefinition {
            dsl_version: DSL_VERSION.to_string(),
            name: name.to_string(),
            description: Some("Conditional workflow".to_string()),
            version: "1.0.0".to_string(),
            author: None,
            tags: vec!["conditional".to_string()],
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            variables: HashMap::new(),
            steps: vec![
                StepDef {
                    id: "check".to_string(),
                    name: Some("Check Condition".to_string()),
                    step_type: "condition".to_string(),
                    skill: None,
                    workflow: None,
                    condition: Some(condition.to_string()),
                    inputs: HashMap::new(),
                    output: None,
                    retry: RetryConfig::default(),
                    timeout: None,
                    on_error: ErrorHandler::default(),
                    parallel: None,
                    loop_config: None,
                    next: None,
                    branches: Some(vec![
                        BranchDef {
                            name: "then".to_string(),
                            condition: format!("{} == true", condition),
                            target: "then_step".to_string(),
                        },
                        BranchDef {
                            name: "else".to_string(),
                            condition: format!("{} == false", condition),
                            target: "else_step".to_string(),
                        },
                    ]),
                },
                StepDef {
                    id: "then_step".to_string(),
                    name: Some("Then".to_string()),
                    step_type: "skill".to_string(),
                    skill: Some(then_skill.to_string()),
                    workflow: None,
                    condition: None,
                    inputs: HashMap::new(),
                    output: Some("then_result".to_string()),
                    retry: RetryConfig::default(),
                    timeout: None,
                    on_error: ErrorHandler::default(),
                    parallel: None,
                    loop_config: None,
                    next: None,
                    branches: None,
                },
                StepDef {
                    id: "else_step".to_string(),
                    name: Some("Else".to_string()),
                    step_type: "skill".to_string(),
                    skill: Some(else_skill.to_string()),
                    workflow: None,
                    condition: None,
                    inputs: HashMap::new(),
                    output: Some("else_result".to_string()),
                    retry: RetryConfig::default(),
                    timeout: None,
                    on_error: ErrorHandler::default(),
                    parallel: None,
                    loop_config: None,
                    next: None,
                    branches: None,
                },
            ],
            config: WorkflowConfig::default(),
        }
    }

    /// 并行处理模板
    pub fn parallel_workflow(name: &str, branches: Vec<&str>) -> WorkflowDefinition {
        WorkflowDefinition {
            dsl_version: DSL_VERSION.to_string(),
            name: name.to_string(),
            description: Some(format!("Parallel workflow with {} branches", branches.len())),
            version: "1.0.0".to_string(),
            author: None,
            tags: vec!["parallel".to_string()],
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            variables: HashMap::new(),
            steps: vec![
                StepDef {
                    id: "parallel_start".to_string(),
                    name: Some("Parallel Start".to_string()),
                    step_type: "parallel".to_string(),
                    skill: None,
                    workflow: None,
                    condition: None,
                    inputs: HashMap::new(),
                    output: None,
                    retry: RetryConfig::default(),
                    timeout: None,
                    on_error: ErrorHandler::default(),
                    parallel: Some(ParallelConfig {
                        branches: branches.iter().enumerate().map(|(i, _)| format!("branch_{}", i)).collect(),
                        aggregate: "all".to_string(),
                    }),
                    loop_config: None,
                    next: None,
                    branches: None,
                },
            ],
            config: WorkflowConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_yaml_parsing() {
        let yaml = r#"
dsl_version: "1.0"
name: test_workflow
version: "1.0.0"
steps:
  - id: step1
    name: Extract Data
    type: skill
    skill: data_extractor
    inputs:
      url: $input.url
    output: extracted_data
    next: step2
  - id: step2
    name: Transform
    type: skill
    skill: transformer
    inputs:
      data: $extracted_data
    output: result
config:
  transactional: true
  timeout: 120
"#;

        let workflow = WorkflowCompiler::from_yaml(yaml).unwrap();
        assert_eq!(workflow.name, "test_workflow");
        assert_eq!(workflow.steps.len(), 2);
    }

    #[test]
    fn test_compile_to_composite() {
        let workflow = WorkflowDefinition {
            dsl_version: DSL_VERSION.to_string(),
            name: "test".to_string(),
            description: None,
            version: "1.0.0".to_string(),
            author: None,
            tags: vec![],
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            variables: HashMap::new(),
            steps: vec![
                StepDef {
                    id: "s1".to_string(),
                    name: None,
                    step_type: "skill".to_string(),
                    skill: Some("skill1".to_string()),
                    workflow: None,
                    condition: None,
                    inputs: HashMap::new(),
                    output: Some("out1".to_string()),
                    retry: RetryConfig::default(),
                    timeout: None,
                    on_error: ErrorHandler::default(),
                    parallel: None,
                    loop_config: None,
                    next: None,
                    branches: None,
                },
            ],
            config: WorkflowConfig::default(),
        };

        let composite = WorkflowCompiler::compile_to_composite(&workflow).unwrap();
        assert_eq!(composite.name, "test");
        assert_eq!(composite.nodes.len(), 1);
    }

    #[test]
    fn test_template_rendering() {
        let template = "Hello, {{name}}!";
        let mut params = HashMap::new();
        params.insert("name".to_string(), json!("World"));

        let result = WorkflowTemplateEngine::render(template, &params).unwrap();
        assert_eq!(result, "Hello, World!");
    }

    #[test]
    fn test_sequential_pipeline_template() {
        let workflow = templates::sequential_pipeline("my_pipeline", vec!["step1", "step2", "step3"]);
        assert_eq!(workflow.steps.len(), 3);
        assert_eq!(workflow.steps[0].skill, Some("step1".to_string()));
    }
}
