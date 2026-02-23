// use crate::types::{Message, TraceStep, ContentPart};
// use std::sync::Arc;
// use tokio::sync::RwLock;
// use tracing::{info, warn};
// use crate::events::{AgentEvent, EventBus};
// use crate::skills::SkillRegistry;
// use crate::cognitive::llm::LlmClient;
// use anyhow::Result;

// pub struct ReActEngine {
//     llm: Arc<Box<dyn LlmClient>>,
//     skills: Arc<RwLock<SkillRegistry>>,
//     event_bus: Arc<EventBus>,
//     max_steps: usize,
// }

// impl ReActEngine {
//     pub fn new(llm: Arc<Box<dyn LlmClient>>, skills: Arc<RwLock<SkillRegistry>>, event_bus: Arc<EventBus>) -> Self {
//         Self {
//             llm,
//             skills,
//             event_bus,
//             max_steps: 5,
//         }
//     }

//     pub async fn execute(&self, initial_context: &[Message]) -> Result<(String, Vec<TraceStep>)> {
//         let mut current_context = initial_context.to_vec();
//         let mut traces = Vec::new();
        
//         for step in 0..self.max_steps {
//             info!("ReAct Step {}/{}", step + 1, self.max_steps);
            
//             // Get available tools definition
//             let tool_definitions = {
//                 let registry = self.skills.read().await;
//                 registry.to_tool_definitions()
//             };

//             // Force system prompt instruction to use tool outputs
//             let mut current_context_with_system = current_context.clone();
//             if step > 0 {
//                 // Add a system reminder to use the tool output
//                 current_context_with_system.push(Message::new("system", "You have just received the output from a tool. Use this information to answer the user's question. Do not repeat the same tool call with the same arguments."));
//             }

//             let response_msg = match self.llm.chat_complete_with_tools(&current_context_with_system, &tool_definitions).await {
//                 Ok(msg) => msg,
//                 Err(e) => {
//                     warn!("ReAct Loop LLM Error: {}", e);
//                     self.event_bus.publish(AgentEvent::Error(format!("LLM Error: {}", e)));
//                     if step == 0 {
//                         return Err(anyhow::anyhow!("LLM Error: {}", e));
//                     } else {
//                          return Ok(("I encountered an error while thinking. Please try again.".to_string(), traces));
//                     }
//                 }
//             };
            
//             // Extract content for thought trace
//             let thought = response_msg.content.as_ref().map(|c| {
//                 c.iter().filter_map(|p| match p {
//                     ContentPart::Text { text } => Some(text.as_str()),
//                     _ => None,
//                 }).collect::<Vec<_>>().join("")
//             }).unwrap_or_default();
            
//             if !thought.is_empty() {
//                 self.event_bus.publish(AgentEvent::ThoughtGenerated(thought.clone()));
//             }

//             let mut current_trace = TraceStep {
//                 step: step + 1,
//                 thought: thought.clone(),
//                 action: None,
//                 action_input: None,
//                 observation: None,
//             };

//             // Fallback: Parse ReAct style "Action: ..." from text if no tool calls
//             let mut effective_tool_calls = response_msg.tool_calls.clone().unwrap_or_default();
            
//             if effective_tool_calls.is_empty() && thought.contains("Action:") {
//                 // Try to parse Action: use <tool> <args> or Action: <tool> <args>
//                 if let Some(action_part) = thought.split("Action:").nth(1) {
//                     let action_line = action_part.lines().next().unwrap_or("").trim();
//                     if !action_line.is_empty() {
//                         // Handle "use tool {...}" or "tool {...}"
//                         let (tool_name, args_str) = if action_line.starts_with("use ") {
//                             let rest = action_line.strip_prefix("use ").unwrap().trim();
//                             // Split by first space or first brace
//                             if let Some(idx) = rest.find(|c| c == ' ' || c == '{') {
//                                 (&rest[..idx], &rest[idx..])
//                             } else {
//                                 (rest, "{}")
//                             }
//                         } else {
//                              if let Some(idx) = action_line.find(|c| c == ' ' || c == '{') {
//                                 (&action_line[..idx], &action_line[idx..])
//                             } else {
//                                 (action_line, "{}")
//                             }
//                         };
                        
//                         let tool_name = tool_name.trim();
//                         let args_str = args_str.trim();
                        
