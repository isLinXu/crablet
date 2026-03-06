use crate::agent::swarm::{Swarm, SwarmAgent, AgentId, SwarmMessage};
use crate::cognitive::llm::LlmClient;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{info, warn};

pub struct DebateModerator {
    id: AgentId,
    swarm: Arc<Swarm>,
    #[allow(dead_code)]
    llm: Arc<Box<dyn LlmClient>>,
    participants: Vec<AgentId>,
    rounds: usize,
    
    // State
    requester_id: Option<AgentId>,
    active_task_id: Option<String>,
    topic: String,
    history: Vec<String>,
    current_round: usize,
    current_participant_idx: usize,
    is_active: bool,
}

impl DebateModerator {
    pub fn new(
        name: &str, 
        swarm: Arc<Swarm>, 
        llm: Arc<Box<dyn LlmClient>>, 
        participants: Vec<AgentId>, 
        rounds: usize
    ) -> Self {
        Self {
            id: AgentId::from_name(name),
            swarm,
            llm,
            participants,
            rounds,
            requester_id: None,
            active_task_id: None,
            topic: String::new(),
            history: Vec::new(),
            current_round: 0,
            current_participant_idx: 0,
            is_active: false,
        }
    }

    async fn process_next_turn(&mut self) -> Option<SwarmMessage> {
        if self.current_round >= self.rounds {
            self.is_active = false;
            let summary = self.summarize_debate().await;
            
            if let Some(requester) = &self.requester_id {
                let result_msg = SwarmMessage::Result {
                    task_id: self.active_task_id.clone().unwrap_or_default(),
                    content: summary.clone(),
                    payload: None,
                };
                info!("Debate finished. Sending result to requester: {}", requester.0);
                if let Err(e) = self.swarm.send(requester, result_msg, &self.id).await {
                     warn!("Failed to send result to requester {}: {}", requester.0, e);
                }
            } else {
                warn!("Debate finished but no requester to send result to.");
            }
            return None; // Already sent manually
        }

        let current_agent_id = &self.participants[self.current_participant_idx];
        
        let prompt = if self.history.is_empty() {
            format!("Topic: {}. \nPlease state your initial position.", self.topic)
        } else {
            let last_entry = self.history.last().unwrap();
            format!("Topic: {}. \nPrevious argument: \"{}\". \nPlease provide your counter-argument or perspective.", self.topic, last_entry)
        };

        let msg = SwarmMessage::Task {
            task_id: format!("{}-r{}-p{}", self.active_task_id.clone().unwrap_or_default(), self.current_round, self.current_participant_idx),
            description: prompt,
            context: vec![],
            payload: None,
        };

        info!("Moderator sending turn to {}", current_agent_id.0);
        if let Err(e) = self.swarm.send(current_agent_id, msg, &self.id).await {
            warn!("Failed to send to agent {}: {}", current_agent_id.0, e);
        }

        // Advance state
        self.current_participant_idx += 1;
        if self.current_participant_idx >= self.participants.len() {
            self.current_participant_idx = 0;
            self.current_round += 1;
        }

        None
    }

    async fn summarize_debate(&self) -> String {
        // Simple concatenation for now, ideally call LLM
        format!("Debate on '{}' finished after {} rounds.\nHistory:\n{}", 
            self.topic, 
            self.rounds,
            self.history.join("\n\n")
        )
    }
}

#[async_trait]
impl SwarmAgent for DebateModerator {
    fn id(&self) -> &AgentId {
        &self.id
    }

    fn name(&self) -> &str {
        &self.id.0
    }

    fn description(&self) -> &str {
        "Moderates a debate between agents."
    }

    async fn receive(&mut self, message: SwarmMessage, sender: AgentId) -> Option<SwarmMessage> {
        match message {
            SwarmMessage::Task { task_id, description, .. } => {
                // Start a new debate
                info!("Moderator received new debate request from {}: {}", sender.0, description);
                self.requester_id = Some(sender.clone());
                self.active_task_id = Some(task_id.clone());
                self.topic = description;
                self.history.clear();
                self.current_round = 0;
                self.current_participant_idx = 0;
                self.is_active = true;

                // Kick off the first turn
                self.process_next_turn().await;

                // Return acknowledgement
                Some(SwarmMessage::StatusUpdate {
                    task_id,
                    status: "Debate Started".to_string(),
                })
            },
            SwarmMessage::Result { content, .. } => {
                if !self.is_active {
                    return None;
                }
                
                info!("Received argument from {}: {}", sender.0, content);
                self.history.push(format!("{}: {}", sender.0, content));
                
                // Trigger next turn
                // If debate ends, process_next_turn returns Result.
                // But who does it return to? The sender (the participant).
                // Participants usually ignore Result messages.
                // We should probably send the final result to the ORIGINAL requester.
                // But we don't have that ID here easily.
                
                // If process_next_turn returns Some, it goes to sender.
                // If it returns None, nothing sent back.
                
                self.process_next_turn().await
            },
            _ => None,
        }
    }
}
