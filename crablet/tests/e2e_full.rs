use anyhow::Result;
use crablet::config::Config;
#[allow(unused_imports)]
use crablet::channels::cli;
use std::sync::Arc;
use tokio::time::Duration;

#[tokio::test]
async fn test_e2e_full_flow() -> Result<()> {
    // 1. Initialize Configuration
    let mut config = Config::default();
    config.database_url = "sqlite::memory:".to_string(); // Use in-memory DB for tests
    config.skills_dir = std::path::PathBuf::from("tests/fixtures/skills");
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

    // Ensure skills dir exists
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
    
    // 4. Test Web Server & Dashboard API
    println!("--- Testing Web API ---");
    let router = app.router.clone();
    let port = config.port;
    
    // Spawn server in background
    let server_handle = tokio::spawn(async move {
        if let Err(e) = crablet::channels::web::run(router, port, None).await {
            eprintln!("Server error: {}", e);
        }
    });
    
    // Wait for server to start
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Call Dashboard API
    let client = reqwest::Client::new();
    let resp = client.get(format!("http://localhost:{}/api/dashboard", port))
        .send()
        .await?;
        
    assert_eq!(resp.status(), 200);
    let json: serde_json::Value = resp.json().await?;
    println!("Dashboard Response: {}", json);
    
    assert_eq!(json["status"], "success");
    assert!(json["skills_count"].is_number());

    // 5. Test Knowledge API (Empty initially)
    let resp = client.get(format!("http://localhost:{}/api/knowledge", port))
        .send()
        .await?;
    assert_eq!(resp.status(), 200);
    
    // Cleanup
    server_handle.abort();
    tokio::fs::remove_dir_all(&config.skills_dir).await?;
    
    Ok(())
}
