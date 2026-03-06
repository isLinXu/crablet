use anyhow::Result;
use tracing::info;
use crate::cognitive::router::CognitiveRouter;
use crate::cognitive::lane::LaneRouter;
use std::io::{self, Write};

pub async fn handle_chat(lane_router: &LaneRouter, router: &CognitiveRouter, session: Option<&str>) -> Result<()> {
    let session_id = session.map(|s| s.to_string()).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    info!("Starting chat mode (Session: {})...", session_id);
    println!("╔════════════════════════════════════════════╗");
    println!("║  🦀 Crablet v0.1.0                         ║");
    println!("║  Session: {} ║", &session_id[0..8]);
    println!("║  Type 'exit' to quit                       ║");
    println!("║  Type '/help' for commands                 ║");
    println!("╚════════════════════════════════════════════╝");

    start_chat_loop(lane_router, router, &session_id).await
}

pub async fn handle_run(lane_router: &LaneRouter, prompt: &str, session: Option<&str>) -> Result<()> {
    let session_id = session.map(|s| s.to_string()).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let spinner = indicatif::ProgressBar::new_spinner();
    spinner.set_style(
        indicatif::ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")?
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    spinner.set_message("Thinking...");
    spinner.enable_steady_tick(std::time::Duration::from_millis(100));

    info!("Running prompt: {} (Session: {})", prompt, session_id);
    // Use Lane Router for dispatch
    let (response, _traces) = lane_router.dispatch(&session_id, prompt.to_string()).await?;

    spinner.finish_and_clear();
    println!("🦀 Crablet: {}", response);
    Ok(())
}

async fn start_chat_loop(lane_router: &LaneRouter, router: &CognitiveRouter, session_id: &str) -> Result<()> {
    let mut input = String::new();
    let stdin = io::stdin(); // Create stdin handle outside loop

    loop {
        print!("\n💬 You: ");
        io::stdout().flush()?;

        input.clear();
        stdin.read_line(&mut input)?; // Use handle

        let trimmed = input.trim();
        if trimmed == "exit" || trimmed == "/exit" {
            break;
        }

        if trimmed.is_empty() {
            continue;
        }

        let spinner = indicatif::ProgressBar::new_spinner();
        spinner.set_style(
            indicatif::ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")?
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        );
        spinner.set_message("Thinking...");
        spinner.enable_steady_tick(std::time::Duration::from_millis(100));

        // Process with Lane Router (which wraps Cognitive Router)
        match lane_router.dispatch(session_id, trimmed.to_string()).await {
            Ok((response, _traces)) => {
                spinner.finish_and_clear();
                println!("🦀 Crablet: {}", response);
            },
            Err(e) => {
                spinner.finish_and_clear();
                tracing::error!("Error: {}", e);
                println!("❌ Error: {}", e);

                // Interactive Configuration Prompt
                if e.to_string().contains("Ollama API returned error") || e.to_string().contains("LLM Initial Failure") {
                    println!("\n⚠️ It seems the local LLM service (Ollama) is not available or returned an error.");
                    println!("Would you like to switch to Cloud Model configuration? [y/N]");

                    let mut confirm = String::new();
                    io::stdin().read_line(&mut confirm)?;
                    if confirm.trim().to_lowercase() == "y" {
                        println!("Redirecting to configuration mode... (Please use 'crablet config' command manually for now)");
                        // Ideally we would call handle_config here, but we need Config struct
                    }
                }
            }
        }
    }

    // Trigger Memory Consolidation
    #[cfg(feature = "knowledge")]
    {
        println!("Consolidating memory...");
        if let Err(e) = router.consolidate_memory(session_id).await {
             tracing::warn!("Memory consolidation failed: {}", e);
        } else {
             println!("Memory consolidated.");
        }
    }

    Ok(())
}
