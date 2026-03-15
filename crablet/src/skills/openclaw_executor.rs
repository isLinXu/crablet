//! OpenClaw Skill 执行引擎
//!
//! OpenClaw 是纯提示词驱动的技能格式，通过 LLM 执行指令。
//! 支持 ReAct 循环、工具调用和结构化输出。

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::cognitive::llm::LlmClient;
use crate::types::{ContentPart, Message, ToolCall, TraceStep};
use crate::tools::manager::ToolManager;

/// OpenClaw 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenClawResult {
    /// 最终输出
    pub output: String,
    /// 执行迭代次数
    pub iterations: usize,
    /// 工具调用记录
    pub tool_calls: Vec<ToolCallRecord>,
    /// 执行追踪
    pub traces: Vec<TraceStep>,
    /// 执行时间（毫秒）
    pub duration_ms: u64,
}

/// 工具调用记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub iteration: usize,
    pub tool_name: String,
    pub arguments: Value,
    pub result: String,
    pub success: bool,
}

/// OpenClaw 执行引擎
pub struct OpenClawEngine {
    llm_client: Arc<dyn LlmClient>,
    tool_manager: Arc<ToolManager>,
    max_iterations: usize,
}

impl OpenClawEngine {
    /// 创建新的 OpenClaw 引擎
    pub fn new(llm_client: Arc<dyn LlmClient>, tool_manager: Arc<ToolManager>) -> Self {
        Self {
            llm_client,
            tool_manager,
            max_iterations: 10,
        }
    }

