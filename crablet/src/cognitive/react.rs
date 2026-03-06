use crate::types::{Message, TraceStep, ContentPart, ToolCall, FunctionCall};
use crate::events::{AgentEvent, EventBus};
use crate::skills::SkillRegistry;
use crate::cognitive::llm::LlmClient;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::collections::{HashSet, HashMap, VecDeque};
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{info, warn, error};
use regex::Regex;
use lazy_static::lazy_static;
// use uuid::Uuid;

// --- 增强型循环检测器 ---

struct LoopDetector {
    // 记录完全匹配的调用 (工具名 + 原始参数字符串)
    exact_history: HashSet<(String, String)>,
    // 记录特定资源的访问频率 (防止对同一张图不断变换 Prompt 刷屏)
    // 结构: tool_name -> resource_id (如 image_path) -> count
    resource_usage: HashMap<String, HashMap<String, usize>>,
    // 语义相似度滑动窗口
    window: VecDeque<String>,
    max_window: usize,
    similarity_threshold: f32,
    // 连续重复计数
    consecutive_repeats: usize,
}

impl LoopDetector {
    fn new() -> Self {
        Self {
            exact_history: HashSet::new(),
            resource_usage: HashMap::new(),
            window: VecDeque::new(),
            max_window: 5,
            similarity_threshold: 0.85, // 85% Jaccard similarity threshold
            consecutive_repeats: 0,
        }
    }

    /// 检测是否陷入循环。包含针对 'see' 工具的特殊逻辑。
    fn is_looping(&mut self, tool: &str, args: &str) -> bool {
        // 1. 基础检测：如果参数完全一致，直接判定为循环
        // 允许最多 1 次重试，但第 2 次相同则报错
        if self.exact_history.contains(&(tool.to_string(), args.to_string())) {
             self.consecutive_repeats += 1;
             if self.consecutive_repeats >= 2 {
                 return true;
             }
        } else {
            self.consecutive_repeats = 0;
            self.exact_history.insert((tool.to_string(), args.to_string()));
        }

        // 2. 资源级语义检测：针对多模态 'see' 工具
        if tool == "see" {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(args) {
                // 提取图片路径作为资源标识符
                if let Some(path) = val.get("image_path").and_then(|v| v.as_str()) {
                    let counts = self.resource_usage.entry(tool.to_string()).or_default();
                    let count = counts.entry(path.to_string()).or_insert(0);
                    *count += 1;
                    
                    // 如果对同一张图片操作超过 3 次，即使 Prompt 不同，也视为陷入语义循环
                    if *count > 3 {
                        warn!("Resource loop detected for path: {}", path);
                        return true;
                    }
                }
            }
        }
        
        // 3. 语义相似度检测 (Sliding Window)
        let action_str = format!("{} {}", tool, args);
        
        for prev in &self.window {
            if self.similarity(prev, &action_str) > self.similarity_threshold {
                warn!("Semantic loop detected: '{}' is similar to '{}'", action_str, prev);
                // 同样允许少量相似重试
                if self.consecutive_repeats >= 1 {
                    return true;
                }
                self.consecutive_repeats += 1;
                return false;
            }
        }
        
        if self.window.len() >= self.max_window {
            self.window.pop_front();
        }
        self.window.push_back(action_str);

        false
    }
    
    fn similarity(&self, s1: &str, s2: &str) -> f32 {
        // Jaccard Similarity on words (Simple Semantic Proxy)
        let set1: HashSet<&str> = s1.split_whitespace().collect();
        let set2: HashSet<&str> = s2.split_whitespace().collect();
        
        let intersection = set1.intersection(&set2).count();
        let union = set1.union(&set2).count();
        
        if union == 0 { return 0.0; }
        intersection as f32 / union as f32
    }
}

// --- 动作解析器 ---

struct ActionParser;

impl ActionParser {
    /// 使用正则提取 "Action: tool {json}" 格式，支持多行 JSON
    fn parse_fallback(text: &str) -> Option<(String, String)> {
        lazy_static! {
            // Updated regex to support multiline JSON args with dot matches newline (?s)
            static ref RE: Regex = Regex::new(r"(?is)Action:\s*(?:use\s+)?(?P<name>[\w\-]+)\s*(?P<args>\{[\s\S]*?\})").expect("Invalid regex pattern");
        }
        RE.captures(text).map(|cap| (cap["name"].to_string(), cap["args"].trim().to_string()))
    }
}

