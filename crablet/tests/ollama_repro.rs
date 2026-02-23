#[tokio::test]
async fn test_ollama_tool_call() -> anyhow::Result<()> {
    // Skip if OLLAMA_API_BASE is not reachable (basic check)
    let client = reqwest::Client::new();
    let base_url = "http://localhost:11434";
    if client.get(format!("{}/api/tags", base_url)).send().await.is_err() {
        println!("Ollama not running, skipping test");
        return Ok(());
    }

    use serde_json::json;

    let model = "qwen3:4b"; // User's model
    // Ensure model exists (optional, or pull it)
    
    // Construct a request with tools
    // Create a very long description to test truncation/buffer issues
    let long_desc = "A".repeat(10000); 
    
    let tools = vec![
        json!({
            "type": "function",
            "function": {
                "name": "calculator",
                "description": format!("Calculate math expressions. {}", long_desc),
                "parameters": {
                    "type": "object",
                    "properties": {
                        "expression": {
                            "type": "string",
                            "description": "The math expression"
                        }
                    },
                    "required": ["expression"]
                }
            }
        })
    ];

    let messages = vec![
        json!({
            "role": "user",
            "content": "Calculate 15 * 7"
        })
    ];

    let body = json!({
        "model": model,
        "messages": messages,
        "stream": false,
        "tools": tools
    });

    println!("Sending Request: {}", serde_json::to_string_pretty(&body)?);

    let res = client.post(format!("{}/api/chat", base_url))
        .json(&body)
        .send()
        .await?;

    let status = res.status();
    let text = res.text().await?;
    println!("Response Status: {}", status);
    println!("Response Body: {}", text);

    assert!(status.is_success(), "Ollama request failed: {}", text);
    
    Ok(())
}
