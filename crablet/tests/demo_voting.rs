use anyhow::Result;
use std::sync::Arc;
use crablet::agent::swarm::{Swarm, SwarmAgent, AgentId, SwarmMessage};
use crablet::agent::voting::VotingAgent;
use crablet::cognitive::llm::{LlmClient, MockClient};

struct MockVoter {
    id: AgentId,
    name: String,
    vote: String,
}

#[async_trait::async_trait]
impl SwarmAgent for MockVoter {
    fn id(&self) -> &AgentId { &self.id }
    fn name(&self) -> &str { &self.name }
    async fn receive(&mut self, message: SwarmMessage, _sender: AgentId) -> Option<SwarmMessage> {
        match message {
            SwarmMessage::Task { task_id, .. } => {
                Some(SwarmMessage::Result {
                    task_id,
                    content: self.vote.clone(),
                    payload: None,
                })
            }
            _ => None,
        }
    }
}

#[tokio::test]
async fn test_voting_flow() -> Result<()> {
    let swarm = Arc::new(Swarm::new());
    
    // 3 Voters: 2 Yes, 1 No
    let v1 = MockVoter { id: AgentId::from_name("V1"), name: "V1".to_string(), vote: "YES, I agree".to_string() };
    let v2 = MockVoter { id: AgentId::from_name("V2"), name: "V2".to_string(), vote: "YES, sounds good".to_string() };
    let v3 = MockVoter { id: AgentId::from_name("V3"), name: "V3".to_string(), vote: "NO, risky".to_string() };
    
    let v1_id = v1.id.clone();
    let v2_id = v2.id.clone();
    let v3_id = v3.id.clone();
    
    swarm.register_agent(Box::new(v1)).await;
    swarm.register_agent(Box::new(v2)).await;
    swarm.register_agent(Box::new(v3)).await;
    
    let mock_llm = Arc::new(Box::new(MockClient) as Box<dyn LlmClient>);
    
    let voting_agent = VotingAgent::new(
        "Chairperson",
        swarm.clone(),
        mock_llm,
        vec![v1_id, v2_id, v3_id]
    );
    
    swarm.register_agent(Box::new(voting_agent)).await;
    
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
    
    // Start Vote
    let start_msg = SwarmMessage::Task {
        task_id: "vote-1".to_string(),
        description: "Deploy to Production?".to_string(),
        context: vec![],
        payload: None,
    };
    
    swarm.send(&AgentId::from_name("Chairperson"), start_msg, &user_id).await?;
    
    // Wait for result
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await;
    assert!(result.is_ok());
    
    let content = result.unwrap().unwrap();
    println!("Vote Result: {}", content);
    
    assert!(content.contains("2 YES"));
    assert!(content.contains("1 NO"));
    
    Ok(())
}
