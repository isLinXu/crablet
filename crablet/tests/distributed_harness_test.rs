//! Integration tests for distributed harness management
//!
//! Migrated from crablet/src/agent/distributed_harness/mod.rs inline tests.

use async_trait::async_trait;
use crablet::agent::distributed_harness::{
    create_backend, BackendType, DistributedConfig, DistributedError, DistributedHarnessManager,
    GenericHarnessAgentSpec, GenericHarnessResourceLimits, GenericHarnessResumeRequest,
    GenericHarnessRunRequest, HarnessBackend, HttpHarnessControlPlane, InMemoryBackend,
    InProcessHarnessControlPlane, NodeInfo, NodeStatus,
};
use crablet::agent::harness::{HarnessConfig, HarnessSignal};
use crablet::agent::harness_agent::{HarnessAgent, HarnessAgentBuilder};
use crablet::agent::harness_manager::HarnessStatus;
use crablet::agent::AgentRole;
use crablet::plugins::Plugin;
use crablet::skills::SkillRegistry;
use crablet::types::Message;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::{Notify, RwLock};
use uuid::Uuid;

#[cfg(feature = "web")]
use {
    axum::{extract::State, routing::post, Json, Router},
    crablet::cognitive::router::CognitiveRouter,
    crablet::config::Config,
    crablet::events::EventBus,
    crablet::gateway::harness_handlers::register_execution_rpc_handlers,
    crablet::gateway::types::{RpcRequest, RpcResponse},
    tokio::net::TcpListener,
};

fn test_dist_config(node_id: &str, port: u16) -> DistributedConfig {
    DistributedConfig {
        node_id: node_id.to_string(),
        node_address: "127.0.0.1".to_string(),
        node_port: port,
        backend_type: BackendType::InMemory,
        backend_uri: "memory://".to_string(),
        lock_ttl_secs: 300,
        heartbeat_interval_secs: 30,
        node_timeout_secs: 60,
    }
}

fn test_node(node_id: &str, port: u16, status: NodeStatus, harness_count: usize) -> NodeInfo {
    NodeInfo {
        id: node_id.to_string(),
        address: "127.0.0.1".to_string(),
        port,
        capabilities: vec!["harness".to_string()],
        status,
        last_heartbeat: chrono::Utc::now(),
        harness_count,
    }
}

#[cfg(feature = "web")]
async fn test_router() -> Arc<CognitiveRouter> {
    let config = Config {
        llm_vendor: Some("mock".to_string()),
        ..Config::default()
    };
    let event_bus = Arc::new(EventBus::new(100));
    Arc::new(CognitiveRouter::new(&config, None, event_bus).await)
}

struct EchoPlugin {
    name: String,
    prefix: String,
}

