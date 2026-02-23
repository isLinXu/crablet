use anyhow::Result;
use crate::agent::researcher::ResearchAgent;
use crate::agent::Agent;
use crate::cognitive::llm::{LlmClient, OpenAiClient};
use crate::cognitive::llm::cache::CachedLlmClient;
use std::env;
use std::sync::Arc;
use colored::*;

use crate::events::EventBus;

pub async fn handle_research(topic: &str, depth: usize) -> Result<()> {
    println!("{} {} (Depth: {})", "🦀 Crablet Research Mode:".cyan().bold(), topic, depth);
    
    // 1. Initialize LLM (System 2 Config)
    let model = env::var("OPENAI_MODEL_NAME")
        .unwrap_or_else(|_| "gpt-4o-mini".to_string());
        
    let llm_inner: Box<dyn LlmClient> = match OpenAiClient::new(&model) {
        Ok(client) => Box::new(client),
        Err(_) => {
            println!("{}", "Warning: OPENAI_API_KEY not found. Using Mock/Ollama for research might yield limited results.".yellow());
            let ollama_model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen3:4b".to_string());
            Box::new(crate::cognitive::llm::OllamaClient::new(&ollama_model))
        }
    };
    
    let llm = Arc::new(Box::new(CachedLlmClient::new(llm_inner, 50)) as Box<dyn LlmClient>);
    let event_bus = Arc::new(EventBus::new());
    
    // 2. Initialize Research Agent
    let agent = ResearchAgent::new(llm.clone(), event_bus.clone());
    
    // 3. Execute
    // Note: The current ResearchAgent.execute() is single-pass. 
    // To support 'depth', we would need to enhance ResearchAgent to iterate.
    // For now, we just pass the topic.
    
    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_style(
        indicatif::ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    spinner.set_message(format!("Researching '{}'...", topic));
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));
    
    let result = agent.execute(topic, &[]).await?;
    
    spinner.finish_and_clear();
    
    println!("\n{}", result);
    
    Ok(())
}