//                         if !tool_name.is_empty() {
//                             info!("ReAct Fallback: Parsed action '{}' args '{}'", tool_name, args_str);
//                             effective_tool_calls.push(crate::types::ToolCall {
//                                 id: format!("call_{}", uuid::Uuid::new_v4()),
//                                 r#type: "function".to_string(),
//                                 function: crate::types::FunctionCall {
//                                     name: tool_name.to_string(),
//                                     arguments: args_str.to_string(),
//                                 }
//                             });
//                         }
//                     }
//                 }
//             }

//             // Check for Tool Calls
//             if !effective_tool_calls.is_empty() {
//                 // Add assistant message with tool calls to context
//                 // If we parsed fallback calls, we need to ensure the message in context reflects that
//                 let mut msg_to_push = response_msg.clone();
//                 if msg_to_push.tool_calls.is_none() || msg_to_push.tool_calls.as_ref().unwrap().is_empty() {
//                     msg_to_push.tool_calls = Some(effective_tool_calls.clone());
//                 }
//                 current_context.push(msg_to_push);
                
//                 for tool_call in &effective_tool_calls {
//                     let function_name = &tool_call.function.name;
//                     let arguments = &tool_call.function.arguments;
                        
//                         info!("ReAct: Decided to use skill '{}'", function_name);
//                         current_trace.action = Some(function_name.clone());
//                         current_trace.action_input = Some(arguments.clone());
                        
//                         self.event_bus.publish(AgentEvent::ToolExecutionStarted { 
//                             tool: function_name.clone(), 
//                             args: arguments.clone() 
//                         });
                        
//                         let registry = self.skills.read().await;
                        
//                         // Execute Skill
//                         let output_result = match serde_json::from_str(&arguments) {
//                             Ok(args) => registry.execute(function_name, args).await,
//                             Err(e) => Err(anyhow::anyhow!("Error parsing arguments: {}", e)),
//                         };

//                         let output = match output_result {
//                             Ok(o) => o,
//                             Err(e) => format!("Error executing skill: {}", e),
//                         };
                        
//                         self.event_bus.publish(AgentEvent::ToolExecutionFinished { 
//                             tool: function_name.clone(), 
//                             output: output.clone() 
//                         });
                        
//                         // Update current trace with observation
//                         current_trace.observation = Some(output.clone());
//                         traces.push(current_trace.clone());

//                         // Add tool response message to context
//                         current_context.push(Message::new_tool_response(&tool_call.id, &output));
                    
//                 } // End for loop over tool_calls

//                 // Loop Detection: Check if we are repeating the exact same tool calls
//                 if traces.len() >= 2 {
//                     let last_trace = &traces[traces.len() - 1];
//                     let prev_trace = &traces[traces.len() - 2];
                    
//                     if last_trace.action.is_some() && 
//                        last_trace.action == prev_trace.action && 
//                        last_trace.action_input == prev_trace.action_input {
                        
//                         warn!("ReAct Loop Detected: Repeating tool {} with args {:?}", last_trace.action.as_deref().unwrap_or("?"), last_trace.action_input);
                        
//                         let final_answer = format!("I seem to be repeating myself. Here is the result from the tool:\n\n{}", last_trace.observation.as_deref().unwrap_or(""));
//                         self.event_bus.publish(AgentEvent::ResponseGenerated(final_answer.clone()));
//                         return Ok((final_answer, traces));
//                     }
//                 }
                
//                 // IMPORTANT: Continue to next iteration so LLM can see the tool output
//                 continue;
//             }

//             // If no tool calls, this is the final response
//             self.event_bus.publish(AgentEvent::ResponseGenerated(thought.clone()));
//             traces.push(current_trace);
//             return Ok((thought, traces));
//         }

//         warn!("ReAct loop reached maximum steps ({}) without final answer.", self.max_steps);
//         // Return the last thought or observation if available
//         let final_msg = if let Some(last) = traces.last() {
//              if let Some(obs) = &last.observation {
//                  format!("I reached the maximum number of steps. Here is the last tool output:\n\n{}", obs)
//              } else {
//                  last.thought.clone()
//              }
//         } else {
//             "I thought for too long and couldn't reach a conclusion.".to_string()
//         };
        
//         Ok((final_msg, traces))
//     }
// }