#[async_trait]
impl Plugin for EchoPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        "echo test plugin"
    }

    async fn initialize(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> anyhow::Result<String> {
        let payload = args
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or("missing");
        Ok(format!("{}{}", self.prefix, payload))
    }

    async fn shutdown(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

fn registry_with_echo_tools() -> Arc<RwLock<SkillRegistry>> {
    let mut registry = SkillRegistry::new();
    registry.register_plugin(Box::new(EchoPlugin {
        name: "echo_tool".to_string(),
        prefix: "echo:".to_string(),
    }));
    Arc::new(RwLock::new(registry))
}

struct SimpleHarnessAgent;

#[async_trait]
impl HarnessAgent for SimpleHarnessAgent {
    fn name(&self) -> &str {
        "simple-harness-agent"
    }

    fn role(&self) -> AgentRole {
        AgentRole::Executor
    }

    fn tools(&self) -> Vec<String> {
        Vec::new()
    }

    async fn execute_step(
        &self,
        _task: &str,
        _context: &[Message],
        _step_number: usize,
    ) -> anyhow::Result<(String, Option<String>, Option<serde_json::Value>)> {
        Ok(("done".to_string(), None, None))
    }
}

struct ResumeCapableHarnessAgent {
    step2_attempts: Arc<AtomicU32>,
}

#[async_trait]
impl HarnessAgent for ResumeCapableHarnessAgent {
    fn name(&self) -> &str {
        "resume-capable-harness-agent"
    }

    fn role(&self) -> AgentRole {
        AgentRole::Executor
    }

    fn tools(&self) -> Vec<String> {
        vec!["echo_tool".to_string()]
    }

    async fn execute_step(
        &self,
        _task: &str,
        context: &[Message],
        step_number: usize,
    ) -> anyhow::Result<(String, Option<String>, Option<serde_json::Value>)> {
        match step_number {
            1 => Ok((
                "Need remote tool context".to_string(),
                Some("echo_tool".to_string()),
                Some(json!({"query": "failover"})),
            )),
            2 => {
                if self.step2_attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    anyhow::bail!("resume after failover")
                }

                let observation = context.last().and_then(Message::text).unwrap_or_default();
                Ok((
                    format!("Recovered on failover: {}", observation),
                    None,
                    None,
                ))
            }
            _ => anyhow::bail!("unexpected step {}", step_number),
        }
    }
}

struct BlockingProgressHarnessAgent {
    step2_started: Arc<Notify>,
    step2_release: Arc<Notify>,
}

#[async_trait]
impl HarnessAgent for BlockingProgressHarnessAgent {
    fn name(&self) -> &str {
        "blocking-progress-harness-agent"
    }

    fn role(&self) -> AgentRole {
        AgentRole::Executor
    }

    fn tools(&self) -> Vec<String> {
        vec!["echo_tool".to_string()]
    }

    async fn execute_step(
        &self,
        _task: &str,
        context: &[Message],
        step_number: usize,
    ) -> anyhow::Result<(String, Option<String>, Option<serde_json::Value>)> {
        match step_number {
            1 => Ok((
                "Capture progress before completion".to_string(),
                Some("echo_tool".to_string()),
                Some(json!({"query": "midflight"})),
            )),
            2 => {
                self.step2_started.notify_one();
                self.step2_release.notified().await;
                let observation = context.last().and_then(Message::text).unwrap_or_default();
                Ok((format!("Finished after block: {}", observation), None, None))
            }
            _ => anyhow::bail!("unexpected step {}", step_number),
        }
    }
}

struct ToolCallingHarnessAgent;

#[async_trait]
impl HarnessAgent for ToolCallingHarnessAgent {
    fn name(&self) -> &str {
        "tool-calling-harness-agent"
    }

    fn role(&self) -> AgentRole {
        AgentRole::Executor
    }

    fn tools(&self) -> Vec<String> {
        vec!["echo_tool".to_string()]
    }

    async fn execute_step(
        &self,
        _task: &str,
        _context: &[Message],
        step_number: usize,
    ) -> anyhow::Result<(String, Option<String>, Option<serde_json::Value>)> {
        match step_number {
            1 => Ok((
                "Need allowed tool".to_string(),
                Some("echo_tool".to_string()),
                Some(json!({"query": "policy"})),
            )),
            2 => Ok(("Tool policy satisfied".to_string(), None, None)),
            _ => anyhow::bail!("unexpected step {}", step_number),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[tokio::test]
async fn test_in_memory_backend() {
    let backend = Arc::new(InMemoryBackend::new());

    // Test harness operations
    let info = crablet::agent::harness_manager::HarnessInfo::new(
        "test-1".to_string(),
        HarnessConfig::default(),
    );

    backend.create_harness(&info).await.unwrap();

    let retrieved = backend.get_harness("test-1").await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, "test-1");

    // Test lock acquisition
    let acquired = backend
        .acquire_lock("resource-1", "node-1", 60)
        .await
        .unwrap();
    assert!(acquired);

    // Test lock not acquired by another
    let acquired2 = backend
        .acquire_lock("resource-1", "node-2", 60)
        .await
        .unwrap();
    assert!(!acquired2);

    // Test lock release
    backend.release_lock("resource-1", "node-1").await.unwrap();

    let acquired3 = backend
        .acquire_lock("resource-1", "node-2", 60)
        .await
        .unwrap();
    assert!(acquired3);
}

#[tokio::test]
async fn test_create_backend_in_memory() {
    let backend = create_backend(&test_dist_config("node-1", 8080))
        .await
        .unwrap();
    let info = crablet::agent::harness_manager::HarnessInfo::new(
        "factory-test".to_string(),
        HarnessConfig::default(),
    );

    backend.create_harness(&info).await.unwrap();
    let retrieved = backend.get_harness("factory-test").await.unwrap();

    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, "factory-test");
}

#[test]
fn test_backend_type_parsing_and_defaults() {
    assert_eq!("redis".parse::<BackendType>().unwrap(), BackendType::Redis);
    assert_eq!(
        "in-memory".parse::<BackendType>().unwrap(),
        BackendType::InMemory
    );
    assert_eq!(
        BackendType::Redis.default_backend_uri(),
        "redis://127.0.0.1/"
    );
    assert_eq!(BackendType::InMemory.to_string(), "memory");
}

#[tokio::test]
async fn test_create_backend_redis_when_url_available() {
    let Some(redis_url) = std::env::var("CRABLET_TEST_REDIS_URL")
        .ok()
        .or_else(|| std::env::var("REDIS_URL").ok())
        .filter(|value| !value.trim().is_empty())
    else {
        eprintln!("skipping redis backend test: CRABLET_TEST_REDIS_URL/REDIS_URL not set");
        return;
    };

    let mut config = test_dist_config("node-redis", 8080);
    config.backend_type = BackendType::Redis;
    config.backend_uri = redis_url;

    let backend = create_backend(&config).await.unwrap();
    let harness_id = format!("redis-factory-{}", Uuid::new_v4());
    let info = crablet::agent::harness_manager::HarnessInfo::new(
        harness_id.clone(),
        HarnessConfig::default(),
    );

    backend.create_harness(&info).await.unwrap();
    let retrieved = backend.get_harness(&harness_id).await.unwrap();
    backend.delete_harness(&harness_id).await.unwrap();

    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().id, harness_id);
}

#[tokio::test]
async fn test_create_backend_rejects_unimplemented_backends() {
    let mut config = test_dist_config("node-1", 8080);
    config.backend_type = BackendType::Consul;

    let result = create_backend(&config).await;
    assert!(matches!(result, Err(DistributedError::ClusterError(_))));
}

#[tokio::test]
async fn test_distributed_manager() {
    let backend: Arc<dyn HarnessBackend> = Arc::new(InMemoryBackend::new());
    let config = DistributedConfig {
        node_id: "node-1".to_string(),
        node_address: "127.0.0.1".to_string(),
        node_port: 8080,
        backend_type: BackendType::InMemory,
        backend_uri: "memory://".to_string(),
        lock_ttl_secs: 300,
        heartbeat_interval_secs: 30,
        node_timeout_secs: 60,
    };

    let manager = DistributedHarnessManager::new(backend, config);

    // Create harness
    let id = manager.create_harness(None).await.unwrap();
    assert!(manager.is_local(&id).await);

    // Get harness
    let harness = manager.get_harness(&id).await.unwrap();
    assert!(harness.is_some());
}

#[tokio::test]
async fn test_distributed_manager_registers_local_node() {
    let backend: Arc<dyn HarnessBackend> = Arc::new(InMemoryBackend::new());
    let manager = DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));

    manager.register_local_node().await.unwrap();

    let nodes = backend.get_nodes().await.unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].id, "node-1");
    assert_eq!(nodes[0].address, "127.0.0.1");
    assert_eq!(nodes[0].port, 8080);
    assert_eq!(nodes[0].status, NodeStatus::Active);
}

