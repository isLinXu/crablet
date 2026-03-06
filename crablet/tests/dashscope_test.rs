use crablet::cognitive::router::CognitiveRouter;
use crablet::events::EventBus;
use std::sync::Arc;
use std::env;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_dashscope_integration() {
    // Load .env file if present
    let _ = dotenvy::dotenv();

    // Check if required env vars are present
    if env::var("DASHSCOPE_API_KEY").is_err() {
        println!("Skipping DashScope test: DASHSCOPE_API_KEY not set");
        return;
    }

    // Ensure we are using DashScope config
    // (If running from IDE/Cargo with .env loaded, these should be set)
    // We print them to verify (masking key)
    let base_url = env::var("OPENAI_API_BASE").unwrap_or_default();
    println!("Using API Base: {}", base_url);
    
    let event_bus = Arc::new(EventBus::new(100));
    
    // 2. Initialize Router
    let config = crablet::config::Config::default();
    let router = CognitiveRouter::new(&config, None, event_bus).await;
    
    println!("Starting DashScope integration test...");

    // 3. Send Request
    // Use [FORCE_CLOUD] to ensure we hit System 2 (Cloud)
    let prompt = "[FORCE_CLOUD] What is the capital of China? Answer in one word.";
    
    // Set a timeout
    let result = timeout(Duration::from_secs(30), router.process(prompt, "test_session_dashscope")).await;

    match result {
        Ok(Ok((response, _traces))) => {
            println!("Response: {}", response);
            assert!(!response.is_empty());
            // Check for "Beijing" (case insensitive)
            assert!(response.to_lowercase().contains("beijing"));
        }
        Ok(Err(e)) => {
            eprintln!("DashScope test failed: {}", e);
            panic!("DashScope test failed: {}", e);
        }
        Err(_) => {
            panic!("DashScope test timed out");
        }
    }
}
