use anyhow::Result;
use std::sync::Arc;
use crablet::agent::swarm::{Swarm, SwarmAgent, AgentId, SwarmMessage};
use crablet::agent::reviewer::ReviewerAgent;
use crablet::cognitive::llm::{LlmClient, MockClient};

#[tokio::test]
async fn test_reviewer() -> Result<()> {
    let swarm = Arc::new(Swarm::new());
    let mock_llm = Arc::new(Box::new(MockClient) as Box<dyn LlmClient>);
    
    let reviewer = ReviewerAgent::new("Reviewer", mock_llm);
    let reviewer_id = reviewer.id().clone();
    
    swarm.register_agent(Box::new(reviewer)).await;
    
    // User Agent
    let user_id = AgentId::from_name("User");
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    
    struct UserAgent {
        id: AgentId,
        tx: tokio::sync::mpsc::Sender<String>,
    }
    
    #[async_trait::async_trait]
    impl SwarmAgent for UserAgent {
        fn id(&self) -> &AgentId { &self.id }
        fn name(&self) -> &str { "User" }
        async fn receive(&mut self, msg: SwarmMessage, _sender: AgentId) -> Option<SwarmMessage> {
            if let SwarmMessage::Result { content, .. } = msg {
                let _ = self.tx.send(content).await;
            }
            None
        }
    }
    
    swarm.register_agent(Box::new(UserAgent { id: user_id.clone(), tx })).await;
    
    let msg = SwarmMessage::Task {
        task_id: "review-1".to_string(),
        description: "fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
        context: vec![],
        payload: None,
    };
    
    swarm.send(&reviewer_id, msg, &user_id).await?;
    
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await;
    assert!(result.is_ok());
    
    let content = result.unwrap().unwrap();
    println!("Review Result: {}", content);
    
    Ok(())
}