#[tokio::test]
async fn test_distributed_manager_run_agent_syncs_backend_state() {
    let backend: Arc<dyn HarnessBackend> = Arc::new(InMemoryBackend::new());
    let manager = DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));
    let builder =
        HarnessAgentBuilder::new(Arc::new(SimpleHarnessAgent)).with_config(HarnessConfig {
            max_steps: 2,
            ..Default::default()
        });

    let (id, result) = manager.run_agent(&builder, "hello", &[]).await.unwrap();

    assert!(result.success);

    let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
    assert!(matches!(backend_info.status, HarnessStatus::Completed));
    assert_eq!(
        backend_info
            .config
            .metadata
            .get("node_id")
            .map(String::as_str),
        Some("node-1")
    );
}

#[tokio::test]
async fn test_distributed_manager_run_agent_enforces_allowed_tools() {
    let backend: Arc<dyn HarnessBackend> = Arc::new(InMemoryBackend::new());
    let manager = DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));
    let builder = HarnessAgentBuilder::new(Arc::new(ToolCallingHarnessAgent))
        .with_config(HarnessConfig {
            max_steps: 3,
            ..Default::default()
        })
        .with_skill_registry(registry_with_echo_tools())
        .with_allowed_tools(vec!["echo_tool".to_string()]);

    let (id, result) = manager
        .run_agent(&builder, "policy task", &[])
        .await
        .unwrap();

    assert!(result.success);
    assert_eq!(result.output, "Tool policy satisfied");
    let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
    assert!(matches!(backend_info.status, HarnessStatus::Completed));
}

#[tokio::test]
async fn test_distributed_manager_run_agent_rejects_disallowed_tools() {
    let backend: Arc<dyn HarnessBackend> = Arc::new(InMemoryBackend::new());
    let manager = DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));
    let builder = HarnessAgentBuilder::new(Arc::new(ToolCallingHarnessAgent))
        .with_config(HarnessConfig {
            max_steps: 3,
            ..Default::default()
        })
        .with_skill_registry(registry_with_echo_tools())
        .with_allowed_tools(vec!["other_tool".to_string()]);

    let (id, result) = manager
        .run_agent(&builder, "policy task", &[])
        .await
        .unwrap();

    assert!(!result.success);
    assert!(result
        .output
        .contains("tool not allowed by execution policy"));
    let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
    assert!(matches!(backend_info.status, HarnessStatus::Failed));
}

