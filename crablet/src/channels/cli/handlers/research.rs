use crate::agent::researcher::ResearchAgent;
use crate::agent::Agent;
use crate::cognitive::llm::LlmClient;
use anyhow::Result;
// use crate::cognitive::llm::{LlmClient, OpenAiClient};
use crate::cognitive::llm::cache::CachedLlmClient;
use colored::*;
use std::env;
use std::sync::Arc;

use crate::events::EventBus;

pub async fn handle_research(topic: String, depth: usize) -> Result<()> {
    println!(
        "{} {} (Depth: {})",
        "🦀 Crablet Research Mode:".cyan().bold(),
        topic,
        depth
    );

    // 1. Initialize LLM (System 2 Config)
    let model = env::var("OPENAI_MODEL_NAME").unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let llm_inner: Arc<dyn LlmClient> = match crate::cognitive::llm::OpenAiClient::new(&model) {
        Ok(client) => Arc::new(client) as Arc<dyn LlmClient>,
        Err(_) => {
            println!("{}", "Warning: OPENAI_API_KEY not found. Using Mock/Ollama for research might yield limited results.".yellow());
            let ollama_model =
                env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:14b".to_string());
            Arc::new(crate::cognitive::llm::OllamaClient::new(&ollama_model)) as Arc<dyn LlmClient>
        }
    };

    let llm = Arc::new(CachedLlmClient::new(llm_inner, 50)) as Arc<dyn LlmClient>;
    let event_bus = Arc::new(EventBus::new(100));

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
            .template("{spinner:.green} {msg}")?,
    );
    spinner.set_message(format!("Researching '{}'...", topic));
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    let result = agent.execute(&topic, &[]).await?;

    spinner.finish_and_clear();

    println!("\n{}", result);

    // Note: This handler likely needs refactoring to use AppContext or inject EventBus if it constructs agent directly.
    // However, cli usually uses AppContext.
    // Let's check imports.
    Ok(())
}
