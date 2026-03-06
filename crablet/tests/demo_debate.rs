use anyhow::Result;
use std::sync::Arc;
use crablet::agent::swarm::{Swarm, SwarmAgent, AgentId, SwarmMessage};
use crablet::agent::debate::DebateModerator;
use crablet::cognitive::llm::{LlmClient, MockClient};

// Mock Agent for testing
struct MockParticipant {
    id: AgentId,
    name: String,
}

#[async_trait::async_trait]
impl SwarmAgent for MockParticipant {
    fn id(&self) -> &AgentId {
        &self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
    async fn receive(&mut self, message: SwarmMessage, _sender: AgentId) -> Option<SwarmMessage> {
        match message {
            SwarmMessage::Task { task_id, .. } => {
                Some(SwarmMessage::Result {
                    task_id,
                    content: format!("Opinion from {}", self.name),
                    payload: None,
                })
            }
            _ => None,
        }
    }
}

#[tokio::test]
async fn test_debate_flow() -> Result<()> {
    // 1. Setup Swarm
    let swarm = Arc::new(Swarm::new());
    
    // 2. Register Participants
    let p1 = MockParticipant { id: AgentId::from_name("Alice"), name: "Alice".to_string() };
    let p2 = MockParticipant { id: AgentId::from_name("Bob"), name: "Bob".to_string() };
    let p1_id = p1.id.clone();
    let p2_id = p2.id.clone();
    
    swarm.register_agent(Box::new(p1)).await;
    swarm.register_agent(Box::new(p2)).await;
    
    // 3. Setup Moderator
    let mock_llm = Arc::new(Box::new(MockClient) as Box<dyn LlmClient>);
    
    let moderator = DebateModerator::new(
        "Moderator",
        swarm.clone(),
        mock_llm,
        vec![p1_id, p2_id],
        2 // 2 rounds
    );
    
    swarm.register_agent(Box::new(moderator)).await;
    
    // 4. Register User Agent to receive result
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
    
    let user_agent = UserAgent { id: user_id.clone(), tx };
    swarm.register_agent(Box::new(user_agent)).await;
    
    // 5. Start Debate
    let start_msg = SwarmMessage::Task {
        task_id: "debate-1".to_string(),
        description: "Debate: Vim vs Emacs".to_string(),
        context: vec![],
        payload: None,
    };
    
    swarm.send(&AgentId::from_name("Moderator"), start_msg, &user_id).await?;
    
    // 6. Wait for result
    // Increase timeout to ensure rounds complete
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv()).await;
    
    assert!(result.is_ok(), "Debate timed out");
    let content = result.unwrap().unwrap();
    println!("Debate Result: {}", content);
    
    assert!(content.contains("Debate on 'Debate: Vim vs Emacs' finished"));
    assert!(content.contains("Opinion from Alice"));
    assert!(content.contains("Opinion from Bob"));
    
    Ok(())
}
