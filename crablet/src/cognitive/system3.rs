use crate::error::{Result, CrabletError};
use crate::cognitive::CognitiveSystem;
use crate::types::{Message, TraceStep};
use async_trait::async_trait;
use std::sync::Arc;
use crate::cognitive::llm::LlmClient;
use crate::agent::researcher::ResearchAgent;
use crate::agent::swarm::{Swarm, SwarmAgent, AgentId, SwarmOrchestrator};
use crate::agent::coder::CoderAgent;
use crate::agent::analyst::AnalystAgent;
use crate::agent::aggregator::AggregatorAgent;
use crate::agent::coordinator::CoordinatorAgent;
use crate::agent::factory::AgentFactory;
use std::time::Duration;

use crate::events::EventBus;

use sqlx::sqlite::SqlitePool;

#[derive(Clone)]
pub struct System3 {
    pub swarm: Arc<Swarm>,
    pub orchestrator: Option<Arc<SwarmOrchestrator>>,
    coordinator: CoordinatorAgent, // Keep for legacy/CLI flow?
    self_id: AgentId,
    event_bus: Arc<EventBus>,
    timeout: Duration,
}

impl System3 {
    pub async fn new(llm: Arc<Box<dyn LlmClient>>, event_bus: Arc<EventBus>, pool: Option<SqlitePool>) -> Self {
        let swarm = Arc::new(Swarm::new().with_event_bus(event_bus.clone()));
        
        let agent_factory = Arc::new(AgentFactory::new(llm.clone(), event_bus.clone()));
        
        let orchestrator = Arc::new(SwarmOrchestrator::new(llm.clone(), swarm.clone(), pool, agent_factory));
        
        // Initialize orchestrator (load active graphs)
        orchestrator.init().await;
        
        let coordinator = CoordinatorAgent::new(llm.clone(), swarm.clone());
        let coordinator_id = coordinator.id().clone();
        
        // Register Researcher
        let researcher = Box::new(ResearchAgent::new(llm.clone(), event_bus.clone()));
        swarm.register_agent(researcher).await;
        
        // Register Coder
        let coder = Box::new(CoderAgent::new(llm.clone()));
        swarm.register_agent(coder).await;
        
        // Register Analyst
        let analyst = Box::new(AnalystAgent::new(llm.clone()));
        swarm.register_agent(analyst).await;
        
        // Register Aggregator
        let aggregator = Box::new(AggregatorAgent::new(llm.clone()));
        swarm.register_agent(aggregator).await;
        
        // Register Coordinator (System3 delegate)
        let coord_clone = coordinator.clone();
        swarm.register_agent(Box::new(coord_clone)).await;

        Self {
            swarm,
            orchestrator: Some(orchestrator),
            coordinator,
            self_id: coordinator_id,
            event_bus,
            timeout: Duration::from_secs(120), // Default 2 minutes timeout
        }
    }
}

#[async_trait]
impl CognitiveSystem for System3 {
    fn name(&self) -> &str {
        "System 3 (Swarm)"
    }

    async fn process(&self, input: &str, _context: &[Message]) -> Result<(String, Vec<TraceStep>)> {
        // Extract topic (flexible matching)
        // Fix P0: Safe string slicing
        let lower = input.to_lowercase();
        let topic = if lower.starts_with("research ") {
            // Find where "research " ends in char indices
            // "research " is 9 chars.
            // We want to skip 9 chars.
            input.chars().skip(9).collect::<String>().trim().to_string()
        } else {
            input.trim().to_string()
        };
        
        // Submit task to coordinator
        let task_id = self.coordinator.submit_task(topic.to_string()).await;
        tracing::info!("System 3 submitted task '{}' (id: {})", topic, task_id);

        // Execute via Coordinator (this will block until all subtasks are done)
        // We use a timeout to prevent hanging forever
        let execution_future = self.coordinator.execute_task(&task_id);
        
        match tokio::time::timeout(self.timeout, execution_future).await {
            Ok(Ok(response)) => {
                let traces = vec![
                    TraceStep {
                        step: 1,
                        thought: format!("Swarm execution completed for task {}", task_id),
                        action: Some("swarm_execution".to_string()),
                        action_input: Some(topic.to_string()),
                        observation: Some("Received aggregated result".to_string()),
                    }
                ];
                Ok((response, traces))
            },
            Ok(Err(e)) => Err(CrabletError::Swarm(format!("Swarm execution failed: {}", e))),
            Err(_) => {
                tracing::warn!("Swarm task {} timed out after {:?}", task_id, self.timeout);
                Ok(("Task timed out. The agents are taking longer than expected.".to_string(), vec![]))
            }
        }
    }
}
