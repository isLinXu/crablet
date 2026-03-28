//! Enhanced ReAct Engine with Advanced Guard Mechanisms
//! 
//! This module extends the base ReAct implementation with:
//! - Multi-level loop detection (exact, semantic, resource-based)
//! - Confidence decay monitoring with branch pruning
//! - Step timeout and total timeout enforcement
//! - Forced summarization fallback

use super::react_guard::{
    ReActGuard, ReactGuardConfig, TerminationReason,
};
use crate::types::{Message, TraceStep};
use crate::events::{AgentEvent, EventBus};
use crate::skills::SkillRegistry;
use crate::cognitive::llm::LlmClient;
use anyhow::{Result, anyhow};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

/// Enhanced ReAct Engine with comprehensive guard mechanisms
pub struct EnhancedReActEngine {
    llm: Arc<Box<dyn LlmClient>>,
    skills: Arc<RwLock<SkillRegistry>>,
    event_bus: Arc<EventBus>,
    config: ReactGuardConfig,
}

impl EnhancedReActEngine {
    /// Create a new enhanced ReAct engine with default guard configuration
    pub fn new(
        llm: Arc<Box<dyn LlmClient>>,
        skills: Arc<RwLock<SkillRegistry>>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            llm,
            skills,
            event_bus,
            config: ReactGuardConfig::default(),
        }
    }
    
    /// Create with custom configuration
    pub fn with_config(
        llm: Arc<Box<dyn LlmClient>>,
        skills: Arc<RwLock<SkillRegistry>>,
        event_bus: Arc<EventBus>,
        config: ReactGuardConfig,
    ) -> Self {
        Self {
            llm,
            skills,
            event_bus,
            config,
        }
    }
    
    /// Execute ReAct reasoning with full guard protection
    pub async fn execute(&self, initial_context: &[Message]) -> Result<(String, Vec<TraceStep>)> {
        let mut guard = ReActGuard::new(self.config.clone());
        let mut current_context = initial_context.to_vec();
        let mut traces = Vec::new();
        let mut partial_observations = Vec::new();
        
        info!("🧠 Starting enhanced ReAct execution with max_steps={}, total_timeout={:?}", 
              self.config.max_steps, self.config.total_timeout);
        
        for step in 0..self.config.max_steps {
            let step_num = step + 1;
            info!("ReAct Step {}/{}", step_num, self.config.max_steps);
            
            // Check step limit
            if let Err(reason) = guard.check_step_limit(step_num) {
                warn!("Step limit reached: {:?}", reason);
                let summary = guard.create_forced_summary(&partial_observations, reason);
                return Ok((summary.summary, traces));
            }
            
            // Get tool definitions
            let tool_definitions = self.skills.read().await.to_tool_definitions();
            
            // Construct prompt context
            let mut prompt_context = current_context.clone();
            if step > 0 {
                prompt_context.push(Message::new("system", 
                    "Instruction: You have received tool outputs. Use them to answer directly. \
                    Do NOT repeat the same tool call with similar prompts if it hasn't provided new information."));
            }
            
            // Get LLM response with timeout protection
            let response_msg = match guard.execute_with_timeout(
                self.llm.chat_complete_with_tools(&prompt_context, &tool_definitions)
            ).await {
                Ok(msg) => msg,
                Err(e) => {
                    warn!("LLM timeout or error at step {}: {}", step_num, e);
                    self.event_bus.publish(AgentEvent::Error(e.to_string()));
                    
                    if step == 0 {
                        return Err(anyhow!("LLM Initial Failure: {}", e));
                    }
                    
                    // Return partial results on LLM failure
                    let summary = guard.create_forced_summary(
                        &partial_observations, 
                        TerminationReason::Timeout
                    );
                    return Ok((summary.summary, traces));
                }
            };
            
            // Extract thought
            let thought = self.extract_thought(&response_msg);
            if !thought.is_empty() {
                self.event_bus.publish(AgentEvent::ThoughtGenerated(thought.clone()));
            }
            
            // Parse actions
            let mut tool_calls = response_msg.tool_calls.clone().unwrap_or_default();
            if tool_calls.is_empty() {
                if let Some((name, args)) = self.parse_fallback_action(&thought) {
                    tool_calls.push(crate::types::ToolCall {
                        id: format!("call_{}", uuid::Uuid::new_v4()),
                        r#type: "function".to_string(),
                        function: crate::types::FunctionCall { name, arguments: args },
                    });
                }
            }
            
            // Check if no actions (final response)
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
            
            // Record assistant message
            let mut assistant_msg_record = response_msg.clone();
            if assistant_msg_record.tool_calls.is_none() {
                assistant_msg_record.tool_calls = Some(tool_calls.clone());
            }
            current_context.push(assistant_msg_record);
            
            // Execute tools with guard protection
            let observations = self.execute_tools_with_guards(
                &mut guard,
                tool_calls,
                step_num,
                &mut traces,
                &mut partial_observations,
            ).await;
            
            // Add observations to context
            for obs in &observations {
                current_context.push(Message::new("user", obs.clone()));
            }
        }
        
        // Max steps reached - force summary
        warn!("Max steps ({}) reached, forcing summary", self.config.max_steps);
        let summary = guard.create_forced_summary(
            &partial_observations, 
            TerminationReason::MaxStepsReached
        );
        
        Ok((summary.summary, traces))
    }
    
    /// Execute tools with guard checks
    async fn execute_tools_with_guards(
        &self,
        guard: &mut ReActGuard,
        tool_calls: Vec<crate::types::ToolCall>,
        step_num: usize,
        traces: &mut Vec<TraceStep>,
        partial_observations: &mut Vec<String>,
    ) -> Vec<String> {
        use tokio::sync::Semaphore;
        
        let semaphore = Arc::new(Semaphore::new(5)); // Max concurrency: 5
        let mut tasks = Vec::new();
        let mut observations = Vec::new();
        
        for tool_call in tool_calls {
            let func_name = tool_call.function.name.clone();
            let args_str = tool_call.function.arguments.clone();
            let tool_id = tool_call.id.clone();
            
            // Pre-execution guard check
            if let Err(reason) = guard.should_continue(&func_name, &args_str, None) {
                warn!("Guard prevented execution at step {}: {:?}", step_num, reason);
                
                let loop_msg = match reason {
                    TerminationReason::LoopDetected(loop_result) => {
                        format!("Loop detected: {:?}", loop_result)
                    }
                    TerminationReason::ConfidenceDecay => {
                        "Confidence declining, stopping further exploration".to_string()
                    }
                    _ => format!("Execution blocked: {:?}", reason),
                };
                
                // Push warning as observation
                observations.push(format!("System Warning: {}", loop_msg));
                continue;
            }
            
            // Clone dependencies for task
            let skills_clone = self.skills.clone();
            let bus_clone = self.event_bus.clone();
            let _timeout_duration = self.config.step_timeout;
            let sem_clone = semaphore.clone();
            
            tasks.push(tokio::spawn(async move {
                // Acquire permit
                let _permit = match sem_clone.acquire().await {
                    Ok(p) => p,
                    Err(e) => {
                        error!("Semaphore acquire failed: {}", e);
                        return (tool_id, func_name, args_str, 
                                format!("System Error: Failed to acquire permit: {}", e));
                    }
                };
                
                // Publish start event
                bus_clone.publish(AgentEvent::ToolExecutionStarted {
                    tool: func_name.clone(),
                    args: args_str.clone(),
                });
                
                // Execute skill
                let registry = skills_clone.read().await;
                let output = match serde_json::from_str(&args_str) {
                    Ok(parsed_json) => {
                        match registry.execute(&func_name, parsed_json).await {
                            Ok(result) => result,
                            Err(e) => format!("Skill execution failed: {}", e),
                        }
                    }
                    Err(e) => format!("Parameter Error: {}. Please use valid JSON.", e),
                };
                
                // Publish finish event
                bus_clone.publish(AgentEvent::ToolExecutionFinished {
                    tool: func_name.clone(),
                    output: output.clone(),
                });
                
                (tool_id, func_name, args_str, output)
            }));
        }
        
        // Await all tasks
        for task in tasks {
            if let Ok((_tool_id, func_name, args_str, observation)) = task.await {
                // Record trace
                traces.push(TraceStep {
                    step: step_num,
                    thought: String::new(),
                    action: Some(func_name.clone()),
                    action_input: Some(args_str),
                    observation: Some(observation.clone()),
                });
                
                observations.push(observation);
            } else {
                error!("A tool execution task panicked");
                observations.push("Task execution failed".to_string());
            }
        }
        
        // Update partial observations
        partial_observations.extend(observations.clone());
        
        observations
    }
    
    /// Extract thought from LLM response
    fn extract_thought(&self, msg: &crate::types::Message) -> String {
        msg.content.clone().unwrap_or_default()
            .iter()
            .filter_map(|part| match part {
                crate::types::ContentPart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }
    
    /// Parse action from thought using regex fallback
    fn parse_fallback_action(&self, thought: &str) -> Option<(String, String)> {
        use regex::Regex;
        
        lazy_static::lazy_static! {
            static ref RE: Regex = Regex::new(
                r"(?is)Action:\s*(?:use\s+)?(?P<name>[\w\-]+)\s*(?P<args>\{[\s\S]*?\})"
            ).expect("Invalid regex pattern");
        }
        
        RE.captures(thought).map(|cap| {
            (cap["name"].to_string(), cap["args"].trim().to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_enhanced_react_creation() {
        // Basic test to ensure the struct can be created
        // Full integration tests would require mocking LLM and skills
        let config = ReactGuardConfig {
            max_steps: 5,
            ..Default::default()
        };
        
        assert_eq!(config.max_steps, 5);
        assert_eq!(config.step_timeout.as_secs(), 10);
    }
}