use crate::types::{Message, TraceStep, ContentPart, ToolCall, FunctionCall};
use crate::events::{AgentEvent, EventBus};
use crate::skills::SkillRegistry;
use crate::cognitive::llm::LlmClient;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::collections::{HashSet, HashMap};
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{info, warn};
use regex::Regex;
use lazy_static::lazy_static;

// --- 增强型循环检测器 ---

struct LoopDetector {
    // 记录完全匹配的调用 (工具名 + 原始参数字符串)
    exact_history: HashSet<(String, String)>,
    // 记录特定资源的访问频率 (防止对同一张图不断变换 Prompt 刷屏)
    // 结构: tool_name -> resource_id (如 image_path) -> count
    resource_usage: HashMap<String, HashMap<String, usize>>,
}

impl LoopDetector {
    fn new() -> Self {
        Self {
            exact_history: HashSet::new(),
            resource_usage: HashMap::new(),
        }
    }

    /// 检测是否陷入循环。包含针对 'see' 工具的特殊逻辑。
    fn is_looping(&mut self, tool: &str, args: &str) -> bool {
        // 1. 基础检测：如果参数完全一致，直接判定为循环
        if !self.exact_history.insert((tool.to_string(), args.to_string())) {
            return true;
        }

        // 2. 资源级语义检测：针对多模态 'see' 工具
        if tool == "see" {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(args) {
                // 提取图片路径作为资源标识符
                if let Some(path) = val.get("image_path").and_then(|v| v.as_str()) {
                    let counts = self.resource_usage.entry(tool.to_string()).or_default();
                    let count = counts.entry(path.to_string()).or_insert(0);
                    *count += 1;
                    
                    // 如果对同一张图片操作超过 2 次，即使 Prompt 不同，也视为陷入语义循环
                    if *count > 2 {
                        warn!("Resource loop detected for path: {}", path);
                        return true;
                    }
                }
            }
        }
        false
    }
}

// --- 动作解析器 ---

struct ActionParser;

impl ActionParser {
    /// 使用正则提取 "Action: tool {json}" 格式
    fn parse_fallback(text: &str) -> Option<(String, String)> {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"(?i)Action:\s*(?:use\s+)?(?P<name>[\w\-]+)\s*(?P<args>\{.*\})").unwrap();
        }
        RE.captures(text).map(|cap| (cap["name"].to_string(), cap["args"].trim().to_string()))
    }
}

// --- ReAct 引擎主实现 ---

pub struct ReActEngine {
    llm: Arc<Box<dyn LlmClient>>,
    skills: Arc<RwLock<SkillRegistry>>,
    event_bus: Arc<EventBus>,
    max_steps: usize,
    skill_timeout: Duration,
}