#[tokio::test]
async fn test_distributed_manager_run_agent_enforces_resource_limits() {
    let backend: Arc<dyn HarnessBackend> = Arc::new(InMemoryBackend::new());
    let manager = DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));
    let builder = HarnessAgentBuilder::new(Arc::new(ToolCallingHarnessAgent))
        .with_config(HarnessConfig {
            max_steps: 3,
            max_cpu_time_ms: Some(0),
            ..Default::default()
        })
        .with_skill_registry(registry_with_echo_tools())
        .with_allowed_tools(vec!["echo_tool".to_string()]);

    let (id, result) = manager
        .run_agent(&builder, "budget task", &[])
        .await
        .unwrap();

    assert!(!result.success);
    assert!(result.output.contains("Resource limit exceeded"));
    let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
    assert!(matches!(backend_info.status, HarnessStatus::Failed));
}

#[tokio::test]
async fn test_distributed_manager_syncs_progress_to_backend_while_running() {
    let backend: Arc<dyn HarnessBackend> = Arc::new(InMemoryBackend::new());
    let manager = Arc::new(DistributedHarnessManager::new(
        backend.clone(),
        test_dist_config("node-1", 8080),
    ));
    let step2_started = Arc::new(Notify::new());
    let step2_release = Arc::new(Notify::new());
    let builder = Arc::new(
        HarnessAgentBuilder::new(Arc::new(BlockingProgressHarnessAgent {
            step2_started: step2_started.clone(),
            step2_release: step2_release.clone(),
        }))
        .with_config(HarnessConfig {
            max_steps: 4,
            ..Default::default()
        })
        .with_skill_registry(registry_with_echo_tools()),
    );

    let run_task = {
        let manager = manager.clone();
        let builder = builder.clone();
        tokio::spawn(async move {
            manager
                .run_agent(builder.as_ref(), "track progress", &[])
                .await
        })
    };

    tokio::time::timeout(std::time::Duration::from_secs(1), step2_started.notified())
        .await
        .unwrap();

    let harnesses = backend.list_harnesses().await.unwrap();
    assert_eq!(harnesses.len(), 1);

    let id = harnesses[0].id.clone();
    let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
    assert!(matches!(backend_info.status, HarnessStatus::Running));
    assert_eq!(
        backend_info
            .execution_state
            .as_ref()
            .map(|state| state.trace.len()),
        Some(1)
    );
    assert_eq!(
        backend_info
            .execution_state
            .as_ref()
            .and_then(|state| state.trace.first())
            .and_then(|step| step.observation.as_deref()),
        Some("echo:midflight")
    );

    step2_release.notify_one();

    let (completed_id, result) = run_task.await.unwrap().unwrap();
    assert_eq!(completed_id, id);
    assert!(result.success);
    assert_eq!(result.trace.len(), 2);

    let completed_info = backend.get_harness(&id).await.unwrap().unwrap();
    assert!(matches!(completed_info.status, HarnessStatus::Completed));
    assert_eq!(
        completed_info
            .execution_state
            .as_ref()
            .map(|state| state.trace.len()),
        Some(2)
    );
}

#[tokio::test]
async fn test_distributed_manager_creates_harness_on_least_loaded_remote_node() {
    let backend = Arc::new(InMemoryBackend::new());
    let control_plane = Arc::new(InProcessHarnessControlPlane::new());
    let primary = Arc::new(
        DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080))
            .with_control_plane(control_plane.clone()),
    );
    let secondary = Arc::new(
        DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081))
            .with_control_plane(control_plane.clone()),
    );
    control_plane.register_manager(&primary).await;
    control_plane.register_manager(&secondary).await;
    primary.register_local_node().await.unwrap();
    secondary.register_local_node().await.unwrap();

    let local_id = primary.create_harness(None).await.unwrap();
    assert!(primary.is_local(&local_id).await);

    let id = primary
        .create_harness_distributed(
            Some(HarnessConfig {
                max_steps: 12,
                ..Default::default()
            }),
            None,
        )
        .await
        .unwrap();

    let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
    let primary_stats = primary.get_cluster_stats().await.unwrap();

    assert!(!primary.is_local(&id).await);
    assert!(secondary.is_local(&id).await);
    assert_eq!(
        backend_info
            .config
            .metadata
            .get("node_id")
            .map(String::as_str),
        Some("node-2")
    );
    assert_eq!(backend_info.config.max_steps, 12);
    assert_eq!(primary_stats.distributed_stats.remote_operations, 1);
    assert_eq!(primary_stats.distributed_stats.cross_node_delegations, 1);
}

