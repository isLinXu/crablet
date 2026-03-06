use anyhow::Result;
use crablet::config::Config;
#[allow(unused_imports)]
use crablet::channels::cli;
use std::sync::Arc;
use tokio::time::Duration;

#[tokio::test]
async fn test_e2e_full_flow() -> Result<()> {
    // 1. Initialize Configuration
    let config = Config {
        database_url: "sqlite::memory:".to_string(), // Use in-memory DB for tests
        skills_dir: std::path::PathBuf::from("tests/fixtures/skills"),
        model_name: "gpt-4o-mini".to_string(),
        log_level: "debug".to_string(),
        mcp_servers: std::collections::HashMap::new(),
        channels: vec![],
        semantic_cache_threshold: 0.9,
        openai_api_key: Some("sk-test".to_string()), // Mock Key
        port: 3001, // Test port
        providers: std::collections::HashMap::new(),
        ollama_model: "llama3".to_string(),
        serper_api_key: None,
        feishu_app_id: None,
        feishu_app_secret: None,
        oidc_issuer: None,
        oidc_client_id: None,
        oidc_client_secret: None,
        jwt_secret: Some("test-secret".to_string()),
        bandit_exploration: 0.55,
        deliberate_threshold: 0.58,
        enable_adaptive_routing: false,
        enable_hierarchical_reasoning: true,
        meta_reasoning_threshold: 0.82,
        mcts_simulations: 24,
        mcts_exploration_weight: 1.2,
        graph_rag_entity_mode: "hybrid".to_string(),
        system2_threshold: 0.3,
        system3_threshold: 0.7,
    };

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
