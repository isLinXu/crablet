use std::collections::HashSet;
use std::sync::Arc;

use crate::agent::distributed_harness::{
    DistributedError, DistributedHarnessManager, GenericHarnessAgentSpec,
    GenericHarnessResumeRequest, GenericHarnessResumeResponse, GenericHarnessRunRequest,
    GenericHarnessRunResponse, NodeStatus,
};
use crate::agent::harness::{HarnessConfig, HarnessSignal};
use crate::agent::harness_agent::{HarnessAgentBuilder, HarnessExecutionState, SharedAgentAdapter};
use crate::agent::{factory::AgentFactory, SharedAgent};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};

use crate::cognitive::router::CognitiveRouter;
use crate::gateway::rpc::RpcDispatcher;
use crate::gateway::server::CrabletGateway;
use crate::gateway::types::RpcError;
use crate::types::Message;

#[derive(Debug, serde::Deserialize)]
pub struct HarnessSignalRequest {
    pub signal: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct CreateHarnessRequest {
    #[serde(default)]
    pub config: Option<HarnessConfig>,
    #[serde(default)]
    pub target_node_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExecuteHarnessRequest {
    pub task: String,
    #[serde(default)]
    pub context: Vec<Message>,
    pub agent: GenericHarnessAgentSpec,
    #[serde(default)]
    pub harness_config: Option<HarnessConfig>,
    #[serde(default)]
    pub target_node_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResumeHarnessRequest {
    #[serde(default)]
    pub agent: Option<GenericHarnessAgentSpec>,
    #[serde(default)]
    pub harness_config: Option<HarnessConfig>,
}

fn signal_from_str(value: &str) -> Option<HarnessSignal> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cancel" => Some(HarnessSignal::Cancel),
        "pause" => Some(HarnessSignal::Pause),
        "resume" => Some(HarnessSignal::Resume),
        "checkpoint" => Some(HarnessSignal::Checkpoint),
        _ => None,
    }
}

fn distributed_manager(
    gateway: &Arc<CrabletGateway>,
) -> Result<Arc<DistributedHarnessManager>, StatusCode> {
    gateway
        .distributed_harness
        .as_ref()
        .cloned()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)
}

fn map_distributed_error(error: DistributedError) -> StatusCode {
    match error {
        DistributedError::HarnessNotFound(_) | DistributedError::NodeNotFound(_) => {
            StatusCode::NOT_FOUND
        }
        DistributedError::Timeout(_) => StatusCode::GATEWAY_TIMEOUT,
        DistributedError::ClusterError(_) => StatusCode::BAD_GATEWAY,
        DistributedError::ConnectionError(_) => StatusCode::BAD_GATEWAY,
        DistributedError::LockFailed(_) => StatusCode::CONFLICT,
    }
}

fn serialize_harness(info: crate::agent::harness_manager::HarnessInfo, local: bool) -> Value {
    let mut value = serde_json::to_value(info).unwrap_or_else(|error| {
        json!({
            "serialization_error": error.to_string(),
        })
    });

    if let Value::Object(ref mut object) = value {
        object.insert("local".to_string(), json!(local));
    }

    value
}

fn build_agent(
    router: &Arc<CognitiveRouter>,
    agent: &GenericHarnessAgentSpec,
) -> Result<SharedAgent, DistributedError> {
    let factory = AgentFactory::new(
        router.sys3.coordinator.llm.clone(),
        router.event_bus.clone(),
    );
    factory
        .create_agent_by_role_with_overrides(
            &agent.role,
            agent.system_prompt.as_deref(),
            agent.name.as_deref(),
            agent.description.as_deref(),
        )
        .map_err(|error| DistributedError::ClusterError(error.to_string()))
}

fn config_with_agent_spec(
    mut config: HarnessConfig,
    agent: &GenericHarnessAgentSpec,
) -> Result<HarnessConfig, DistributedError> {
    if let Some(resource_limits) = &agent.resource_limits {
        resource_limits.apply_to_config(&mut config);
    }
    agent.persist_into_metadata(&mut config.metadata)?;
    Ok(config)
}

