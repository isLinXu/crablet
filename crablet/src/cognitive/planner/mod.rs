use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::cognitive::llm::LlmClient;
use crate::types::Message;
use std::sync::Arc;
use tracing::info;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubTask {
    pub id: usize,
    pub description: String,
    pub tool_needed: Option<String>, // e.g. "search", "calculator"
    pub status: TaskStatus,
    pub result: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Plan {
    pub goal: String,
    pub tasks: Vec<SubTask>,
}

pub struct TaskPlanner {
    llm: Arc<Box<dyn LlmClient>>,
}

impl TaskPlanner {
    pub fn new(llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self { llm }
    }

    pub async fn create_plan(&self, goal: &str) -> Result<Plan> {
        info!("Creating plan for goal: {}", goal);
        
        let prompt = format!(
            "You are an expert task planner. Break down the following goal into a sequence of subtasks.\n\
            Goal: \"{}\"\n\
            \n\
            Return the plan as a JSON object with the following structure:\n\
            {{\n\
              \"tasks\": [\n\
                {{\n\
                  \"id\": 1,\n\
                  \"description\": \"Step 1 description\",\n\
                  \"tool_needed\": \"search\" (optional, if a tool is likely needed)\n\
                }}\n\
              ]\n\
            }}\n\
            Keep the plan concise (3-5 steps max). Available tools: search, calculator, weather, read_url.\n\
            Output ONLY the JSON.",
            goal
        );

        let messages = vec![
            Message::new("system", "You are a JSON-speaking planning assistant."),
            Message::new("user", &prompt),
        ];

        let response = self.llm.chat_complete(&messages).await?;
        
        // Clean markdown code blocks if present
        let clean_json = response.trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        #[derive(Deserialize)]
        struct PlanResponse {
            tasks: Vec<SubTaskResponse>,
        }
        
        #[derive(Deserialize)]
        struct SubTaskResponse {
            id: usize,
            description: String,
            tool_needed: Option<String>,
        }

        let plan_resp: PlanResponse = serde_json::from_str(clean_json)?;
        
        let tasks = plan_resp.tasks.into_iter().map(|t| SubTask {
            id: t.id,
            description: t.description,
            tool_needed: t.tool_needed,
            status: TaskStatus::Pending,
            result: None,
        }).collect();

        Ok(Plan {
            goal: goal.to_string(),
            tasks,
        })
    }
}
