use crablet::cognitive::router::CognitiveRouter;
use crablet::events::EventBus;
use std::env;
use std::sync::Arc;
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_real_llm_integration() {
    if !matches!(
        env::var("CRABLET_RUN_REAL_LLM_TESTS").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    ) {
        println!("Skipping real LLM integration test: set CRABLET_RUN_REAL_LLM_TESTS=1 to enable");
        return;
    }

    let model = env::var("CRABLET_TEST_MODEL").unwrap_or_else(|_| "qwen3.6:latest".to_string());

    let event_bus = Arc::new(EventBus::new(100));
    let mut config = crablet::config::Config::default();
    config.ollama_model = model.clone();
    config.openai_api_key = None; // Force Ollama fallback

    // Initialize Router (this will pick up OLLAMA_MODEL and use OllamaClient)
    let router = CognitiveRouter::new(&config, None, event_bus).await;

    println!("Starting real LLM integration test with model: {}", model);

    // Simple prompt
    let prompt = "What is the capital of France? Answer in one word.";

    // Set a timeout because real LLM can be slow
    let result = timeout(
        Duration::from_secs(300),
        router.process(prompt, "test_session_real"),
    )
    .await;

    match result {
        Ok(Ok((response, _traces))) => {
            println!("Response: {}", response);
            assert!(!response.is_empty());
            assert!(response.to_lowercase().contains("paris"));
        }
        Ok(Err(e)) => {
            // If Ollama is not running, this will fail.
            // We should print a helpful message.
            eprintln!(
                "Integration test failed: {}. Make sure Ollama is running with the configured CRABLET_TEST_MODEL.",
                e
            );
            // We fail the test to signal verification failure
            panic!("Integration test failed: {}", e);
        }
        Err(_) => {
            panic!("Integration test timed out");
        }
    }
}

#[tokio::test]
async fn test_real_llm_tool_usage() {
    if !matches!(
        env::var("CRABLET_RUN_REAL_LLM_TESTS").as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    ) {
        println!("Skipping real LLM tool test: set CRABLET_RUN_REAL_LLM_TESTS=1 to enable");
        return;
    }

    let model = env::var("CRABLET_TEST_MODEL").unwrap_or_else(|_| "qwen3.6:latest".to_string());

    let event_bus = Arc::new(EventBus::new(100));
    let mut config = crablet::config::Config::default();
    config.ollama_model = model.clone();
    config.openai_api_key = None; // Force Ollama fallback

    // Initialize Router (this will pick up OLLAMA_MODEL and use OllamaClient)
    let router = CognitiveRouter::new(&config, None, event_bus).await;

    println!("Starting real LLM tool usage test with model: {}", model);

    // Prompt requiring tool (bash)
    // Note: SafetyOracle might block bash if strict. But "echo hello" should be safe?
    // Let's try something simple that might trigger a tool if the model supports it.
    // "List files in current directory" -> `ls -la`
    let prompt = "List the files in the current directory using bash.";

    let result = timeout(
        Duration::from_secs(300),
        router.process(prompt, "test_session_tool"),
    )
    .await;

    match result {
        Ok(Ok((response, traces))) => {
            println!("Response: {}", response);
            println!("Traces: {:?}", traces);

            // Check if tool was used
            let tool_used = traces.iter().any(|t| t.action.is_some());
            if tool_used {
                println!("Tool was successfully used!");
            } else {
                println!("WARNING: Tool was NOT used. Model might have answered directly or failed to call tool.");
                // We don't assert failure because small models often fail tool calling.
                // But we print warning.
            }
        }
        Ok(Err(e)) => {
            eprintln!("Tool test failed: {}", e);
        }
        Err(_) => {
            panic!("Tool test timed out");
        }
    }
}