fn builder_with_agent_spec<A: crate::agent::harness_agent::HarnessAgent>(
    mut builder: HarnessAgentBuilder<A>,
    agent: &GenericHarnessAgentSpec,
) -> HarnessAgentBuilder<A> {
    if let Some(system_prompt) = agent.system_prompt.clone() {
        builder = builder.with_system_prompt(system_prompt);
    }
    if let Some(allowed_tools) = agent.allowed_tools.clone() {
        builder = builder.with_allowed_tools(allowed_tools);
    }
    builder
}

fn merge_harness_metadata(mut config: HarnessConfig, existing: &HarnessConfig) -> HarnessConfig {
    for (key, value) in &existing.metadata {
        config
            .metadata
            .entry(key.clone())
            .or_insert_with(|| value.clone());
    }
    config
}

async fn sync_resume_state_with_agent_spec(
    manager: &Arc<DistributedHarnessManager>,
    harness_id: &str,
    agent: &GenericHarnessAgentSpec,
) -> Result<(), DistributedError> {
    let Some(system_prompt) = agent.system_prompt.clone() else {
        return Ok(());
    };

    let Some(state) = manager
        .local_manager()
        .get_execution_state(harness_id)
        .await
    else {
        return Ok(());
    };

    if state.system_prompt.as_deref() == Some(system_prompt.as_str()) {
        return Ok(());
    }

    let updated_state = HarnessExecutionState {
        system_prompt: Some(system_prompt),
        ..state
    };

    manager
        .local_manager()
        .set_execution_state(harness_id, updated_state)
        .await
        .map_err(|error| DistributedError::ClusterError(error.to_string()))?;

    Ok(())
}

async fn execute_generic_agent_locally(
    router: Arc<CognitiveRouter>,
    manager: Arc<DistributedHarnessManager>,
    request: GenericHarnessRunRequest,
) -> Result<GenericHarnessRunResponse, DistributedError> {
    let agent = build_agent(&router, &request.agent)?;
    let adapter = Arc::new(SharedAgentAdapter::new(agent));
    let config = config_with_agent_spec(
        request.harness_config.clone().unwrap_or_default(),
        &request.agent,
    )?;
    let builder = builder_with_agent_spec(
        HarnessAgentBuilder::new(adapter).with_config(config),
        &request.agent,
    );

    let (harness_id, result) = manager
        .run_agent(&builder, &request.task, &request.context)
        .await?;
    Ok(GenericHarnessRunResponse { harness_id, result })
}

async fn resume_generic_agent_locally(
    router: Arc<CognitiveRouter>,
    manager: Arc<DistributedHarnessManager>,
    request: GenericHarnessResumeRequest,
) -> Result<GenericHarnessResumeResponse, DistributedError> {
    let info = manager
        .get_harness_info(&request.harness_id)
        .await?
        .ok_or_else(|| DistributedError::HarnessNotFound(request.harness_id.clone()))?;
    let agent_spec = request
        .agent
        .clone()
        .or_else(|| GenericHarnessAgentSpec::from_metadata(&info.config.metadata))
        .ok_or_else(|| {
            DistributedError::ClusterError(format!(
                "missing persisted agent spec for harness {}",
                request.harness_id
            ))
        })?;

    let agent = build_agent(&router, &agent_spec)?;
    let adapter = Arc::new(SharedAgentAdapter::new(agent));
    let config = request
        .harness_config
        .clone()
        .map(|config| merge_harness_metadata(config, &info.config))
        .unwrap_or_else(|| info.config.clone());
    let config = config_with_agent_spec(config, &agent_spec)?;
    let builder = builder_with_agent_spec(
        HarnessAgentBuilder::new(adapter).with_config(config),
        &agent_spec,
    );
    sync_resume_state_with_agent_spec(&manager, &request.harness_id, &agent_spec).await?;

    let result = manager.resume_agent(&request.harness_id, &builder).await?;
    Ok(GenericHarnessResumeResponse {
        harness_id: request.harness_id,
        result,
    })
}