#[tokio::test]
async fn test_distributed_manager_forwards_signal_to_active_remote_owner() {
    let backend = Arc::new(InMemoryBackend::new());
    let control_plane = Arc::new(InProcessHarnessControlPlane::new());
    let primary = Arc::new(
        DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080))
            .with_control_plane(control_plane.clone()),
    );
    let secondary = Arc::new(
        DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081))
            .with_control_plane(control_plane.clone()),
    );
    control_plane.register_manager(&primary).await;
    control_plane.register_manager(&secondary).await;

    let id = primary.create_harness(None).await.unwrap();
    primary
        .local_manager()
        .update_status(&id, HarnessStatus::Running)
        .await;
    let running_info = primary.local_manager().get_info(&id).await.unwrap();
    backend.update_harness(&running_info).await.unwrap();

    secondary
        .send_signal(&id, HarnessSignal::Pause)
        .await
        .unwrap();

    let paused_info = primary.local_manager().get_info(&id).await.unwrap();
    let paused_backend = backend.get_harness(&id).await.unwrap().unwrap();
    assert!(matches!(paused_info.status, HarnessStatus::Paused));
    assert!(matches!(paused_backend.status, HarnessStatus::Paused));

    secondary
        .send_signal(&id, HarnessSignal::Resume)
        .await
        .unwrap();

    let resumed_info = primary.local_manager().get_info(&id).await.unwrap();
    let resumed_backend = backend.get_harness(&id).await.unwrap().unwrap();
    let secondary_stats = secondary.get_cluster_stats().await.unwrap();
    assert!(matches!(resumed_info.status, HarnessStatus::Running));
    assert!(matches!(resumed_backend.status, HarnessStatus::Running));
    assert_eq!(secondary_stats.distributed_stats.remote_operations, 2);
    assert_eq!(secondary_stats.distributed_stats.cross_node_delegations, 2);
}

#[tokio::test]
async fn test_distributed_manager_removes_terminal_harness_via_active_remote_owner() {
    let backend = Arc::new(InMemoryBackend::new());
    let control_plane = Arc::new(InProcessHarnessControlPlane::new());
    let primary = Arc::new(
        DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080))
            .with_control_plane(control_plane.clone()),
    );
    let secondary = Arc::new(
        DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081))
            .with_control_plane(control_plane.clone()),
    );
    control_plane.register_manager(&primary).await;
    control_plane.register_manager(&secondary).await;
    primary.register_local_node().await.unwrap();
    secondary.register_local_node().await.unwrap();

    let id = primary.create_harness(None).await.unwrap();
    primary.local_manager().complete_harness(&id).await.unwrap();
    let completed_info = primary.local_manager().get_info(&id).await.unwrap();
    backend.update_harness(&completed_info).await.unwrap();

    secondary.remove_harness(&id).await.unwrap();

    let secondary_stats = secondary.get_cluster_stats().await.unwrap();
    assert!(primary.local_manager().get_info(&id).await.is_none());
    assert!(backend.get_harness(&id).await.unwrap().is_none());
    assert_eq!(secondary_stats.distributed_stats.remote_operations, 1);
    assert_eq!(secondary_stats.distributed_stats.cross_node_delegations, 1);
}

