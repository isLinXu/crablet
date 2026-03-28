use crablet::agent::coder::CoderAgent;
use crablet::agent::swarm::{SwarmAgent};
use crablet::cognitive::llm::{OllamaClient};
use std::sync::Arc;
use tokio;

#[tokio::test]
async fn test_agent_basic() {
    let mock_client = OllamaClient::new("mock_model");
    let agent = CoderAgent::new(Arc::new(Box::new(mock_client)));
    assert_eq!(agent.name(), "coder");
    assert_eq!(agent.id().0, "coder");
}