impl ReActEngine {
    pub fn new(
        llm: Arc<Box<dyn LlmClient>>,
        skills: Arc<RwLock<SkillRegistry>>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            llm,
            skills,
            event_bus,
            max_steps: 5,
            // 针对多模态任务，设置一个合理的全局超时（如 30 秒）
            skill_timeout: Duration::from_secs(30),
        }
    }

    pub async fn execute(&self, initial_context: &[Message]) -> Result<(String, Vec<TraceStep>)> {
        let mut current_context = initial_context.to_vec();
        let mut traces = Vec::with_capacity(self.max_steps);
        let mut loop_detector = LoopDetector::new();

        for step in 0..self.max_steps {
            let step_num = step + 1;
            info!("ReAct Step {}/{}", step_num, self.max_steps);

            let tool_definitions = self.skills.read().await.to_tool_definitions();

            // 1. 动态构造上下文
            let mut prompt_context = current_context.clone();
            if step > 0 {
                // 注入强力系统提示，防止模型复读
                prompt_context.push(Message::new("system", 
                    "Instruction: You have received tool outputs. Use them to answer directly. \
                    Do NOT repeat the same tool call with similar prompts if it hasn't provided new information."));
            }

            // 2. 获取 LLM 响应
            let response_msg = match self.llm.chat_complete_with_tools(&prompt_context, &tool_definitions).await {
                Ok(msg) => msg,
                Err(e) => {
                    warn!("LLM Error at step {}: {}", step_num, e);
                    self.event_bus.publish(AgentEvent::Error(e.to_string()));
                    if step == 0 { return Err(anyhow!("LLM Initial Failure: {}", e)); }
                    return Ok(("Thinking failed due to an LLM error.".to_string(), traces));
                }
            };

            let thought = self.extract_thought(&response_msg);
            if !thought.is_empty() {
                self.event_bus.publish(AgentEvent::ThoughtGenerated(thought.clone()));
            }

            // 3. 动作识别
            let mut tool_calls = response_msg.tool_calls.clone().unwrap_or_default();
            if tool_calls.is_empty() {
                if let Some((name, args)) = ActionParser::parse_fallback(&thought) {
                    tool_calls.push(ToolCall {
                        id: format!("call_{}", uuid::Uuid::new_v4()),
                        r#type: "function".to_string(),
                        function: FunctionCall { name, arguments: args },
                    });
                }
            }

            // 4. 终止条件检查
            if tool_calls.is_empty() {
                self.event_bus.publish(AgentEvent::ResponseGenerated(thought.clone()));
                traces.push(TraceStep {
                    step: step_num,
                    thought: thought.clone(),
                    action: None,
                    action_input: None,
                    observation: None,
                });
                return Ok((thought, traces));
            }

            // 5. 执行工具 (同步 Assistant 消息)
            let mut assistant_msg_record = response_msg.clone();
            if assistant_msg_record.tool_calls.is_none() {
                assistant_msg_record.tool_calls = Some(tool_calls.clone());
            }
            current_context.push(assistant_msg_record);

            for tool_call in tool_calls {
                let func_name = &tool_call.function.name;
                let args_str = &tool_call.function.arguments;

                // --- 循环拦截 ---
                if loop_detector.is_looping(func_name, args_str) {
                    let loop_msg = format!("Loop detected: repeated or redundant use of '{}'.", func_name);
                    warn!("{}", loop_msg);
                    let last_obs = traces.last().and_then(|t| t.observation.clone()).unwrap_or_default();
                    return Ok((format!("I noticed a loop in my thinking. Here's the last info I got: {}", last_obs), traces));
                }

                // --- 带超时的执行 ---
                let observation = match timeout(self.skill_timeout, self.run_skill_task(func_name, args_str)).await {
                    Ok(result) => result,
                    Err(_) => {
                        let timeout_err = format!("Execution of '{}' timed out after {}s.", func_name, self.skill_timeout.as_secs());
                        warn!("{}", timeout_err);
                        timeout_err
                    }
                };

                // --- 更新 Trace 和上下文 ---
                traces.push(TraceStep {
                    step: step_num,
                    thought: thought.clone(),
                    action: Some(func_name.clone()),
                    action_input: Some(args_str.clone()),
                    observation: Some(observation.clone()),
                });

                current_context.push(Message::new_tool_response(&tool_call.id, &observation));
            }
        }

        // 6. 达到步数上限
        warn!("ReAct engine reached max_steps ({})", self.max_steps);
        Ok((self.format_limit_reached_msg(&traces), traces))
    }

    // --- 内部辅助函数 ---

    async fn run_skill_task(&self, name: &str, args: &str) -> String {
        self.event_bus.publish(AgentEvent::ToolExecutionStarted {
            tool: name.into(),
            args: args.into(),
        });

        let registry = self.skills.read().await;
        let output = match serde_json::from_str(args) {
            Ok(parsed_json) => registry.execute(name, parsed_json).await
                .unwrap_or_else(|e| format!("Skill execution failed: {}", e)),
            Err(e) => format!("Parameter Error: {}. Please use valid JSON.", e),
        };

        self.event_bus.publish(AgentEvent::ToolExecutionFinished {
            tool: name.into(),
            output: output.clone(),
        });
        output
    }

    fn extract_thought(&self, msg: &Message) -> String {
        msg.content.as_ref().map(|parts| {
            parts.iter().filter_map(|p| match p {
                ContentPart::Text { text } => Some(text.as_str()),
                _ => None,
            }).collect::<Vec<_>>().join("")
        }).unwrap_or_default()
    }

    fn format_limit_reached_msg(&self, traces: &[TraceStep]) -> String {
        if let Some(last) = traces.last() {
            if let Some(obs) = &last.observation {
                return format!("I reached my maximum steps. Here is the last piece of information I found: \n\n{}", obs);
            }
            return last.thought.clone();
        }
        "I couldn't finish the task within the maximum step limit.".to_string()
    }
}