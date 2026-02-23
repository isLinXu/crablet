use crablet::channels::cli;
use crablet::config::Config;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Warning: Failed to load .env file: {}", e);
    }

    // Load Config early
    let config = Config::load().unwrap_or_else(|_| {
         // Fallback default
         Config {
             database_url: "sqlite:crablet.db?mode=rwc".to_string(),
             skills_dir: std::path::PathBuf::from("skills"),
             model_name: "gpt-4o-mini".to_string(),
             log_level: "info".to_string(),
             mcp_servers: std::collections::HashMap::new(),
             channels: Vec::new(),
             openai_api_key: None,
         }
    });

    // Initialize Telemetry (Logging + Tracing)
    crablet::telemetry::init_telemetry(&config.log_level)?;
    
    info!("🦀 Crablet v0.1.0 starting up...");
    
    // Start CLI
    if let Err(e) = cli::run(config).await {
        tracing::error!("Application error: {}", e);
    }
    
    // Cleanup Telemetry
    crablet::telemetry::shutdown_telemetry();
    
    Ok(())
}
