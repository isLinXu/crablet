use anyhow::Result;
use crate::agent::security::SecurityAuditAgent;
use crate::agent::swarm::{SwarmMessage, AgentId, SwarmAgent};
use crate::cognitive::router::CognitiveRouter;
use tracing::info;
use serde_json::Value;

pub async fn handle_audit(router: &CognitiveRouter, path: String, format: String) -> Result<()> {
    info!("Starting Security Audit on: {}", path);
    
    // Get LLM from System 2
    let llm = router.sys2.llm.clone();
    
    let mut agent = SecurityAuditAgent::new(llm);
    let agent_id = AgentId::new(); // Dummy ID for sender
    
    let msg = SwarmMessage::Task {
        task_id: uuid::Uuid::new_v4().to_string(),
        description: path.clone(),
        context: vec![],
        payload: None,
    };
    
    println!("🔍 Analyzing codebase at '{}'...", path);
    
    if let Some(response) = agent.receive(msg, agent_id).await {
        match response {
            SwarmMessage::Result { content, payload, .. } => {
                if format == "json" {
                    if let Some(p) = payload {
                        println!("{}", serde_json::to_string_pretty(&p)?);
                    } else {
                        // Create a JSON object if payload missing
                        let json_content = serde_json::json!({
                            "summary": content
                        });
                        println!("{}", serde_json::to_string_pretty(&json_content)?);
                    }
                } else {
                    println!("\n📊 Audit Report Summary:\n");
                    println!("{}", content);
                    
                    if let Some(Value::Object(map)) = payload {
                        if let Some(Value::Array(vulns)) = map.get("vulnerabilities") {
                            if !vulns.is_empty() {
                                println!("\n🛑 Found {} Vulnerabilities:\n", vulns.len());
                                for (idx, v) in vulns.iter().enumerate() {
                                    let file = v.get("file").and_then(|s| s.as_str()).unwrap_or("unknown");
                                    let line = v.get("line").and_then(|l| l.as_u64()).map(|l| l.to_string()).unwrap_or("-".to_string());
                                    let severity = v.get("severity").and_then(|s| s.as_str()).unwrap_or("Unknown");
                                    let desc = v.get("description").and_then(|s| s.as_str()).unwrap_or("");
                                    let suggestion = v.get("suggestion").and_then(|s| s.as_str()).unwrap_or("");
                                    
                                    println!("{}. [{}] {} : {}", idx + 1, severity, file, line);
                                    println!("   Issue: {}", desc);
                                    println!("   Fix:   {}\n", suggestion);
                                }
                            } else {
                                println!("✅ No vulnerabilities found.");
                            }
                        }
                    }
                }
            }
            SwarmMessage::Error { error, .. } => {
                eprintln!("❌ Audit failed: {}", error);
            }
            _ => {
                eprintln!("⚠️ Unexpected response type from agent.");
            }
        }
    } else {
        eprintln!("❌ Agent did not return a response.");
    }
    
    Ok(())
}
