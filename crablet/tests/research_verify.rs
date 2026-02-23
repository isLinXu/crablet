use anyhow::Result;
use crablet::agent::researcher::ResearchAgent;
use crablet::agent::Agent;
use crablet::cognitive::llm::LlmClient;
use crablet::types::{Message, ContentPart};
use async_trait::async_trait;
use std::sync::Arc;
use serde_json::json;

// Mock LLM for Research
struct MockResearchLlm;

#[async_trait]
impl LlmClient for MockResearchLlm {
    async fn chat_complete(&self, messages: &[Message]) -> Result<String> {
        let last_msg = messages.last().unwrap();
        let content = last_msg.text().unwrap_or("".to_string());
        
        // 1. Generate Queries
        if content.contains("Generate 3 distinct search queries") {
            return Ok("- Rust Async Runtime Performance\n- Tokio vs Async-std benchmarks\n- Rust future polling model".to_string());
        }
        
        // 2. Summarize Findings
        if content.contains("write a comprehensive summary") {
            return Ok("Rust's async model is based on polling. Tokio is the most popular runtime. Benchmarks show high throughput.".to_string());
        }
        
        Ok("".to_string())
    }

    async fn chat_complete_with_tools(&self, messages: &[Message], _tools: &[serde_json::Value]) -> Result<Message> {
        let text = self.chat_complete(messages).await?;
        Ok(Message::new("assistant", &text))
    }
}

#[tokio::test]
async fn test_deep_research_agent() -> Result<()> {
    // 1. Initialize Research Agent with Mock LLM
    // Note: We need to mock WebSearchTool too, but WebSearchTool uses a real client or returns empty if not configured.
    // For unit testing logic flow, we rely on LLM mocking.
    // The ResearchAgent.execute() calls self.search.search(q).
    // WebSearchTool currently makes real HTTP requests if API key present, or returns empty list.
    // If it returns empty list, the summary prompt will just have empty findings.
    // The Mock LLM should handle empty findings gracefully or we assume WebSearchTool is mocked.
    // Since WebSearchTool is a struct, not a trait, mocking it is hard without refactoring.
    // However, for this test, we just want to verify the Agent's orchestration logic (LLM calls).
    
    let llm = Arc::new(Box::new(MockResearchLlm) as Box<dyn LlmClient>);
    let agent = ResearchAgent::new(llm);
    
    // 2. Execute Task
    let task = "Research Rust Async Performance";
    let result = agent.execute(task, &[]).await?;
    
    // 3. Verify Result
    println!("Research Result:\n{}", result);
    assert!(result.contains("**Research Report**"));
    assert!(result.contains("Rust's async model"));
    
    Ok(())
}
