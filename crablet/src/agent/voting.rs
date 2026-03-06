use crate::agent::swarm::{Swarm, SwarmAgent, AgentId, SwarmMessage};
use crate::cognitive::llm::LlmClient;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{info, warn};
use std::collections::HashMap;

pub struct VotingAgent {
    id: AgentId,
    swarm: Arc<Swarm>,
    #[allow(dead_code)]
    llm: Arc<Box<dyn LlmClient>>,
    voters: Vec<AgentId>,
    
    // State
    requester_id: Option<AgentId>,
    active_task_id: Option<String>,
    proposal: String,
    votes: HashMap<String, String>, // voter_name -> vote_content
    is_active: bool,
}

impl VotingAgent {
    pub fn new(
        name: &str, 
        swarm: Arc<Swarm>, 
        llm: Arc<Box<dyn LlmClient>>, 
        voters: Vec<AgentId>
    ) -> Self {
        Self {
            id: AgentId::from_name(name),
            swarm,
            llm,
            voters,
            requester_id: None,
            active_task_id: None,
            proposal: String::new(),
            votes: HashMap::new(),
            is_active: false,
        }
    }

    async fn broadcast_proposal(&self) {
        let msg = SwarmMessage::Task {
            task_id: self.active_task_id.clone().unwrap_or_default(),
            description: format!("Vote on the following proposal: '{}'. Reply with YES/NO and a brief reason.", self.proposal),
            context: vec![],
            payload: None,
        };

        for voter in &self.voters {
            info!("VotingAgent asking {} for vote", voter.0);
            if let Err(e) = self.swarm.send(voter, msg.clone(), &self.id).await {
                warn!("Failed to send vote request to {}: {}", voter.0, e);
            }
        }
    }

    async fn check_consensus(&mut self) {
        if self.votes.len() >= self.voters.len() {
            self.is_active = false;
            let result = self.tally_votes();
            
            if let Some(requester) = &self.requester_id {
                let result_msg = SwarmMessage::Result {
                    task_id: self.active_task_id.clone().unwrap_or_default(),
                    content: result.clone(),
                    payload: None,
                };
                info!("Voting finished. Sending result to requester: {}", requester.0);
                if let Err(e) = self.swarm.send(requester, result_msg, &self.id).await {
                     warn!("Failed to send result to requester {}: {}", requester.0, e);
                }
            } else {
                info!("Voting finished: {}", result);
            }
        }
    }

    fn tally_votes(&self) -> String {
        let mut yes = 0;
        let mut no = 0;
        let mut details = String::new();

        for (voter, vote) in &self.votes {
            let lower = vote.to_lowercase();
            if lower.contains("yes") {
                yes += 1;
            } else if lower.contains("no") {
                no += 1;
            }
            details.push_str(&format!("- {}: {}\n", voter, vote));
        }

        format!("Voting Result: {} YES, {} NO.\nDetails:\n{}", yes, no, details)
    }
}

#[async_trait]
impl SwarmAgent for VotingAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }

    fn name(&self) -> &str {
        &self.id.0
    }

    fn description(&self) -> &str {
        "Facilitates voting on a proposal."
    }

    async fn receive(&mut self, message: SwarmMessage, sender: AgentId) -> Option<SwarmMessage> {
        match message {
            SwarmMessage::Task { task_id, description, .. } => {
                info!("VotingAgent received new proposal from {}: {}", sender.0, description);
                self.requester_id = Some(sender.clone());
                self.active_task_id = Some(task_id.clone());
                self.proposal = description;
                self.votes.clear();
                self.is_active = true;

                self.broadcast_proposal().await;

                Some(SwarmMessage::StatusUpdate {
                    task_id,
                    status: "Voting Started".to_string(),
                })
            },
            SwarmMessage::Result { content, .. } => {
                if !self.is_active {
                    return None;
                }
                
                // Only count votes from registered voters
                if self.voters.contains(&sender) {
                    info!("Received vote from {}: {}", sender.0, content);
                    self.votes.insert(sender.0.clone(), content);
                    self.check_consensus().await;
                } else {
                    warn!("Received vote from unauthorized agent: {}", sender.0);
                }
                
                None
            },
            _ => None,
        }
    }
}
