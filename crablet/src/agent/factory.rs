use std::sync::Arc;
use anyhow::Result;
use crate::agent::{SharedAgent, AgentConfig, AgentRole};
use crate::agent::researcher::ResearchAgent;
use crate::agent::planning::PlannerAgent;
use crate::agent::generic::GenericAgent;
use crate::cognitive::llm::LlmClient;
use crate::events::EventBus;

#[derive(Clone)]
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
            AgentRole::Planner => {
                let agent = PlannerAgent::new(self.llm.clone());
                Ok(Arc::new(agent))
            },
            _ => {
                // Fallback to Generic Agent
                let agent = GenericAgent::new(config.role.clone(), self.llm.clone(), &config.system_prompt);
                Ok(Arc::new(agent))
            }
        }
    }
    
    pub fn create_agent_by_role(&self, role_str: &str) -> Result<SharedAgent> {
        let role_str_lower = role_str.to_lowercase();
        let role = match role_str_lower.as_str() {
            "researcher" => AgentRole::Researcher,
            "planner" => AgentRole::Planner,
            "coder" => AgentRole::Coder,
            "analyst" => AgentRole::Analyst,
            "reviewer" => AgentRole::Reviewer,
            "drafter" => AgentRole::Drafter,
            "critic" => AgentRole::Critic,
            "security" => AgentRole::Custom("Security".to_string()),
            _ => AgentRole::Custom(role_str.to_string()),
        };
        
        let system_prompt = match role_str_lower.as_str() {
            "coder" => "You are an expert software engineer. Write clean, efficient, and well-documented code. Follow best practices and handle errors gracefully.".to_string(),
            "analyst" => "You are a data analyst. Analyze the provided information, identify patterns, and draw insights. Be objective and data-driven.".to_string(),
            "reviewer" => "You are a code reviewer and content moderator. Review the input for accuracy, quality, style, and safety. Point out issues and suggest improvements.".to_string(),
            "drafter" => "You are a professional content creator and technical writer. Your goal is to draft high-quality documents, reports, or code according to requirements. Focus on clarity, structure, and thoroughness.".to_string(),
            "critic" => "You are a meticulous critic and editor. Your job is to find weaknesses, gaps, or errors in a draft. Provide constructive but sharp feedback to improve the quality to the highest standard.".to_string(),
            "security" => "You are a security expert. Analyze the code or design for vulnerabilities and security risks. Recommend mitigation strategies.".to_string(),
            _ => format!("You are a helpful assistant with the role of {}.", role_str),
        };
        
        match role {
            AgentRole::Researcher => {
                let agent = ResearchAgent::new(self.llm.clone(), self.event_bus.clone());
                Ok(Arc::new(agent))
            },
            AgentRole::Planner => {
                let agent = PlannerAgent::new(self.llm.clone());
                Ok(Arc::new(agent))
            },
            _ => {
                let agent = GenericAgent::new(role, self.llm.clone(), &system_prompt);
                Ok(Arc::new(agent))
            }
        }
    }
    
    pub fn create_from_yaml(&self, yaml_content: &str) -> Result<SharedAgent> {
        let config: AgentConfig = serde_yaml::from_str(yaml_content)?;
        self.create_agent(&config)
    }
}
