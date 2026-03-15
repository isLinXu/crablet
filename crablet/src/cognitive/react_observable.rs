//! Observable ReAct Engine
//!
//! Enhanced ReAct engine with full observability integration.

use crate::types::{Message, TraceStep, ContentPart, ToolCall, FunctionCall};
use crate::events::{AgentEvent, EventBus};
use crate::skills::SkillRegistry;
use crate::cognitive::llm::LlmClient;
use crate::observability::{
    AgentTracer, BreakpointManager, ExecutionMetrics, StepMetrics,
    ExecutionContext, BreakpointAction, LoopType, LoopResolution,
};
use anyhow::{Result, anyhow};
use std::sync::Arc;
use std::collections::{HashSet, HashMap, VecDeque};
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::timeout;
use tracing::{info, warn, error};
use regex::Regex;
use lazy_static::lazy_static;

/// Observable ReAct Engine with full tracing and debugging support
pub struct ObservableReActEngine {
    llm: Arc<Box<dyn LlmClient>>,
    skills: Arc<RwLock<SkillRegistry>>,
    event_bus: Arc<EventBus>,
    tracer: Arc<RwLock<AgentTracer>>,
    breakpoint_manager: Arc<RwLock<BreakpointManager>>,
    skill_timeout: Duration,
}

impl ObservableReActEngine {
    pub fn new(
        llm: Arc<Box<dyn LlmClient>>,
        skills: Arc<RwLock<SkillRegistry>>,
        event_bus: Arc<EventBus>,
        tracer: Arc<RwLock<AgentTracer>>,
        breakpoint_manager: Arc<RwLock<BreakpointManager>>,
    ) -> Self {
        Self {
            llm,
            skills,
            event_bus,
            tracer,
            breakpoint_manager,
            skill_timeout: Duration::from_secs(30),
        }
    }

