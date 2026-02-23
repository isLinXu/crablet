use std::sync::Arc;
use anyhow::Result;
use crate::agent::{SharedAgent, AgentConfig, AgentRole};
use crate::agent::researcher::ResearchAgent;
use crate::cognitive::llm::LlmClient;
use crate::events::EventBus;

pub struct AgentFactory {
    llm: Arc<Box<dyn LlmClient>>,
    event_bus: Arc<EventBus>,
}

impl AgentFactory {
    pub fn new(llm: Arc<Box<dyn LlmClient>>, event_bus: Arc<EventBus>) -> Self {
        Self { llm, event_bus }
    }

    pub fn create_agent(&self, config: &AgentConfig) -> Result<SharedAgent> {
        match config.role {
            AgentRole::Researcher => {
                let agent = ResearchAgent::new(self.llm.clone(), self.event_bus.clone());
                Ok(Arc::new(agent))
            },
            _ => {
                // For now, only Researcher is implemented.
                // Others will fallback to a generic agent or return error.
                Err(anyhow::anyhow!("Agent role {:?} not yet implemented", config.role))
            }
        }
    }
    
    pub fn create_from_yaml(&self, yaml_content: &str) -> Result<SharedAgent> {
        let config: AgentConfig = serde_yaml::from_str(yaml_content)?;
        self.create_agent(&config)
    }
}
