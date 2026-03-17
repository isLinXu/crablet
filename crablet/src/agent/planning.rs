use anyhow::Result;
use std::sync::Arc;
use crate::cognitive::llm::LlmClient;
use crate::types::Message;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use crate::agent::{Agent, AgentRole};

#[derive(Debug, Serialize, Deserialize)]
pub struct SubTaskPlan {
    pub id: String,
    pub description: String,
    pub dependencies: Vec<String>,
    pub required_capabilities: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub subtasks: Vec<SubTaskPlan>,
    pub strategy: String,
}

#[derive(Clone)]
pub struct TaskPlanner {
    llm: Arc<Box<dyn LlmClient>>,
}

impl TaskPlanner {
    pub fn new(llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self { llm }
    }

    pub async fn decompose(&self, task_description: &str) -> Result<ExecutionPlan> {
        let prompt = format!(
            r#"You are an expert project manager and system architect.
Your goal is to decompose the following complex task into smaller, manageable subtasks for a swarm of autonomous agents.

Task: "{}"

Available Agent Capabilities:
- researcher: web search, information gathering, fact checking
- coder: writing code, debugging, software design (Python, Rust, etc.)
- analyst: data analysis, summarizing, logical reasoning, critical review
- drafter: writing content, drafting documents, reports, technical writing
- critic: reviewing, finding weaknesses, suggesting improvements, editing
- planner: breaking down tasks, project management, strategy
- reviewer: code review, content moderation, quality assurance

Output JSON format:
{{
  "subtasks": [
    {{
      "id": "task_1",
      "description": "Detailed description of what needs to be done",
      "dependencies": [], // IDs of tasks that must finish before this one
      "required_capabilities": ["researcher"] // One or more from available list
    }},
    {{
      "id": "task_2",
      "description": "...",
      "dependencies": ["task_1"],
      "required_capabilities": ["coder"]
    }}
  ],
  "strategy": "Brief explanation of the execution strategy"
}}

Ensure the plan is logical and dependencies are correct. Do not create cycles.
Return ONLY the JSON."#,
            task_description
        );

        let messages = vec![Message::system("You are a task planning engine. Output valid JSON only."), Message::user(prompt)];
        
        let response = self.llm.chat_complete(&messages).await?;
        
        // Clean up markdown code blocks if present
        let json_str = response.trim();
        let json_str = if json_str.starts_with("```json") {
            json_str.strip_prefix("```json").unwrap_or(json_str).strip_suffix("```").unwrap_or(json_str).trim()
        } else if json_str.starts_with("```") {
            json_str.strip_prefix("```").unwrap_or(json_str).strip_suffix("```").unwrap_or(json_str).trim()
        } else {
            json_str
        };

        let plan: ExecutionPlan = serde_json::from_str(json_str)?;
        Ok(plan)
    }
}

pub struct PlannerAgent {
    planner: TaskPlanner,
}

impl PlannerAgent {
    pub fn new(llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self {
            planner: TaskPlanner::new(llm),
        }
    }
}

#[async_trait]
impl Agent for PlannerAgent {
    fn name(&self) -> &str {
        "planner"
    }

    fn role(&self) -> AgentRole {
        AgentRole::Planner
    }

    fn description(&self) -> &str {
        "Decomposes complex tasks into actionable subtasks with dependencies."
    }

    async fn execute(&self, task: &str, _context: &[Message]) -> Result<String> {
        let plan = self.planner.decompose(task).await?;
        Ok(serde_json::to_string_pretty(&plan)?)
    }
}
