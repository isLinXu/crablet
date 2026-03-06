use async_trait::async_trait;
use anyhow::Result;
use crate::types::{Message, TraceStep};
use super::{CognitiveMiddleware, MiddlewareState};
use tracing::{info, warn};
use tokio::spawn;
use tiktoken_rs::{cl100k_base, CoreBPE};

pub struct CostGuardMiddleware {
    tokenizer: Option<CoreBPE>,
}

impl Default for CostGuardMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl CostGuardMiddleware {
    pub fn new() -> Self {
        let tokenizer = cl100k_base().ok();
        if tokenizer.is_none() {
            warn!("CostGuard: Failed to initialize tokenizer. Token counting will be approximate.");
        }
        Self { tokenizer }
    }
}

#[async_trait]
impl CognitiveMiddleware for CostGuardMiddleware {
    fn name(&self) -> &str {
        "Cost Guard"
    }

    async fn execute(&self, input: &str, context: &mut Vec<Message>, state: &MiddlewareState) -> Result<Option<(String, Vec<TraceStep>)>> {
        // Use tiktoken for accurate token counting if available, else heuristic
        let input_tokens = if let Some(bpe) = &self.tokenizer {
            bpe.encode_with_special_tokens(input).len()
        } else {
            input.len() / 4
        };
        
        let context_tokens: usize = context.iter()
            .map(|m| {
                if let Some(bpe) = &self.tokenizer {
                    m.text().map(|s| bpe.encode_with_special_tokens(&s).len()).unwrap_or(0)
                } else {
                    m.text().map(|s| s.len() / 4).unwrap_or(0)
                }
            })
            .sum();
            
        let total_tokens = input_tokens + context_tokens;
        
        // Limits
        let soft_limit = 8000;
        let hard_limit = 16000;
        
        if total_tokens > soft_limit {
            // Strategy: Keep System (0) and last N messages. Summarize middle.
            if context.len() > 4 {
                let keep_count = 4;
                let split_idx = if context.len() > keep_count + 1 { context.len() - keep_count } else { 1 };
                
                // HARD LIMIT: Block and compress to ensure safety
                if total_tokens > hard_limit {
                    warn!("CostGuard: Hard limit exceeded ({}). Blocking for compression.", total_tokens);
                    
                    let mut new_context = Vec::new();
                    if let Some(sys) = context.first() {
                        new_context.push(sys.clone());
                    }
                    
                    let middle_messages = &context[1..split_idx];
                    let middle_text = middle_messages.iter()
                        .filter_map(|m| m.text())
                        .collect::<Vec<String>>()
                        .join("\n");
                        
                    let summary_prompt = format!("Summarize the following conversation history concisely in under 200 words, preserving key details and context:\n\n{}", middle_text);
                    let summary_msg_req = Message::new("user", &summary_prompt);
                    
                    match state.llm.chat_complete(&[summary_msg_req]).await {
                        Ok(summary) => {
                            let summary_msg = Message::new("system", format!("[Previous conversation summary: {}]", summary));
                            new_context.push(summary_msg);
                            info!("CostGuard: Compressed middle context into summary.");
                        },
                        Err(e) => {
                            warn!("CostGuard: Failed to summarize context: {}. Falling back to truncation.", e);
                            let summary_placeholder = Message::new("system", "[Context Truncated: Old messages removed for brevity]");
                            new_context.push(summary_placeholder);
                        }
                    }
                    
                    new_context.extend_from_slice(&context[split_idx..]);
                    *context = new_context;
                    info!("CostGuard: Context compressed to {} messages", context.len());
                    
                } else {
                    // SOFT LIMIT: Async compression (background) + Local Truncation
                    info!("CostGuard: Soft limit exceeded ({}). Triggering background summarization.", total_tokens);
                    
                    let llm = state.llm.clone();
                    let event_bus = state.event_bus.clone();
                    
                    // Capture text for background task
                    let middle_messages = context[1..split_idx].to_vec();
                    
                    spawn(async move {
                        let middle_text = middle_messages.iter()
                            .filter_map(|m| m.text())
                            .collect::<Vec<String>>()
                            .join("\n");
                            
                        let summary_prompt = format!("Summarize the following conversation history concisely in under 200 words:\n\n{}", middle_text);
                        match llm.chat_complete(&[Message::new("user", &summary_prompt)]).await {
                            Ok(summary) => {
                                event_bus.publish(crate::events::AgentEvent::SystemLog(
                                    format!("Background Summary Generated: {}", summary)
                                ));
                                // TODO: Emit specific MemoryUpdate event when architecture supports it
                            },
                            Err(e) => warn!("Background summarization failed: {}", e),
                        }
                    });
                    
                    // Truncate locally for this request
                    let mut new_context = Vec::new();
                    if let Some(sys) = context.first() {
                        new_context.push(sys.clone());
                    }
                    new_context.push(Message::new("system", "[System: Older conversation history is being summarized in background. Temporarily unavailable.]"));
                    new_context.extend_from_slice(&context[split_idx..]);
                    *context = new_context;
                }
            }
        }
        
        Ok(None)
    }
}
