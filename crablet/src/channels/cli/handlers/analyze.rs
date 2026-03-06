use anyhow::Result;
use crate::agent::analyst_v2::DataAnalystAgent;
use crate::agent::swarm::{SwarmMessage, AgentId, SwarmAgent};
use crate::cognitive::router::CognitiveRouter;
use tracing::info;

pub async fn handle_analyze(router: &CognitiveRouter, path: String, goal: String) -> Result<()> {
    info!("Starting Data Analysis on: {}", path);
    
    // Get LLM from System 2
    let llm = router.sys2.llm.clone();
    
    // Use current directory as workspace
    let work_dir = std::env::current_dir()?;
    
    let mut agent = DataAnalystAgent::new(llm, work_dir);
    let agent_id = AgentId::new(); // Dummy ID for sender
    
    let payload = serde_json::json!({
        "file_path": path,
        "goal": goal
    });
    
    let msg = SwarmMessage::Task {
        task_id: uuid::Uuid::new_v4().to_string(),
        description: goal.clone(),
        context: vec![],
        payload: Some(payload),
    };
    
    println!("🔍 Analyzing data file '{}'...", path);
    println!("🎯 Goal: {}", goal);
    
    if let Some(response) = agent.receive(msg, agent_id).await {
        match response {
            SwarmMessage::Result { content, payload, .. } => {
                println!("\n📊 Analysis Result:\n");
                println!("{}", content);
                
                if let Some(p) = payload {
                    if let Some(code) = p.get("code_executed").and_then(|s| s.as_str()) {
                        println!("\n💻 Python Code Executed:\n");
                        println!("```python\n{}\n```", code);
                    }
                }
            }
            SwarmMessage::Error { error, .. } => {
                eprintln!("❌ Analysis failed: {}", error);
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