async fn active_remote_owner(
    manager: &Arc<DistributedHarnessManager>,
    harness_id: &str,
) -> Result<Option<crate::agent::distributed_harness::NodeInfo>, DistributedError> {
    let info = manager
        .get_harness_info(harness_id)
        .await?
        .ok_or_else(|| DistributedError::HarnessNotFound(harness_id.to_string()))?;
    let Some(owner_node_id) = info.config.metadata.get("node_id").cloned() else {
        return Ok(None);
    };
    if owner_node_id == manager.node_id() {
        return Ok(None);
    }

    Ok(manager
        .list_nodes()
        .await?
        .into_iter()
        .find(|node| node.id == owner_node_id && node.status == NodeStatus::Active))
}

pub async fn register_execution_rpc_handlers(
    rpc: &RpcDispatcher,
    manager: Arc<DistributedHarnessManager>,
    router: Arc<CognitiveRouter>,
) -> Result<(), DistributedError> {
    let run_manager = manager.clone();
    let run_router = router.clone();
    rpc.register("distributed_harness.run", move |params| {
        let manager = run_manager.clone();
        let router = run_router.clone();
        async move {
            let payload: GenericHarnessRunRequest = serde_json::from_value(
                params.ok_or_else(|| RpcError::new(-32602, "Missing params", None))?,
            )
            .map_err(|error| RpcError::new(-32602, &format!("Invalid params: {}", error), None))?;

            let response = execute_generic_agent_locally(router, manager, payload)
                .await
                .map_err(|error| RpcError::new(-32000, &error.to_string(), None))?;

            Ok(Some(serde_json::to_value(response).map_err(|error| {
                RpcError::new(-32603, &format!("Serialization error: {}", error), None)
            })?))
        }
    })
    .await;

    let resume_manager = manager;
    let resume_router = router;
    rpc.register("distributed_harness.resume", move |params| {
        let manager = resume_manager.clone();
        let router = resume_router.clone();
        async move {
            let payload: GenericHarnessResumeRequest = serde_json::from_value(
                params.ok_or_else(|| RpcError::new(-32602, "Missing params", None))?,
            )
            .map_err(|error| RpcError::new(-32602, &format!("Invalid params: {}", error), None))?;

            let response = resume_generic_agent_locally(router, manager, payload)
                .await
                .map_err(|error| RpcError::new(-32000, &error.to_string(), None))?;

            Ok(Some(serde_json::to_value(response).map_err(|error| {
                RpcError::new(-32603, &format!("Serialization error: {}", error), None)
            })?))
        }
    })
    .await;

    Ok(())
}

pub async fn get_cluster_status(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    let Some(manager) = gateway.distributed_harness.as_ref() else {
        return Json(json!({
            "status": "success",
            "distributed_enabled": false,
        }));
    };

    match manager.get_cluster_stats().await {
        Ok(cluster) => Json(json!({
            "status": "success",
            "distributed_enabled": true,
            "node_id": manager.node_id(),
            "node_address": manager.node_address(),
            "node_port": manager.node_port(),
            "cluster": cluster,
        })),
        Err(error) => Json(json!({
            "status": "error",
            "distributed_enabled": true,
            "error": error.to_string(),
        })),
    }
}

pub async fn list_harnesses(State(gateway): State<Arc<CrabletGateway>>) -> Json<serde_json::Value> {
    let Some(manager) = gateway.distributed_harness.as_ref() else {
        return Json(json!({
            "status": "success",
            "distributed_enabled": false,
            "count": 0,
            "harnesses": [],
        }));
    };

    match (
        manager.list_all_harnesses().await,
        manager.get_local_harness_ids().await,
    ) {
        (Ok(harnesses), local_ids) => {
            let local_ids: HashSet<String> = local_ids.into_iter().collect();
            let harnesses: Vec<Value> = harnesses
                .into_iter()
                .map(|info| {
                    let is_local = local_ids.contains(&info.id);
                    let mut value = serde_json::to_value(info).unwrap_or_else(|error| {
                        json!({
                            "serialization_error": error.to_string(),
                            "local": is_local,
                        })
                    });

                    if let Value::Object(ref mut object) = value {
                        object.insert("local".to_string(), json!(is_local));
                    }

                    value
                })
                .collect();

            Json(json!({
                "status": "success",
                "distributed_enabled": true,
                "node_id": manager.node_id(),
                "count": harnesses.len(),
                "harnesses": harnesses,
            }))
        }
        (Err(error), _) => Json(json!({
            "status": "error",
            "distributed_enabled": true,
            "error": error.to_string(),
        })),
    }
}

