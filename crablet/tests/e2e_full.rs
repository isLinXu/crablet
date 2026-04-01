use anyhow::Result;
use crablet::config::Config;
use crablet::gateway::{CrabletGateway, types::GatewayConfig};
use axum::extract::State;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
async fn test_e2e_full_flow() -> Result<()> {
    std::env::set_var("OPENAI_API_KEY", "sk-test");

    let temp_dir = TempDir::new()?;
    let skills_dir = temp_dir.path().join("skills");

    // 1. Initialize Configuration
    let mut config = Config::default();
    config.database_url = "sqlite::memory:".to_string(); // Use in-memory DB for tests
    config.skills_dir = skills_dir.clone();
    config.model_name = "gpt-4o-mini".to_string();
    config.log_level = "debug".to_string();
    config.mcp_servers = std::collections::HashMap::new();
    config.channels = vec![];
    config.semantic_cache_threshold = 0.9;
    config.openai_api_key = Some("sk-test".to_string()); // Mock Key
    config.port = 3001; // Test port
    config.providers = std::collections::HashMap::new();
    config.ollama_model = "llama3".to_string();
    config.serper_api_key = None;
    config.feishu_app_id = None;
    config.feishu_app_secret = None;
    config.oidc_issuer = None;
    config.oidc_client_id = None;
    config.oidc_client_secret = None;
    config.jwt_secret = Some("test-secret".to_string());
    config.bandit_exploration = 0.55;
    config.deliberate_threshold = 0.58;
    config.enable_adaptive_routing = false;
    config.enable_hierarchical_reasoning = true;
    config.meta_reasoning_threshold = 0.82;
    config.mcts_simulations = 24;
    config.mcts_exploration_weight = 1.2;
    config.graph_rag_entity_mode = "hybrid".to_string();
    config.system2_threshold = 0.3;
    config.system3_threshold = 0.7;

    tokio::fs::create_dir_all(&config.skills_dir).await?;

    // 2. Initialize App Context
    let app = Arc::new(crablet::channels::cli::context::AppContext::new(config.clone()).await?);
    
    // 3. Test Security Audit (CLI Command)
    // Create a dummy vulnerable file
    let vuln_file = config.skills_dir.join("vuln.py");
    tokio::fs::write(&vuln_file, "password = '123456' # Hardcoded password").await?;
    
    // Run Audit Logic Directly (simulating CLI handler)
    println!("--- Testing Security Audit ---");
    crablet::channels::cli::handlers::audit::handle_audit(
        &app.router, 
        config.skills_dir.to_string_lossy().to_string(), 
        "json".to_string()
    ).await?;
    
    // 4. Test Gateway handlers without binding a real socket
    println!("--- Testing Web API ---");
    let cancel_token = tokio_util::sync::CancellationToken::new();
    let gateway = Arc::new(CrabletGateway::new(
        GatewayConfig {
            host: "127.0.0.1".to_string(),
            port: config.port,
            auth_mode: "off".to_string(),
        },
        app.router.clone(),
        cancel_token.clone(),
    ).await?);

    let json = crablet::gateway::web_handlers::get_dashboard_stats(State(gateway.clone()))
        .await
        .0;
    println!("Dashboard Response: {}", json);
    
    assert_eq!(json["status"], "healthy");
    assert!(json["skills_count"].is_number());

    // 5. Test Knowledge API (Empty initially)
    let docs = crablet::gateway::knowledge_handlers::list_documents(State(gateway))
        .await
        .0;
    assert_eq!(docs["status"], "success");
    assert!(docs["documents"].is_array());
    
    Ok(())
}
