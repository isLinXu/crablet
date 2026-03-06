use async_trait::async_trait;
use anyhow::Result;
use crate::types::{Message, TraceStep};
use super::{CognitiveMiddleware, MiddlewareState, MiddlewarePipeline};

pub struct SkillContextMiddleware;

#[async_trait]
impl CognitiveMiddleware for SkillContextMiddleware {
    fn name(&self) -> &str {
        "Skill Context Injection"
    }

    async fn execute(&self, _input: &str, context: &mut Vec<Message>, state: &MiddlewareState) -> Result<Option<(String, Vec<TraceStep>)>> {
        let registry = state.skills.read().await;
        let skills_list = registry.list_skills();
        
        if !skills_list.is_empty() {
            let mut skills_desc = String::from("You have access to the following tools:\n");
            for skill in &skills_list {
                skills_desc.push_str(&format!("- {}: {} (Args: {})\n", skill.name, skill.description, skill.parameters));
            }
            
            let msg = format!("\n[TOOLS]\n{}\nIf you need to use a tool, please generate a tool call.\n", skills_desc);
            MiddlewarePipeline::ensure_system_prompt(context, &msg);
        }
        
        Ok(None)
    }
}