#[cfg(feature = "web")]
#[tokio::test]
async fn test_distributed_manager_forwards_signal_over_http_rpc() {
    let backend = Arc::new(InMemoryBackend::new());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let primary_port = listener.local_addr().unwrap().port();
    let primary = Arc::new(DistributedHarnessManager::new(
        backend.clone(),
        test_dist_config("node-1", primary_port),
    ));

    let rpc = crablet::gateway::rpc::RpcDispatcher::new();
    DistributedHarnessManager::register_rpc_handlers(&primary, &rpc)
        .await
        .unwrap();

    let app = Router::new()
        .route(
            "/rpc",
            post(
                |State(rpc): State<crablet::gateway::rpc::RpcDispatcher>,
                 Json(request): Json<RpcRequest>| async move {
                    Json::<RpcResponse>(rpc.dispatch(request).await)
                },
            ),
        )
        .with_state(rpc.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let secondary = Arc::new(
        DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081))
            .with_control_plane(Arc::new(HttpHarnessControlPlane::new())),
    );

    let id = primary.create_harness(None).await.unwrap();
    primary
        .local_manager()
        .update_status(&id, HarnessStatus::Running)
        .await;
    let running_info = primary.local_manager().get_info(&id).await.unwrap();
    backend.update_harness(&running_info).await.unwrap();

    secondary
        .send_signal(&id, HarnessSignal::Pause)
        .await
        .unwrap();
    let paused_info = primary.local_manager().get_info(&id).await.unwrap();
    let paused_backend = backend.get_harness(&id).await.unwrap().unwrap();
    assert!(matches!(paused_info.status, HarnessStatus::Paused));
    assert!(matches!(paused_backend.status, HarnessStatus::Paused));

    secondary
        .send_signal(&id, HarnessSignal::Resume)
        .await
        .unwrap();
    let resumed_info = primary.local_manager().get_info(&id).await.unwrap();
    let resumed_backend = backend.get_harness(&id).await.unwrap().unwrap();
    let secondary_stats = secondary.get_cluster_stats().await.unwrap();
    assert!(matches!(resumed_info.status, HarnessStatus::Running));
    assert!(matches!(resumed_backend.status, HarnessStatus::Running));
    assert_eq!(secondary_stats.distributed_stats.remote_operations, 2);
    assert_eq!(secondary_stats.distributed_stats.cross_node_delegations, 2);

    server.abort();
}

#[cfg(feature = "web")]
#[tokio::test]
async fn test_distributed_manager_forwards_run_over_http_rpc() {
    let backend = Arc::new(InMemoryBackend::new());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let primary_port = listener.local_addr().unwrap().port();
    let primary = Arc::new(DistributedHarnessManager::new(
        backend.clone(),
        test_dist_config("node-1", primary_port),
    ));

    let rpc = crablet::gateway::rpc::RpcDispatcher::new();
    DistributedHarnessManager::register_rpc_handlers(&primary, &rpc)
        .await
        .unwrap();
    register_execution_rpc_handlers(&rpc, primary.clone(), test_router().await)
        .await
        .unwrap();

    let app = Router::new()
        .route(
            "/rpc",
            post(
                |State(rpc): State<crablet::gateway::rpc::RpcDispatcher>,
                 Json(request): Json<RpcRequest>| async move {
                    Json::<RpcResponse>(rpc.dispatch(request).await)
                },
            ),
        )
        .with_state(rpc.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let secondary = Arc::new(
        DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081))
            .with_control_plane(Arc::new(HttpHarnessControlPlane::new())),
    );
    primary.register_local_node().await.unwrap();
    secondary.register_local_node().await.unwrap();
    let agent_spec = GenericHarnessAgentSpec {
        role: "http-runner".to_string(),
        name: Some("HTTP Runner".to_string()),
        description: Some("remote execution agent".to_string()),
        system_prompt: Some("You are a remote execution agent.".to_string()),
        allowed_tools: None,
        resource_limits: Some(GenericHarnessResourceLimits {
            max_steps: Some(2),
            tool_timeout_ms: Some(1_500),
            step_timeout_ms: Some(2_500),
            max_memory_bytes: Some(1_048_576),
            max_cpu_time_ms: Some(5_000),
        }),
    };

    let response = secondary
        .forward_generic_run(
            &test_node("node-1", primary_port, NodeStatus::Active, 0),
            GenericHarnessRunRequest {
                task: "http rpc task".to_string(),
                context: Vec::new(),
                agent: agent_spec.clone(),
                harness_config: Some(HarnessConfig::default()),
                target_node_id: None,
            },
        )
        .await
        .unwrap();

    let info = primary
        .local_manager()
        .get_info(&response.harness_id)
        .await
        .unwrap();
    let backend_info = backend
        .get_harness(&response.harness_id)
        .await
        .unwrap()
        .unwrap();
    let secondary_stats = secondary.get_cluster_stats().await.unwrap();

    assert!(response.result.success);
    assert_eq!(
        response.result.output,
        "Step 1: (Mock LLM) Processed: http rpc task"
    );
    assert_eq!(
        info.execution_state
            .as_ref()
            .and_then(|state| state.system_prompt.as_deref()),
        Some("You are a remote execution agent.")
    );
    assert_eq!(info.config.max_steps, 2);
    assert_eq!(info.config.tool_timeout.as_millis(), 1_500);
    assert_eq!(info.config.step_timeout.as_millis(), 2_500);
    assert_eq!(
        GenericHarnessAgentSpec::from_metadata(&info.config.metadata).unwrap(),
        agent_spec
    );
    assert!(matches!(info.status, HarnessStatus::Completed));
    assert!(matches!(backend_info.status, HarnessStatus::Completed));
    assert_eq!(secondary_stats.distributed_stats.remote_operations, 1);
    assert_eq!(secondary_stats.distributed_stats.cross_node_delegations, 1);

    server.abort();
}