pub async fn get_harness(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let manager = distributed_manager(&gateway)?;
    let info = manager
        .get_harness_info(&id)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let local = manager.is_local(&id).await;

    Ok(Json(json!({
        "status": "success",
        "distributed_enabled": true,
        "node_id": manager.node_id(),
        "harness": serialize_harness(info, local),
    })))
}

pub async fn create_harness(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(payload): Json<CreateHarnessRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let manager = distributed_manager(&gateway)?;
    let harness_id = manager
        .create_harness_distributed(payload.config, payload.target_node_id.as_deref())
        .await
        .map_err(map_distributed_error)?;
    let info = manager
        .get_harness_info(&harness_id)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(json!({
        "status": "success",
        "distributed_enabled": true,
        "node_id": manager.node_id(),
        "harness_id": harness_id,
        "target_node_id": info.config.metadata.get("node_id").cloned(),
        "harness": serialize_harness(info, manager.is_local(&harness_id).await),
    })))
}

pub async fn execute_harness(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(payload): Json<ExecuteHarnessRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let manager = distributed_manager(&gateway)?;
    let target_node_id = payload.target_node_id.clone();
    let request = GenericHarnessRunRequest {
        task: payload.task,
        context: payload.context,
        agent: payload.agent,
        harness_config: payload.harness_config,
        target_node_id: target_node_id.clone(),
    };

    let target_node = manager
        .select_target_node(target_node_id.as_deref())
        .await
        .map_err(map_distributed_error)?;
    let response = if target_node.id == manager.node_id() {
        execute_generic_agent_locally(gateway.router.clone(), manager.clone(), request)
            .await
            .map_err(map_distributed_error)?
    } else {
        manager
            .forward_generic_run(&target_node, request)
            .await
            .map_err(map_distributed_error)?
    };

    let info = manager
        .get_harness_info(&response.harness_id)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(json!({
        "status": "success",
        "distributed_enabled": true,
        "node_id": manager.node_id(),
        "harness_id": response.harness_id,
        "result": response.result,
        "harness": serialize_harness(info, manager.is_local(&response.harness_id).await),
    })))
}

pub async fn resume_harness(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(id): Path<String>,
    Json(payload): Json<ResumeHarnessRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let manager = distributed_manager(&gateway)?;
    let request = GenericHarnessResumeRequest {
        harness_id: id.clone(),
        agent: payload.agent,
        harness_config: payload.harness_config,
    };

    let response = if let Some(owner_node) = active_remote_owner(&manager, &id)
        .await
        .map_err(map_distributed_error)?
    {
        manager
            .forward_generic_resume(&owner_node, request)
            .await
            .map_err(map_distributed_error)?
    } else {
        resume_generic_agent_locally(gateway.router.clone(), manager.clone(), request)
            .await
            .map_err(map_distributed_error)?
    };

    let info = manager
        .get_harness_info(&response.harness_id)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(json!({
        "status": "success",
        "distributed_enabled": true,
        "node_id": manager.node_id(),
        "harness_id": response.harness_id,
        "result": response.result,
        "harness": serialize_harness(info, manager.is_local(&response.harness_id).await),
    })))
}

pub async fn send_harness_signal(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(id): Path<String>,
    Json(payload): Json<HarnessSignalRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let manager = distributed_manager(&gateway)?;
    let signal = signal_from_str(&payload.signal).ok_or(StatusCode::BAD_REQUEST)?;

    manager
        .send_signal(&id, signal)
        .await
        .map_err(map_distributed_error)?;

    let info = manager
        .get_harness_info(&id)
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(json!({
        "status": "success",
        "distributed_enabled": true,
        "node_id": manager.node_id(),
        "harness_id": id,
        "signal": payload.signal.to_ascii_lowercase(),
        "harness": serialize_harness(info, manager.is_local(&id).await),
    })))
}