    /// 设置最大迭代次数
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }

    /// 执行 OpenClaw Skill
    pub async fn execute(
        &self,
        skill_name: &str,
        skill_instructions: &str,
        args: &Value,
        context: Option<ExecutionContext>,
    ) -> Result<OpenClawResult> {
        let start_time = std::time::Instant::now();
        info!("Executing OpenClaw skill: {}", skill_name);

        // 1. 准备系统提示词
        let system_prompt = self.build_system_prompt(skill_name, skill_instructions);

        // 2. 准备用户消息（参数插值）
        let user_prompt = self.interpolate_args(skill_instructions, args);

        // 3. 构建消息历史
        let mut messages = vec![
            Message::system(system_prompt),
            Message::user(user_prompt),
        ];

        // 4. 添加上下文（如果有）
        if let Some(ctx) = context {
            messages = self.inject_context(messages, ctx);
        }

        let mut tool_calls = Vec::new();
        let mut traces = Vec::new();
        let mut iteration = 0;

        // 5. ReAct 循环
        while iteration < self.max_iterations {
            debug!("OpenClaw iteration {}/{}", iteration + 1, self.max_iterations);

            // 获取可用工具定义
            let tools = self.tool_manager.to_tool_definitions();

            // 调用 LLM
            let response_message = if tools.is_empty() {
                let response_text = self.llm_client.chat_complete(&messages).await?;
                Message::assistant(response_text)
            } else {
                self.llm_client.chat_complete_with_tools(&messages, &tools).await?
            };

            // 解析响应
            match self.parse_response(&response_message) {
                ResponseType::ToolCall(tool_call) => {
                    debug!("Tool call requested: {}", tool_call.function.name);
                    
                    // 记录追踪
                    traces.push(TraceStep {
                        step: iteration,
                        thought: format!("Calling tool: {} with args: {}", 
                            tool_call.function.name, 
                            tool_call.function.arguments
                        ),
                        action: Some(tool_call.function.name.clone()),
                        action_input: Some(tool_call.function.arguments.clone()),
                        observation: None,
                    });

                    // 执行工具
                    let tool_result = match self.execute_tool_call(&tool_call).await {
                        Ok(result) => {
                            tool_calls.push(ToolCallRecord {
                                iteration,
                                tool_name: tool_call.function.name.clone(),
                                arguments: serde_json::from_str(&tool_call.function.arguments)?,
                                result: result.clone(),
                                success: true,
                            });
                            result
                        }
                        Err(e) => {
                            let error_msg = format!("Tool execution failed: {}", e);
                            tool_calls.push(ToolCallRecord {
                                iteration,
                                tool_name: tool_call.function.name.clone(),
                                arguments: serde_json::from_str(&tool_call.function.arguments)?,
                                result: error_msg.clone(),
                                success: false,
                            });
                            error_msg
                        }
                    };

                    // 添加工具结果到消息历史
                    messages.push(response_message);
                    messages.push(Message::tool_result(
                        tool_call.id,
                        tool_result.clone(),
                    ));

                    // 更新追踪
                    if let Some(last_trace) = traces.last_mut() {
                        last_trace.observation = Some(tool_result);
                    }
                }
                ResponseType::FinalResult(output) => {
                    debug!("Final result received");
                    traces.push(TraceStep {
                        step: iteration,
                        thought: "Task completed successfully".to_string(),
                        action: None,
                        action_input: None,
                        observation: Some(output.clone()),
                    });

                    return Ok(OpenClawResult {
                        output,
                        iterations: iteration + 1,
                        tool_calls,
                        traces,
                        duration_ms: start_time.elapsed().as_millis() as u64,
                    });
                }
                ResponseType::Error(e) => {
                    warn!("Error in OpenClaw execution: {}", e);
                    return Err(anyhow!("OpenClaw execution error: {}", e));
                }
            }

            iteration += 1;
            
            // 检查是否达到最大迭代次数
            if iteration >= self.max_iterations {
                warn!("Max iterations reached for OpenClaw skill");
                return Err(anyhow!("Max iterations ({}) reached", self.max_iterations));
            }
        }

        Err(anyhow!("OpenClaw execution failed unexpectedly"))
    }

    /// 构建系统提示词
    fn build_system_prompt(&self, skill_name: &str, _skill_instructions: &str) -> String {
        let tools_desc = if self.tool_manager.is_empty() {
            "No additional tools available.".to_string()
        } else {
            format!("Available tools:\n{}", self.format_tool_descriptions())
        };

        format!(
            r#"You are executing the '{}' skill.
Your goal is to complete the task by following the instructions and using the available tools.

Guidelines:
1. Break down complex tasks into smaller steps.
2. Use tools when necessary to gather information or perform actions.
3. If a tool fails, try to understand why and adjust your approach.
4. Provide a clear and concise final answer once the task is complete.

{}

Execute the skill according to the instructions."#,
            skill_name, tools_desc
        )
    }

    /// 格式化工具描述
    fn format_tool_descriptions(&self) -> String {
        self.tool_manager
            .list_tools()
            .iter()
            .map(|t| format!("- {}: {}", t.name(), t.description()))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 参数插值
    fn interpolate_args(&self, template: &str, args: &Value) -> String {
        let mut result = template.to_string();
        
        if let Some(obj) = args.as_object() {
            for (key, val) in obj {
                let placeholder = format!("{{{{{}}}}}", key);
                let replacement = match val {
                    Value::String(s) => s.clone(),
                    _ => val.to_string(),
                };
                result = result.replace(&placeholder, &replacement);
            }
        }
        
        result
    }

    /// 注入上下文到消息历史
    fn inject_context(&self, mut messages: Vec<Message>, context: ExecutionContext) -> Vec<Message> {
        // 保留系统消息
        let mut result = vec![messages.remove(0)];
        
        // 合并历史消息
        if let Some(history) = context.conversation_history {
            result.extend(history);
        }
        
        // 添加上下文（限制最近 5 条）
        // 这里可以根据 metadata 注入更多信息
        
        // 添加当前用户消息
        result.push(messages[0].clone());
        
        result
    }

    /// 解析 LLM 响应
    fn parse_response(&self, message: &Message) -> ResponseType {
        // 检查是否有工具调用
        if let Some(tool_calls) = &message.tool_calls {
            if let Some(first_call) = tool_calls.first() {
                return ResponseType::ToolCall(first_call.clone());
            }
        }

        // 检查内容
        if let Some(content_parts) = &message.content {
            let text: String = content_parts
                .iter()
                .filter_map(|part| match part {
                    ContentPart::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect();

            if text.trim().is_empty() {
                ResponseType::Error("Empty response from LLM".to_string())
            } else {
                ResponseType::FinalResult(text)
            }
        } else {
            ResponseType::Error("No content in LLM response".to_string())
        }
    }

    /// 执行工具调用
    async fn execute_tool_call(&self, tool_call: &ToolCall) -> Result<String> {
        let args: Value = serde_json::from_str(&tool_call.function.arguments)
            .context("Failed to parse tool arguments")?;
        
        self.tool_manager.execute(&tool_call.function.name, args).await
    }
}

/// 执行上下文
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    pub conversation_history: Option<Vec<Message>>,
    pub metadata: HashMap<String, Value>,
}

impl ExecutionContext {
    pub fn new() -> Self {
        Self {
            conversation_history: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_history(mut self, history: Vec<Message>) -> Self {
        self.conversation_history = Some(history);
        self
    }

    pub fn with_metadata(mut self, key: &str, value: Value) -> Self {
        self.metadata.insert(key.to_string(), value);
        self
    }
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self::new()
    }
}

/// 响应类型
enum ResponseType {
    ToolCall(ToolCall),
    FinalResult(String),
    Error(String),
}

/// 工具信息
#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// 工具管理器 trait
pub trait ToolManagerTrait: Send + Sync {
    fn list_tools(&self) -> Vec<ToolInfo>;
    fn to_tool_definitions(&self) -> Vec<Value>;
    fn execute(&self, name: &str, args: Value) -> impl std::future::Future<Output = Result<String>> + Send;
    fn is_empty(&self) -> bool;
}

// 为 ToolManager 实现 trait（需要在 tools/manager.rs 中实现）
impl ToolManagerTrait for ToolManager {
    fn list_tools(&self) -> Vec<ToolInfo> {
        // 实现工具列表获取
        vec![]
    }

    fn to_tool_definitions(&self) -> Vec<Value> {
        // 转换为 OpenAI 工具格式
        vec![]
    }

    async fn execute(&self, name: &str, args: Value) -> Result<String> {
        // 执行工具
        Ok(format!("Tool {} executed with args: {}", name, args))
    }

    fn is_empty(&self) -> bool {
        true
    }
}
