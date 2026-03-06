use async_trait::async_trait;
use anyhow::Result;
use crate::types::{Message, TraceStep};
use super::{CognitiveMiddleware, MiddlewareState, MiddlewarePipeline};
use tracing::info;

pub struct PlanningMiddleware;

#[async_trait]
impl CognitiveMiddleware for PlanningMiddleware {
    fn name(&self) -> &str {
        "Planning"
    }

    async fn execute(&self, input: &str, context: &mut Vec<Message>, state: &MiddlewareState) -> Result<Option<(String, Vec<TraceStep>)>> {
        // Planning Phase (for complex queries)
        if input.len() > 100 || input.contains(" and ") || input.contains(" then ") {
             info!("Complex query detected, invoking Task Planner...");
             if let Ok(plan) = state.planner.create_plan(input).await {
                 info!("Plan generated: {} steps", plan.tasks.len());
                 if let Ok(plan_str) = serde_json::to_string_pretty(&plan.tasks) {
                     let plan_context = format!("\n[CURRENT PLAN]\n{}\nFollow this plan to answer the user request.", plan_str);
                     // Inject into system prompt
                     MiddlewarePipeline::ensure_system_prompt(context, &plan_context);
                 }
             }
        }
        Ok(None)
    }
}