pub async fn delete_harness(
    State(gateway): State<Arc<CrabletGateway>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let manager = distributed_manager(&gateway)?;

    manager
        .remove_harness(&id)
        .await
        .map_err(|error| match error {
            DistributedError::ClusterError(message)
                if message.contains("must be terminal before removal") =>
            {
                StatusCode::CONFLICT
            }
            other => map_distributed_error(other),
        })?;

    Ok(Json(json!({
        "status": "success",
        "distributed_enabled": true,
        "node_id": manager.node_id(),
        "harness_id": id,
        "deleted": true,
    })))
}

pub async fn list_nodes(State(gateway): State<Arc<CrabletGateway>>) -> Json<serde_json::Value> {
    let Some(manager) = gateway.distributed_harness.as_ref() else {
        return Json(json!({
            "status": "success",
            "distributed_enabled": false,
            "count": 0,
            "nodes": [],
        }));
    };

    match manager.list_nodes().await {
        Ok(nodes) => Json(json!({
            "status": "success",
            "distributed_enabled": true,
            "node_id": manager.node_id(),
            "count": nodes.len(),
            "nodes": nodes,
        })),
        Err(error) => Json(json!({
            "status": "error",
            "distributed_enabled": true,
            "error": error.to_string(),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::distributed_harness::{
        BackendType, DistributedConfig, DistributedHarnessManager, HarnessBackend, InMemoryBackend,
    };
    use crate::agent::harness_agent::HarnessExecutionState;
    use crate::agent::harness_manager::HarnessStatus;
    use crate::cognitive::router::CognitiveRouter;
    use crate::config::Config;
    use crate::events::EventBus;
    use crate::gateway::rpc::RpcDispatcher;
    use crate::gateway::types::GatewayConfig;
    use crate::gateway::types::RpcRequest;

    async fn test_gateway() -> Arc<CrabletGateway> {
        let mut config = Config::default();
        config.llm_vendor = Some("mock".to_string());
        let event_bus = Arc::new(EventBus::new(100));
        let router = Arc::new(CognitiveRouter::new(&config, None, event_bus).await);
        let gateway = CrabletGateway::new(
            GatewayConfig {
                host: "127.0.0.1".to_string(),
                port: 18790,
                auth_mode: "off".to_string(),
            },
            router,
            tokio_util::sync::CancellationToken::new(),
        )
        .await
        .expect("test gateway should initialize");

        Arc::new(gateway)
    }

    fn test_manager() -> Arc<DistributedHarnessManager> {
        let backend: Arc<dyn HarnessBackend> = Arc::new(InMemoryBackend::new());
        Arc::new(DistributedHarnessManager::new(
            backend,
            DistributedConfig {
                node_id: "node-test".to_string(),
                node_address: "127.0.0.1".to_string(),
                node_port: 18790,
                backend_type: BackendType::InMemory,
                backend_uri: "memory://".to_string(),
                lock_ttl_secs: 300,
                heartbeat_interval_secs: 30,
                node_timeout_secs: 60,
            },
        ))
    }

    #[tokio::test]
    async fn test_get_cluster_status_when_disabled() {
        let gateway = test_gateway().await;
        let payload = get_cluster_status(State(gateway)).await.0;

        assert_eq!(payload["status"], "success");
        assert_eq!(payload["distributed_enabled"], false);
    }

    #[tokio::test]
    async fn test_harness_endpoints_when_enabled() {
        let manager = test_manager();
        manager
            .register_local_node()
            .await
            .expect("local harness node should register");
        let harness_id = manager
            .create_harness(None)
            .await
            .expect("test harness should be created");

        let gateway = test_gateway().await;
        let gateway = Arc::new((*gateway).clone().with_distributed_harness(manager));

        let cluster = get_cluster_status(State(gateway.clone())).await.0;
        assert_eq!(cluster["status"], "success");
        assert_eq!(cluster["distributed_enabled"], true);
        assert_eq!(cluster["cluster"]["active_nodes"], 1);
        assert_eq!(cluster["cluster"]["total_harnesses"], 1);

        let harnesses = list_harnesses(State(gateway.clone())).await.0;
        assert_eq!(harnesses["status"], "success");
        assert_eq!(harnesses["count"], 1);
        assert_eq!(harnesses["harnesses"][0]["local"], true);

        let detail = get_harness(State(gateway.clone()), Path(harness_id.clone()))
            .await
            .expect("existing harness should be returned")
            .0;
        assert_eq!(detail["status"], "success");
        assert_eq!(detail["harness"]["id"], harness_id);

        let nodes = list_nodes(State(gateway)).await.0;
        assert_eq!(nodes["status"], "success");
        assert_eq!(nodes["count"], 1);
        assert_eq!(nodes["nodes"][0]["id"], "node-test");
    }

    #[tokio::test]
    async fn test_create_harness_endpoint_creates_harness() {
        let manager = test_manager();
        manager
            .register_local_node()
            .await
            .expect("local harness node should register");

        let gateway = test_gateway().await;
        let gateway = Arc::new((*gateway).clone().with_distributed_harness(manager.clone()));

        let response = create_harness(
            State(gateway),
            Json(CreateHarnessRequest {
                config: Some(HarnessConfig {
                    max_steps: 42,
                    ..Default::default()
                }),
                target_node_id: None,
            }),
        )
        .await
        .expect("create harness endpoint should succeed")
        .0;

        let harness_id = response["harness_id"]
            .as_str()
            .expect("create harness response should include a harness_id")
            .to_string();
        let info = manager
            .get_harness_info(&harness_id)
            .await
            .expect("harness lookup should succeed")
            .expect("created harness should exist");

        assert_eq!(response["status"], "success");
        assert_eq!(response["target_node_id"], "node-test");
        assert_eq!(info.config.max_steps, 42);
    }

    #[tokio::test]
    async fn test_execute_harness_runs_generic_agent() {
        let manager = test_manager();
        manager
            .register_local_node()
            .await
            .expect("local harness node should register");

        let gateway = test_gateway().await;
        let gateway = Arc::new((*gateway).clone().with_distributed_harness(manager.clone()));
        let agent = GenericHarnessAgentSpec {
            role: "custom-runner".to_string(),
            name: Some("Custom Runner".to_string()),
            description: Some("custom harness agent".to_string()),
            system_prompt: Some("You are a custom harness agent.".to_string()),
            allowed_tools: Some(vec!["echo_tool".to_string()]),
            resource_limits: Some(
                crate::agent::distributed_harness::GenericHarnessResourceLimits {
                    max_steps: Some(3),
                    tool_timeout_ms: Some(1_500),
                    step_timeout_ms: Some(2_500),
                    max_memory_bytes: Some(1_048_576),
                    max_cpu_time_ms: Some(5_000),
                },
            ),
        };

        let built_agent =
            build_agent(&gateway.router, &agent).expect("generic harness agent should build");
        assert_eq!(built_agent.name(), "Custom Runner");
        assert_eq!(built_agent.description(), "custom harness agent");

        let response = execute_harness(
            State(gateway),
            Json(ExecuteHarnessRequest {
                task: "say hi".to_string(),
                context: Vec::new(),
                agent,
                harness_config: Some(HarnessConfig {
                    max_steps: 2,
                    ..Default::default()
                }),
                target_node_id: None,
            }),
        )
        .await
        .expect("execute harness endpoint should succeed")
        .0;

        let harness_id = response["harness_id"]
            .as_str()
            .expect("execute harness response should include a harness_id")
            .to_string();
        let info = manager
            .get_harness_info(&harness_id)
            .await
            .expect("harness lookup should succeed")
            .expect("executed harness should exist");

        assert_eq!(response["status"], "success");
        assert_eq!(response["result"]["success"], true);
        assert_eq!(
            response["result"]["output"],
            "Step 1: (Mock LLM) Processed: say hi"
        );
        assert_eq!(info.config.max_steps, 3);
        assert_eq!(info.config.tool_timeout.as_millis(), 1_500);
        assert_eq!(info.config.step_timeout.as_millis(), 2_500);
        assert_eq!(info.config.max_memory_bytes, Some(1_048_576));
        assert_eq!(info.config.max_cpu_time_ms, Some(5_000));
        assert_eq!(
            info.execution_state
                .as_ref()
                .and_then(|state| state.system_prompt.as_deref()),
            Some("You are a custom harness agent.")
        );
        assert_eq!(
            GenericHarnessAgentSpec::from_metadata(&info.config.metadata)
                .expect("generic harness agent spec should round-trip from metadata"),
            GenericHarnessAgentSpec {
                role: "custom-runner".to_string(),
                name: Some("Custom Runner".to_string()),
                description: Some("custom harness agent".to_string()),
                system_prompt: Some("You are a custom harness agent.".to_string()),
                allowed_tools: Some(vec!["echo_tool".to_string()]),
                resource_limits: Some(
                    crate::agent::distributed_harness::GenericHarnessResourceLimits {
                        max_steps: Some(3),
                        tool_timeout_ms: Some(1_500),
                        step_timeout_ms: Some(2_500),
                        max_memory_bytes: Some(1_048_576),
                        max_cpu_time_ms: Some(5_000),
                    },
                ),
            }
        );
        assert!(matches!(info.status, HarnessStatus::Completed));
    }

    #[tokio::test]
    async fn test_resume_harness_runs_from_saved_state() {
        let manager = test_manager();
        manager
            .register_local_node()
            .await
            .expect("local harness node should register");
        let mut harness_config = HarnessConfig::default();
        let persisted_agent = GenericHarnessAgentSpec {
            role: "custom-runner".to_string(),
            name: Some("Resume Runner".to_string()),
            description: Some("resume harness agent".to_string()),
            system_prompt: Some("You are a resume harness agent.".to_string()),
            allowed_tools: Some(vec!["echo_tool".to_string()]),
            resource_limits: Some(
                crate::agent::distributed_harness::GenericHarnessResourceLimits {
                    max_steps: Some(4),
                    tool_timeout_ms: Some(1_000),
                    step_timeout_ms: Some(2_000),
                    max_memory_bytes: Some(2_097_152),
                    max_cpu_time_ms: Some(10_000),
                },
            ),
        };
        persisted_agent
            .persist_into_metadata(&mut harness_config.metadata)
            .expect("generic harness agent spec should persist into metadata");
        let harness_id = manager
            .create_harness(Some(harness_config))
            .await
            .expect("paused harness should be created");
        manager
            .local_manager()
            .set_execution_state(
                &harness_id,
                HarnessExecutionState::new("resume task", &[], None),
            )
            .await
            .expect("execution state should be stored");
        manager
            .local_manager()
            .update_status(&harness_id, HarnessStatus::Paused)
            .await;

        let gateway = test_gateway().await;
        let gateway = Arc::new((*gateway).clone().with_distributed_harness(manager.clone()));

        let response = resume_harness(
            State(gateway),
            Path(harness_id.clone()),
            Json(ResumeHarnessRequest {
                agent: None,
                harness_config: None,
            }),
        )
        .await
        .expect("resume harness endpoint should succeed")
        .0;

        let info = manager
            .get_harness_info(&harness_id)
            .await
            .expect("harness lookup should succeed")
            .expect("resumed harness should exist");

        assert_eq!(response["status"], "success");
        assert_eq!(response["result"]["success"], true);
        assert_eq!(
            response["result"]["output"],
            "Step 1: (Mock LLM) Processed: resume task"
        );
        assert_eq!(info.config.max_steps, 4);
        assert_eq!(info.config.tool_timeout.as_millis(), 1_000);
        assert_eq!(info.config.step_timeout.as_millis(), 2_000);
        assert_eq!(
            info.execution_state
                .as_ref()
                .and_then(|state| state.system_prompt.as_deref()),
            Some("You are a resume harness agent.")
        );
        assert_eq!(
            GenericHarnessAgentSpec::from_metadata(&info.config.metadata)
                .expect("generic harness agent spec should round-trip from metadata"),
            persisted_agent
        );
        assert!(matches!(info.status, HarnessStatus::Completed));
    }

    #[tokio::test]
    async fn test_register_execution_rpc_handlers_dispatches_run() {
        let manager = test_manager();
        manager
            .register_local_node()
            .await
            .expect("local harness node should register");
        let gateway = test_gateway().await;
        let rpc = RpcDispatcher::new();

        register_execution_rpc_handlers(&rpc, manager, gateway.router.clone())
            .await
            .expect("distributed harness RPC handlers should register");

        let response = rpc
            .dispatch(RpcRequest {
                jsonrpc: "2.0".to_string(),
                method: "distributed_harness.run".to_string(),
                params: Some(
                    serde_json::to_value(GenericHarnessRunRequest {
                        task: "rpc task".to_string(),
                        context: Vec::new(),
                        agent: GenericHarnessAgentSpec {
                            role: "rpc-runner".to_string(),
                            ..Default::default()
                        },
                        harness_config: None,
                        target_node_id: None,
                    })
                    .expect("generic harness run request should serialize"),
                ),
                id: Some("rpc-run".to_string()),
            })
            .await;

        assert!(response.error.is_none());
        let result = response
            .result
            .as_ref()
            .expect("successful RPC response should include a result");
        assert_eq!(result["result"]["success"], true);
        assert_eq!(
            result["result"]["output"],
            "Step 1: (Mock LLM) Processed: rpc task"
        );
    }

    #[tokio::test]
    async fn test_send_harness_signal_updates_status() {
        let manager = test_manager();
        manager
            .register_local_node()
            .await
            .expect("local harness node should register");
        let harness_id = manager
            .create_harness(None)
            .await
            .expect("test harness should be created");

        let gateway = test_gateway().await;
        let gateway = Arc::new((*gateway).clone().with_distributed_harness(manager));

        let response = send_harness_signal(
            State(gateway),
            Path(harness_id),
            Json(HarnessSignalRequest {
                signal: "pause".to_string(),
            }),
        )
        .await
        .expect("pause signal should succeed")
        .0;

        assert_eq!(response["status"], "success");
        assert_eq!(response["signal"], "pause");
        assert_eq!(response["harness"]["status"], "Paused");
    }

    #[tokio::test]
    async fn test_send_harness_signal_rejects_invalid_signal() {
        let manager = test_manager();
        manager
            .register_local_node()
            .await
            .expect("local harness node should register");
        let harness_id = manager
            .create_harness(None)
            .await
            .expect("test harness should be created");

        let gateway = test_gateway().await;
        let gateway = Arc::new((*gateway).clone().with_distributed_harness(manager));

        let err = send_harness_signal(
            State(gateway),
            Path(harness_id),
            Json(HarnessSignalRequest {
                signal: "not-a-signal".to_string(),
            }),
        )
        .await
        .expect_err("invalid signal should be rejected");

        assert_eq!(err, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_delete_terminal_harness() {
        let manager = test_manager();
        manager
            .register_local_node()
            .await
            .expect("local harness node should register");
        let harness_id = manager
            .create_harness(None)
            .await
            .expect("test harness should be created");
        manager
            .local_manager()
            .update_status(&harness_id, HarnessStatus::Completed)
            .await;

        let gateway = test_gateway().await;
        let gateway = Arc::new((*gateway).clone().with_distributed_harness(manager.clone()));

        let response = delete_harness(State(gateway), Path(harness_id.clone()))
            .await
            .expect("completed harness should be deleted")
            .0;

        assert_eq!(response["status"], "success");
        assert_eq!(response["deleted"], true);
        assert!(manager
            .get_harness_info(&harness_id)
            .await
            .expect("harness lookup should succeed")
            .is_none());
    }

    #[tokio::test]
    async fn test_delete_running_harness_conflicts() {
        let manager = test_manager();
        manager
            .register_local_node()
            .await
            .expect("local harness node should register");
        let harness_id = manager
            .create_harness(None)
            .await
            .expect("test harness should be created");
        manager
            .local_manager()
            .update_status(&harness_id, HarnessStatus::Running)
            .await;

        let gateway = test_gateway().await;
        let gateway = Arc::new((*gateway).clone().with_distributed_harness(manager));

        let err = delete_harness(State(gateway), Path(harness_id))
            .await
            .expect_err("running harness deletion should conflict");

        assert_eq!(err, StatusCode::CONFLICT);
    }
}