// --- ReAct 引擎主实现 ---

pub struct ReActEngine {
    llm: Arc<Box<dyn LlmClient>>,
    skills: Arc<RwLock<SkillRegistry>>,
    event_bus: Arc<EventBus>,
    // max_steps: usize,
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
            // max_steps: 5,
            // 针对多模态任务，设置一个合理的全局超时（如 30 秒）
            skill_timeout: Duration::from_secs(30),
        }
    }

    pub async fn execute(&self, initial_context: &[Message], max_steps: usize) -> Result<(String, Vec<TraceStep>)> {
        let mut current_context = initial_context.to_vec();
        let mut traces = Vec::with_capacity(max_steps);
        let mut loop_detector = LoopDetector::new();

        for step in 0..max_steps {
            let step_num = step + 1;
            info!("ReAct Step {}/{}", step_num, max_steps);

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
                // Scheme E: Self-Reflection
                // If thought is the final answer, reflect on it.
                // We do a simple reflection if the answer seems short or if we have history.
                // For MVP, we reflect if max_steps > 3 (complex task) and step > 1
                if max_steps > 3 && step > 1 {
                    info!("Triggering Self-Reflection on final answer...");
                    match self.reflect_on_result(&thought, &current_context).await {
                         Ok(reflection) => {
                             if reflection.confidence < 0.8 {
                                 info!("Reflection confidence low ({}), revising...", reflection.confidence);
                                 // Return revised response
                                 let revised = format!("{} (Revised after reflection)", reflection.revised_response);
                                 self.event_bus.publish(AgentEvent::ResponseGenerated(revised.clone()));
                                 traces.push(TraceStep {
                                     step: step_num,
                                     thought: format!("Reflection: {}\nCritique: {}", thought, reflection.critique),
                                     action: None,
                                     action_input: None,
                                     observation: None,
                                 });
                                 return Ok((revised, traces));
                             }
                         },
                         Err(e) => warn!("Reflection failed: {}", e),
                    }
                }
                
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

            // 5. 执行工具 (并行优化)
            let mut assistant_msg_record = response_msg.clone();
            if assistant_msg_record.tool_calls.is_none() {
                assistant_msg_record.tool_calls = Some(tool_calls.clone());
            }
            current_context.push(assistant_msg_record);

            // 分组工具调用：根据依赖关系 (MVP: 简单地全部并行，除非有明确依赖，这里假设无依赖)
            // 如果工具有副作用或顺序依赖，应该在 Prompt 中要求 LLM 顺序调用或使用 Planner。
            // 这里我们使用 FuturesUnordered 或 tokio::join! 进行并行执行。
            
            // 为了安全，限制最大并发数，例如 5
            let semaphore = Arc::new(tokio::sync::Semaphore::new(5));
            let mut tasks = Vec::new();

            for tool_call in tool_calls {
                let func_name = tool_call.function.name.clone();
                let args_str = tool_call.function.arguments.clone();
                let tool_id = tool_call.id.clone();
                
                // 循环检测 (Pre-check)
                // 注意：由于并行执行，resource_usage 可能需要锁，但这里 LoopDetector 是局部的且单线程访问
                // 除非我们将 LoopDetector 放入任务中。
                // 简单起见，我们在主线程做 check，如果通过则 spawn。
                
                if loop_detector.is_looping(&func_name, &args_str) {
                    let loop_msg = format!("Loop detected: repeated or redundant use of '{}'.", func_name);
                    warn!("{}", loop_msg);
                    let obs = format!("System Warning: {}", loop_msg);
                    // Push a completed result immediately
                    tasks.push(tokio::spawn(async move {
                        (tool_id, func_name, args_str, obs)
                    }));
                    continue;
                }
                
                // Clone necessary ARCs for the task
                let skills_clone = self.skills.clone();
                let bus_clone = self.event_bus.clone();
                let timeout_duration = self.skill_timeout;
                let sem_clone = semaphore.clone();
                
                tasks.push(tokio::spawn(async move {
                    // Use unwrap_or_default() or handle error gracefully instead of unwrap()
                    let _permit = match sem_clone.acquire().await {
                        Ok(p) => p,
                        Err(e) => {
                            error!("Semaphore acquire failed: {}", e);
                            return (tool_id, func_name, args_str, format!("System Error: Failed to acquire concurrency permit: {}", e));
                        }
                    };
                    
                    // Publish Start Event
                    bus_clone.publish(AgentEvent::ToolExecutionStarted {
                        tool: func_name.clone(),
                        args: args_str.clone(),
                    });
                    
                    // Execute
                    let execution_future = async {
                        let registry = skills_clone.read().await;
                        match serde_json::from_str(&args_str) {
                            Ok(parsed_json) => registry.execute(&func_name, parsed_json).await
                                .unwrap_or_else(|e| format!("Skill execution failed: {}", e)),
                            Err(e) => format!("Parameter Error: {}. Please use valid JSON.", e),
                        }
                    };
                    
                    let output = match timeout(timeout_duration, execution_future).await {
                        Ok(res) => res,
                        Err(_) => {
                            let err = format!("Execution of '{}' timed out after {}s.", func_name, timeout_duration.as_secs());
                            warn!("{}", err);
                            err
                        }
                    };
                    
                    // Publish Finish Event
                    bus_clone.publish(AgentEvent::ToolExecutionFinished {
                        tool: func_name.clone(),
                        output: output.clone(),
                    });
                    
                    (tool_id, func_name, args_str, output)
                }));
            }
            
            // Await all tasks
            let mut results = Vec::new();
            for task in tasks {
                if let Ok(res) = task.await {
                    results.push(res);
                } else {
                    // Task panic handling
                    error!("A tool execution task panicked");
                }
            }
            
            // Process results in order (or any order, but we need to match tool_ids)
            // Since we collected them, we can just iterate.
            
            for (tool_id, func_name, args_str, observation) in results {
                traces.push(TraceStep {
                    step: step_num,
                    thought: thought.clone(),
                    action: Some(func_name),
                    action_input: Some(args_str),
                    observation: Some(observation.clone()),
                });
                
                current_context.push(Message::new_tool_response(&tool_id, &observation));
            }
        }

        // 6. 达到步数上限
        warn!("ReAct engine reached max_steps ({})", max_steps);
        Ok((self.format_limit_reached_msg(&traces), traces))
    }

    // --- 内部辅助函数 ---
    
    // Scheme E: Self-Reflection Implementation
    async fn reflect_on_result(&self, result: &str, context: &[Message]) -> Result<ReflectionStep> {
        let critique_prompt = format!(
            "Review your answer critically:\n\
             1. Are there factual errors?\n\
             2. Did you miss important nuances?\n\
             3. Could the reasoning be improved?\n\
             Original answer: {}\n\
             \n\
             Provide your critique and a revised answer in JSON format: \
             {{ \"critique\": \"...\", \"revised_response\": \"...\", \"confidence\": 0.0-1.0 }}",
            result
        );
        
        let mut reflection_context = context.to_vec();
        reflection_context.push(Message::new("user", &critique_prompt));
        
        let response = self.llm.chat_complete(&reflection_context).await?;
        
        // Parse JSON from response (naive parsing)
        // Try to find JSON block
        let json_str = if let Some(start) = response.find('{') {
            if let Some(end) = response.rfind('}') {
                &response[start..=end]
            } else {
                &response
            }
        } else {
            &response
        };
        
        let reflection: ReflectionStep = serde_json::from_str(json_str)
            .map_err(|e| anyhow!("Failed to parse reflection JSON: {}", e))?;
            
        Ok(reflection)
    }

    // async fn run_skill_task(&self, name: &str, args: &str) -> String {
    //     self.event_bus.publish(AgentEvent::ToolExecutionStarted {
    //         tool: name.into(),
    //         args: args.into(),
    //     });
    //
    //     let registry = self.skills.read().await;
    //     let output = match serde_json::from_str(args) {
    //         Ok(parsed_json) => registry.execute(name, parsed_json).await
    //             .unwrap_or_else(|e| format!("Skill execution failed: {}", e)),
    //         Err(e) => format!("Parameter Error: {}. Please use valid JSON.", e),
    //     };
    //
    //     self.event_bus.publish(AgentEvent::ToolExecutionFinished {
    //         tool: name.into(),
    //         output: output.clone(),
    //     });
    //     output
    // }

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

#[derive(serde::Deserialize)]
struct ReflectionStep {
    #[allow(dead_code)]
    critique: String,
    revised_response: String,
    confidence: f64,
}