#[cfg(feature = "web")]
#[tokio::test]
async fn test_distributed_manager_forwards_resume_over_http_rpc() {
    let backend = Arc::new(InMemoryBackend::new());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let primary_port = listener.local_addr().unwrap().port();
    let primary = Arc::new(DistributedHarnessManager::new(
        backend.clone(),
        test_dist_config("node-1", primary_port),
    ));

    let rpc = crablet::gateway::rpc::RpcDispatcher::new();
    DistributedHarnessManager::register_rpc_handlers(&primary, &rpc)
        .await
        .unwrap();
    register_execution_rpc_handlers(&rpc, primary.clone(), test_router().await)
        .await
        .unwrap();

    let app = Router::new()
        .route(
            "/rpc",
            post(
                |State(rpc): State<crablet::gateway::rpc::RpcDispatcher>,
                 Json(request): Json<RpcRequest>| async move {
                    Json::<RpcResponse>(rpc.dispatch(request).await)
                },
            ),
        )
        .with_state(rpc.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let secondary = Arc::new(
        DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081))
            .with_control_plane(Arc::new(HttpHarnessControlPlane::new())),
    );
    primary.register_local_node().await.unwrap();
    secondary.register_local_node().await.unwrap();
    let persisted_agent = GenericHarnessAgentSpec {
        role: "http-resume-runner".to_string(),
        name: Some("HTTP Resume Runner".to_string()),
        description: Some("remote resume agent".to_string()),
        system_prompt: Some("You are a remote resume agent.".to_string()),
        allowed_tools: None,
        resource_limits: Some(GenericHarnessResourceLimits {
            max_steps: Some(4),
            tool_timeout_ms: Some(1_000),
            step_timeout_ms: Some(2_000),
            max_memory_bytes: Some(2_097_152),
            max_cpu_time_ms: Some(10_000),
        }),
    };
    let mut harness_config = HarnessConfig::default();
    persisted_agent
        .persist_into_metadata(&mut harness_config.metadata)
        .unwrap();

    let harness_id = primary.create_harness(Some(harness_config)).await.unwrap();
    primary
        .local_manager()
        .set_execution_state(
            &harness_id,
            crablet::agent::harness_agent::HarnessExecutionState::new(
                "resume over http",
                &[],
                None,
            ),
        )
        .await
        .unwrap();
    primary
        .local_manager()
        .update_status(&harness_id, HarnessStatus::Paused)
        .await;
    let paused_info = primary.local_manager().get_info(&harness_id).await.unwrap();
    backend.update_harness(&paused_info).await.unwrap();

    let response = secondary
        .forward_generic_resume(
            &test_node("node-1", primary_port, NodeStatus::Active, 1),
            GenericHarnessResumeRequest {
                harness_id: harness_id.clone(),
                agent: None,
                harness_config: None,
            },
        )
        .await
        .unwrap();

    let info = primary.local_manager().get_info(&harness_id).await.unwrap();
    let backend_info = backend.get_harness(&harness_id).await.unwrap().unwrap();
    let secondary_stats = secondary.get_cluster_stats().await.unwrap();

    assert_eq!(response.harness_id, harness_id);
    assert!(response.result.success);
    assert_eq!(
        response.result.output,
        "Step 1: (Mock LLM) Processed: resume over http"
    );
    assert_eq!(
        info.execution_state
            .as_ref()
            .and_then(|state| state.system_prompt.as_deref()),
        Some("You are a remote resume agent.")
    );
    assert_eq!(info.config.max_steps, 4);
    assert_eq!(info.config.tool_timeout.as_millis(), 1_000);
    assert_eq!(info.config.step_timeout.as_millis(), 2_000);
    assert_eq!(
        GenericHarnessAgentSpec::from_metadata(&info.config.metadata).unwrap(),
        persisted_agent
    );
    assert!(matches!(info.status, HarnessStatus::Completed));
    assert!(matches!(backend_info.status, HarnessStatus::Completed));
    assert_eq!(secondary_stats.distributed_stats.remote_operations, 1);
    assert_eq!(secondary_stats.distributed_stats.cross_node_delegations, 1);

    server.abort();
}