    pub async fn execute(
        &self,
        execution_id: &str,
        initial_context: &[Message],
        max_steps: usize,
    ) -> Result<(String, Vec<TraceStep>)> {
        let mut current_context = initial_context.to_vec();
        let mut traces = Vec::with_capacity(max_steps);
        let mut loop_detector = LoopDetector::new();
        let mut metrics = ExecutionMetrics::new(execution_id.to_string());

        for step in 0..max_steps {
            let step_num = step + 1;
            info!("ReAct Step {}/{}", step_num, max_steps);

            // Check for breakpoint before step
            let context = ExecutionContext {
                execution_id: execution_id.to_string(),
                step_number: step_num,
                current_thought: None,
                current_action: None,
                variables: HashMap::new(),
            };

            let bp_manager = self.breakpoint_manager.read().await;
            if let Some(action) = bp_manager.check_breakpoint(&context).await {
                match action {
                    BreakpointAction::Abort { reason } => {
                        return Err(anyhow!("Execution aborted by breakpoint: {}", reason));
                    }
                    BreakpointAction::InjectHint { hint } => {
                        current_context.push(Message::new("system", &hint));
                    }
                    BreakpointAction::ModifyContext { variable_updates } => {
                        // Apply variable updates
                        for (key, value) in variable_updates {
                            // Would need to integrate with actual variable system
                            info!("Variable {} updated to {:?}", key, value);
                        }
                    }
                    _ => {} // Continue or Skip handled below
                }
            }

            let tool_definitions = self.skills.read().await.to_tool_definitions();

            // 1. Dynamic context construction
            let mut prompt_context = current_context.clone();
            if step > 0 {
                prompt_context.push(Message::new("system",
                    "Instruction: You have received tool outputs. Use them to answer directly. \
                    Do NOT repeat the same tool call with similar prompts if it hasn't provided new information."));
            }

            // 2. Get LLM response
            let start_time = std::time::Instant::now();
            let response_msg = match self.llm.chat_complete_with_tools(&prompt_context, &tool_definitions).await {
                Ok(msg) => msg,
                Err(e) => {
                    warn!("LLM Error at step {}: {}", step_num, e);
                    self.event_bus.publish(AgentEvent::Error(e.to_string()));
                    
                    // Trace error
                    let tracer = self.tracer.write().await;
                    tracer.trace_error(
                        execution_id,
                        &format!("LLM Error: {}", e),
                        false
                    ).await;
                    
                    if step == 0 { return Err(anyhow!("LLM Initial Failure: {}", e)); }
                    return Ok(("Thinking failed due to an LLM error.".to_string(), traces));
                }
            };

            let llm_duration = start_time.elapsed().as_millis() as u64;

            // 3. Extract thought and trace it
            let thought = self.extract_thought(&response_msg);
            if !thought.is_empty() {
                self.event_bus.publish(AgentEvent::ThoughtGenerated(thought.clone()));
                
                // Trace thought
                let tracer = self.tracer.write().await;
                tracer.trace_thought(
                    execution_id,
                    &thought,
                    Some(crate::observability::ThoughtMetadata {
                        step_number: step_num,
                        iteration: step,
                        confidence: None,
                        tags: vec!["react".to_string()],
                    })
                ).await;
            }

            // 4. Action recognition
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

            // 5. Termination check
            if tool_calls.is_empty() {
                // Self-Reflection
                if max_steps > 3 && step > 1 {
                    info!("Triggering Self-Reflection on final answer...");
                    match self.reflect_on_result(&thought, &current_context).await {
                        Ok(reflection) => {
                            // Trace reflection
                            let tracer = self.tracer.write().await;
                            tracer.trace_reflection(
                                execution_id,
                                &reflection.critique,
                                reflection.confidence,
                                Some(reflection.revised_response.clone())
                            ).await;

                            if reflection.confidence < 0.8 {
                                info!("Reflection confidence low ({}), revising...", reflection.confidence);
                                let revised = format!("{} (Revised after reflection)", reflection.revised_response);
                                self.event_bus.publish(AgentEvent::ResponseGenerated(revised.clone()));
                                traces.push(TraceStep {
                                    step: step_num,
                                    thought: format!("Reflection: {}\nCritique: {}", thought, reflection.critique),
                                    action: None,
                                    action_input: None,
                                    observation: None,
                                });
                                
                                // Record metrics
                                metrics.record_step(StepMetrics {
                                    step_number: step_num,
                                    step_type: "reflection".to_string(),
                                    duration_ms: llm_duration,
                                    ..Default::default()
                                });
                                
                                metrics.finish();
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

                // Record final metrics
                metrics.record_step(StepMetrics {
                    step_number: step_num,
                    step_type: "final_answer".to_string(),
                    duration_ms: llm_duration,
                    ..Default::default()
                });
                metrics.finish();
                
                // End trace session
                let tracer = self.tracer.write().await;
                tracer.end_session(execution_id, true, Some(thought.clone())).await;
                
                return Ok((thought, traces));
            }

            // 6. Execute tools (parallel optimization)
            let mut assistant_msg_record = response_msg.clone();
            if assistant_msg_record.tool_calls.is_none() {
                assistant_msg_record.tool_calls = Some(tool_calls.clone());
            }
            current_context.push(assistant_msg_record);

            // Trace actions
            let tracer = self.tracer.write().await;
            for tool_call in &tool_calls {
                tracer.trace_action(
                    execution_id,
                    &tool_call.function.name,
                    serde_json::from_str(&tool_call.function.arguments).unwrap_or_default(),
                    Some(thought.clone()),
                ).await;
            }

            // Parallel execution with semaphore
            let semaphore = Arc::new(tokio::sync::Semaphore::new(5));
            let mut tasks = Vec::new();

            for tool_call in tool_calls {
                let func_name = tool_call.function.name.clone();
                let args_str = tool_call.function.arguments.clone();
                let tool_id = tool_call.id.clone();

                // Loop detection (Pre-check)
                if loop_detector.is_looping(&func_name, &args_str) {
                    let loop_msg = format!("Loop detected: repeated or redundant use of '{}'.", func_name);
                    warn!("{}", loop_msg);
                    
                    // Trace loop detection
                    let tracer = self.tracer.write().await;
                    tracer.trace_loop_detected(
                        execution_id,
                        LoopType::ExactRepetition,
                        loop_msg.clone(),
                        LoopResolution::Continue,
                    ).await;
                    
                    let obs = format!("System Warning: {}", loop_msg);
                    tasks.push(tokio::spawn(async move {
                        (tool_id, func_name, args_str, obs)
                    }));
                    continue;
                }

                let skills_clone = self.skills.clone();
                let bus_clone = self.event_bus.clone();
                let timeout_duration = self.skill_timeout;
                let sem_clone = semaphore.clone();

                tasks.push(tokio::spawn(async move {
                    let _permit = match sem_clone.acquire().await {
                        Ok(p) => p,
                        Err(e) => {
                            error!("Semaphore acquire failed: {}", e);
                            return (tool_id, func_name, args_str, format!("System Error: Failed to acquire concurrency permit: {}", e));
                        }
                    };

                    bus_clone.publish(AgentEvent::ToolExecutionStarted {
                        tool: func_name.clone(),
                        args: args_str.clone(),
                    });

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
                    error!("A tool execution task panicked");
                }
            }

            // Process results
            for (tool_id, func_name, args_str, observation) in results {
                traces.push(TraceStep {
                    step: step_num,
                    thought: thought.clone(),
                    action: Some(func_name.clone()),
                    action_input: Some(args_str),
                    observation: Some(observation.clone()),
                });

                // Trace observation
                let tracer = self.tracer.write().await;
                tracer.trace_observation(
                    execution_id,
                    serde_json::json!({"result": observation}),
                    0, // Duration tracked separately
                    !observation.contains("Error"),
                ).await;

                current_context.push(Message::new_tool_response(&tool_id, &observation));
            }

            // Record step metrics
            metrics.record_step(StepMetrics {
                step_number: step_num,
                step_type: "react_step".to_string(),
                duration_ms: llm_duration,
                token_usage: crate::observability::TokenUsage::default(),
                tool_calls: traces.last().map(|t| if t.action.is_some() { 1 } else { 0 }).unwrap_or(0),
                llm_calls: 1,
                success: true,
                error: None,
            });
        }

        // 6. Max steps reached
        warn!("ReAct engine reached max_steps ({})", max_steps);
        let final_msg = self.format_limit_reached_msg(&traces);
        
        // Trace limit reached
        let tracer = self.tracer.write().await;
        tracer.trace_error(
            execution_id,
            &format!("Max steps ({}) reached", max_steps),
            true
        ).await;
        
        metrics.finish();
        let tracer = self.tracer.write().await;
        tracer.end_session(execution_id, false, Some(final_msg.clone())).await;
        
        Ok((final_msg, traces))
    }

    // Self-Reflection Implementation
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

// --- Enhanced Loop Detector ---

struct LoopDetector {
    exact_history: HashSet<(String, String)>,
    resource_usage: HashMap<String, HashMap<String, usize>>,
    window: VecDeque<String>,
    max_window: usize,
    similarity_threshold: f32,
    consecutive_repeats: usize,
}

impl LoopDetector {
    fn new() -> Self {
        Self {
            exact_history: HashSet::new(),
            resource_usage: HashMap::new(),
            window: VecDeque::new(),
            max_window: 5,
            similarity_threshold: 0.85,
            consecutive_repeats: 0,
        }
    }

    fn is_looping(&mut self, tool: &str, args: &str) -> bool {
        if self.exact_history.contains(&(tool.to_string(), args.to_string())) {
            self.consecutive_repeats += 1;
            if self.consecutive_repeats >= 2 {
                return true;
            }
        } else {
            self.consecutive_repeats = 0;
            self.exact_history.insert((tool.to_string(), args.to_string()));
        }

        if tool == "see" {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(args) {
                if let Some(path) = val.get("image_path").and_then(|v| v.as_str()) {
                    let counts = self.resource_usage.entry(tool.to_string()).or_default();
                    let count = counts.entry(path.to_string()).or_insert(0);
                    *count += 1;

                    if *count > 3 {
                        warn!("Resource loop detected for path: {}", path);
                        return true;
                    }
                }
            }
        }

        let action_str = format!("{} {}", tool, args);

        for prev in &self.window {
            if self.similarity(prev, &action_str) > self.similarity_threshold {
                warn!("Semantic loop detected: '{}' is similar to '{}'", action_str, prev);
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
        let set1: HashSet<&str> = s1.split_whitespace().collect();
        let set2: HashSet<&str> = s2.split_whitespace().collect();

        let intersection = set1.intersection(&set2).count();
        let union = set1.union(&set2).count();

        if union == 0 { return 0.0; }
        intersection as f32 / union as f32
    }
}

// --- Action Parser ---

struct ActionParser;

impl ActionParser {
    fn parse_fallback(text: &str) -> Option<(String, String)> {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"(?is)Action:\s*(?:use\s+)?(?P<name>[\w\-]+)\s*(?P<args>\{[\s\S]*?\})").expect("Invalid regex pattern");
        }
        RE.captures(text).map(|cap| (cap["name"].to_string(), cap["args"].trim().to_string()))
    }
}