#[tokio::test]
async fn test_distributed_manager_claims_dead_remote_harness() {
    let backend = Arc::new(InMemoryBackend::new());
    let primary = DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));
    let failover =
        DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081));

    let id = primary.create_harness(None).await.unwrap();
    primary
        .local_manager()
        .update_status(&id, HarnessStatus::Running)
        .await;
    let running_info = primary.local_manager().get_info(&id).await.unwrap();
    backend.update_harness(&running_info).await.unwrap();
    backend
        .register_node(&test_node("node-1", 8080, NodeStatus::Dead, 1))
        .await
        .unwrap();

    let claimed = failover.get_harness(&id).await.unwrap().unwrap();
    let metadata = claimed.read().await.metadata();
    let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
    let cluster_stats = failover.get_cluster_stats().await.unwrap();

    assert!(metadata.paused);
    assert!(failover.is_local(&id).await);
    assert!(matches!(backend_info.status, HarnessStatus::Paused));
    assert_eq!(
        backend_info
            .config
            .metadata
            .get("node_id")
            .map(String::as_str),
        Some("node-2")
    );
    assert_eq!(cluster_stats.distributed_stats.node_failovers, 1);
}

#[tokio::test]
async fn test_handle_node_failure_claims_only_non_terminal_harnesses() {
    let backend = Arc::new(InMemoryBackend::new());
    let primary = DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));
    let failover =
        DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081));

    let running_id = primary.create_harness(None).await.unwrap();
    primary
        .local_manager()
        .update_status(&running_id, HarnessStatus::Running)
        .await;
    let running_info = primary.local_manager().get_info(&running_id).await.unwrap();
    backend.update_harness(&running_info).await.unwrap();

    let completed_id = primary.create_harness(None).await.unwrap();
    primary
        .local_manager()
        .complete_harness(&completed_id)
        .await
        .unwrap();
    let completed_info = primary
        .local_manager()
        .get_info(&completed_id)
        .await
        .unwrap();
    backend.update_harness(&completed_info).await.unwrap();

    backend
        .register_node(&test_node("node-1", 8080, NodeStatus::Dead, 2))
        .await
        .unwrap();

    let claimed = failover
        .handle_node_failure(&"node-1".to_string())
        .await
        .unwrap();
    let backend_completed = backend.get_harness(&completed_id).await.unwrap().unwrap();

    assert_eq!(claimed, vec![running_id.clone()]);
    assert!(failover.is_local(&running_id).await);
    assert!(!failover.is_local(&completed_id).await);
    assert!(matches!(backend_completed.status, HarnessStatus::Completed));
    assert_eq!(
        backend_completed
            .config
            .metadata
            .get("node_id")
            .map(String::as_str),
        Some("node-1")
    );
}

#[tokio::test]
async fn test_distributed_manager_resumes_claimed_harness_after_failover() {
    let backend = Arc::new(InMemoryBackend::new());
    let primary = DistributedHarnessManager::new(backend.clone(), test_dist_config("node-1", 8080));
    let failover =
        DistributedHarnessManager::new(backend.clone(), test_dist_config("node-2", 8081));
    let step2_attempts = Arc::new(AtomicU32::new(0));
    let builder = HarnessAgentBuilder::new(Arc::new(ResumeCapableHarnessAgent {
        step2_attempts: step2_attempts.clone(),
    }))
    .with_config(HarnessConfig {
        max_steps: 4,
        ..Default::default()
    })
    .with_retry_config(crablet::agent::harness::RetryConfig {
        max_retries: 1,
        ..Default::default()
    })
    .with_skill_registry(registry_with_echo_tools());

    let (id, first_result) = primary
        .run_agent(&builder, "resume failover", &[])
        .await
        .unwrap();
    assert!(!first_result.success);
    assert_eq!(first_result.trace.len(), 1);

    backend
        .register_node(&test_node("node-1", 8080, NodeStatus::Dead, 1))
        .await
        .unwrap();

    let resumed = failover.resume_agent(&id, &builder).await.unwrap();
    assert!(resumed.success);
    assert_eq!(resumed.output, "Recovered on failover: echo:failover");
    assert_eq!(resumed.trace.len(), 2);
    assert_eq!(step2_attempts.load(Ordering::SeqCst), 2);

    let backend_info = backend.get_harness(&id).await.unwrap().unwrap();
    assert!(matches!(backend_info.status, HarnessStatus::Completed));
    assert_eq!(
        backend_info
            .execution_state
            .as_ref()
            .map(|state| state.trace.len()),
        Some(2)
    );
    assert_eq!(
        backend_info
            .config
            .metadata
            .get("node_id")
            .map(String::as_str),
        Some("node-2")
    );
}
