//! Control plane implementations for distributed harness coordination
//!
//! Provides:
//! - HarnessControlPlane trait for forwarding commands to remote owners
//! - NoopHarnessControlPlane (default no-op implementation)
//! - HttpHarnessControlPlane (HTTP/JSON-RPC transport)
//! - InProcessHarnessControlPlane (in-process for tests/embedded)
//! - Generic harness request/response types

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use super::harness::{HarnessConfig, HarnessSignal};
use super::harness_agent::HarnessAgentResult;
use super::types::{DistributedError, NodeInfo};

use crate::types::Message;

/// Optional control plane used to forward commands to a live remote owner.
#[async_trait]
pub trait HarnessControlPlane: Send + Sync {
    async fn forward_create(
        &self,
        node: &NodeInfo,
        config: Option<HarnessConfig>,
    ) -> Result<String, DistributedError>;

    async fn forward_run(
        &self,
        node: &NodeInfo,
        request: GenericHarnessRunRequest,
    ) -> Result<GenericHarnessRunResponse, DistributedError>;

    async fn forward_resume(
        &self,
        node: &NodeInfo,
        request: GenericHarnessResumeRequest,
    ) -> Result<GenericHarnessResumeResponse, DistributedError>;

    async fn forward_signal(
        &self,
        node: &NodeInfo,
        harness_id: &str,
        signal: HarnessSignal,
    ) -> Result<(), DistributedError>;

    async fn forward_delete(
        &self,
        node: &NodeInfo,
        harness_id: &str,
    ) -> Result<(), DistributedError>;
}

#[derive(Default)]
pub(crate) struct NoopHarnessControlPlane;

#[async_trait]
impl HarnessControlPlane for NoopHarnessControlPlane {
    async fn forward_create(
        &self,
        node: &NodeInfo,
        config: Option<HarnessConfig>,
    ) -> Result<String, DistributedError> {
        let _ = (node, config);
        Err(DistributedError::ClusterError(
            "remote control plane unavailable".to_string(),
        ))
    }

    async fn forward_run(
        &self,
        node: &NodeInfo,
        request: GenericHarnessRunRequest,
    ) -> Result<GenericHarnessRunResponse, DistributedError> {
        let _ = (node, request);
        Err(DistributedError::ClusterError(
            "remote control plane unavailable".to_string(),
        ))
    }

    async fn forward_resume(
        &self,
        node: &NodeInfo,
        request: GenericHarnessResumeRequest,
    ) -> Result<GenericHarnessResumeResponse, DistributedError> {
        let _ = (node, request);
        Err(DistributedError::ClusterError(
            "remote control plane unavailable".to_string(),
        ))
    }

    async fn forward_signal(
        &self,
        node: &NodeInfo,
        harness_id: &str,
        _signal: HarnessSignal,
    ) -> Result<(), DistributedError> {
        Err(DistributedError::ClusterError(format!(
            "remote control plane unavailable for node {} and harness {}",
            node.id, harness_id
        )))
    }

    async fn forward_delete(
        &self,
        node: &NodeInfo,
        harness_id: &str,
    ) -> Result<(), DistributedError> {
        Err(DistributedError::ClusterError(format!(
            "remote control plane unavailable for node {} and harness {}",
            node.id, harness_id
        )))
    }
}

// ============================================
// Request/Response Types
// ============================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HarnessCreateRequest {
    pub config: Option<HarnessConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HarnessCreateResponse {
    pub harness_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HarnessDeleteRequest {
    pub harness_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HarnessDeleteResponse {
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct GenericHarnessResourceLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_steps: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_memory_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cpu_time_ms: Option<u64>,
}

impl GenericHarnessResourceLimits {
    pub fn apply_to_config(&self, config: &mut HarnessConfig) {
        if let Some(max_steps) = self.max_steps {
            config.max_steps = max_steps;
        }
        if let Some(tool_timeout_ms) = self.tool_timeout_ms {
            config.tool_timeout = Duration::from_millis(tool_timeout_ms);
        }
        if let Some(step_timeout_ms) = self.step_timeout_ms {
            config.step_timeout = Duration::from_millis(step_timeout_ms);
        }
        if let Some(max_memory_bytes) = self.max_memory_bytes {
            config.max_memory_bytes = Some(max_memory_bytes);
        }
        if let Some(max_cpu_time_ms) = self.max_cpu_time_ms {
            config.max_cpu_time_ms = Some(max_cpu_time_ms);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct GenericHarnessAgentSpec {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_limits: Option<GenericHarnessResourceLimits>,
}

const AGENT_SPEC_METADATA_KEY: &str = "crablet.agent_spec";
const AGENT_ROLE_METADATA_KEY: &str = "crablet.agent_role";
const AGENT_NAME_METADATA_KEY: &str = "crablet.agent_name";
const AGENT_DESCRIPTION_METADATA_KEY: &str = "crablet.agent_description";
const AGENT_SYSTEM_PROMPT_METADATA_KEY: &str = "crablet.agent_system_prompt";

impl GenericHarnessAgentSpec {
    pub fn from_metadata(metadata: &HashMap<String, String>) -> Option<Self> {
        if let Some(raw) = metadata.get(AGENT_SPEC_METADATA_KEY) {
            if let Ok(spec) = serde_json::from_str(raw) {
                return Some(spec);
            }
        }

        let role = metadata.get(AGENT_ROLE_METADATA_KEY)?.clone();
        Some(Self {
            role,
            name: metadata.get(AGENT_NAME_METADATA_KEY).cloned(),
            description: metadata.get(AGENT_DESCRIPTION_METADATA_KEY).cloned(),
            system_prompt: metadata.get(AGENT_SYSTEM_PROMPT_METADATA_KEY).cloned(),
            allowed_tools: None,
            resource_limits: None,
        })
    }

    pub fn persist_into_metadata(
        &self,
        metadata: &mut HashMap<String, String>,
    ) -> Result<(), DistributedError> {
        let encoded = serde_json::to_string(self).map_err(|error| {
            DistributedError::ClusterError(format!("failed to serialize agent spec: {}", error))
        })?;

        metadata.insert(AGENT_SPEC_METADATA_KEY.to_string(), encoded);
        metadata.insert(AGENT_ROLE_METADATA_KEY.to_string(), self.role.clone());

        if let Some(name) = &self.name {
            metadata.insert(AGENT_NAME_METADATA_KEY.to_string(), name.clone());
        } else {
            metadata.remove(AGENT_NAME_METADATA_KEY);
        }

        if let Some(description) = &self.description {
            metadata.insert(
                AGENT_DESCRIPTION_METADATA_KEY.to_string(),
                description.clone(),
            );
        } else {
            metadata.remove(AGENT_DESCRIPTION_METADATA_KEY);
        }

        if let Some(system_prompt) = &self.system_prompt {
            metadata.insert(
                AGENT_SYSTEM_PROMPT_METADATA_KEY.to_string(),
                system_prompt.clone(),
            );
        } else {
            metadata.remove(AGENT_SYSTEM_PROMPT_METADATA_KEY);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericHarnessRunRequest {
    pub task: String,
    #[serde(default)]
    pub context: Vec<Message>,
    pub agent: GenericHarnessAgentSpec,
    #[serde(default)]
    pub harness_config: Option<HarnessConfig>,
    #[serde(default)]
    pub target_node_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericHarnessRunResponse {
    pub harness_id: String,
    pub result: HarnessAgentResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericHarnessResumeRequest {
    pub harness_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<GenericHarnessAgentSpec>,
    #[serde(default)]
    pub harness_config: Option<HarnessConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericHarnessResumeResponse {
    pub harness_id: String,
    pub result: HarnessAgentResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HarnessSignalRequest {
    pub harness_id: String,
    pub signal: HarnessSignal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct HarnessSignalResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ControlPlaneRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ControlPlaneRpcResponse {
    pub jsonrpc: String,
    pub id: Option<String>,
    pub result: Option<serde_json::Value>,
    pub error: Option<ControlPlaneRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ControlPlaneRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

// ============================================
// HTTP Control Plane
// ============================================

/// HTTP transport for live remote owner forwarding.
pub struct HttpHarnessControlPlane {
    client: reqwest::Client,
    rpc_path: String,
    bearer_token: Option<String>,
}

impl Default for HttpHarnessControlPlane {
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
            rpc_path: "/rpc".to_string(),
            bearer_token: None,
        }
    }
}

impl HttpHarnessControlPlane {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_rpc_path(mut self, rpc_path: impl Into<String>) -> Self {
        self.rpc_path = rpc_path.into();
        self
    }

    pub fn with_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    async fn invoke_rpc<T, P>(
        &self,
        node: &NodeInfo,
        method: &str,
        params: Option<P>,
    ) -> Result<T, DistributedError>
    where
        T: DeserializeOwned,
        P: Serialize,
    {
        let url = format!("http://{}:{}{}", node.address, node.port, self.rpc_path);
        let request = ControlPlaneRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: params
                .map(serde_json::to_value)
                .transpose()
                .map_err(|error| DistributedError::ClusterError(error.to_string()))?,
            id: Some(uuid::Uuid::new_v4().to_string()),
        };

        let mut builder = self.client.post(url).json(&request);
        if let Some(token) = &self.bearer_token {
            builder = builder.bearer_auth(token);
        }

        let response = builder
            .send()
            .await
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;

        if !status.is_success() {
            return Err(DistributedError::ClusterError(format!(
                "remote rpc {} failed with {}: {}",
                method, status, body
            )));
        }

        let response: ControlPlaneRpcResponse = serde_json::from_str(&body)
            .map_err(|error| DistributedError::ConnectionError(error.to_string()))?;
        if let Some(error) = response.error {
            return Err(DistributedError::ClusterError(format!(
                "rpc error {}: {}",
                error.code, error.message
            )));
        }

        let result = response
            .result
            .ok_or_else(|| DistributedError::ClusterError("missing rpc result".to_string()))?;
        serde_json::from_value(result)
            .map_err(|error| DistributedError::ClusterError(error.to_string()))
    }
}

#[async_trait]
impl HarnessControlPlane for HttpHarnessControlPlane {
    async fn forward_create(
        &self,
        node: &NodeInfo,
        config: Option<HarnessConfig>,
    ) -> Result<String, DistributedError> {
        let response: HarnessCreateResponse = self
            .invoke_rpc(
                node,
                "distributed_harness.create",
                Some(HarnessCreateRequest { config }),
            )
            .await?;
        Ok(response.harness_id)
    }

    async fn forward_run(
        &self,
        node: &NodeInfo,
        request: GenericHarnessRunRequest,
    ) -> Result<GenericHarnessRunResponse, DistributedError> {
        self.invoke_rpc(node, "distributed_harness.run", Some(request))
            .await
    }

    async fn forward_resume(
        &self,
        node: &NodeInfo,
        request: GenericHarnessResumeRequest,
    ) -> Result<GenericHarnessResumeResponse, DistributedError> {
        self.invoke_rpc(node, "distributed_harness.resume", Some(request))
            .await
    }

    async fn forward_signal(
        &self,
        node: &NodeInfo,
        harness_id: &str,
        signal: HarnessSignal,
    ) -> Result<(), DistributedError> {
        let _: HarnessSignalResponse = self
            .invoke_rpc(
                node,
                "distributed_harness.signal",
                Some(HarnessSignalRequest {
                    harness_id: harness_id.to_string(),
                    signal,
                }),
            )
            .await?;
        Ok(())
    }

    async fn forward_delete(
        &self,
        node: &NodeInfo,
        harness_id: &str,
    ) -> Result<(), DistributedError> {
        let _: HarnessDeleteResponse = self
            .invoke_rpc(
                node,
                "distributed_harness.delete",
                Some(HarnessDeleteRequest {
                    harness_id: harness_id.to_string(),
                }),
            )
            .await?;
        Ok(())
    }
}

// ============================================
// In-Process Control Plane
// ============================================

/// In-process control plane used for tests or embedded multi-node runtimes.
#[derive(Default)]
pub struct InProcessHarnessControlPlane {
    managers: Arc<RwLock<HashMap<super::types::NodeId, Weak<super::manager::DistributedHarnessManager>>>>,
}

use std::sync::Weak;

impl InProcessHarnessControlPlane {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register_manager(&self, manager: &Arc<super::manager::DistributedHarnessManager>) {
        self.managers
            .write()
            .await
            .insert(manager.node_id().to_string(), Arc::downgrade(manager));
    }

    pub async fn unregister_node(&self, node_id: &str) {
        self.managers.write().await.remove(node_id);
    }
}

#[async_trait]
impl HarnessControlPlane for InProcessHarnessControlPlane {
    async fn forward_create(
        &self,
        node: &NodeInfo,
        config: Option<HarnessConfig>,
    ) -> Result<String, DistributedError> {
        let manager = self
            .managers
            .read()
            .await
            .get(&node.id)
            .cloned()
            .ok_or_else(|| DistributedError::NodeNotFound(node.id.clone()))?
            .upgrade()
            .ok_or_else(|| DistributedError::NodeNotFound(node.id.clone()))?;

        manager.create_harness(config).await
    }

    async fn forward_run(
        &self,
        _node: &NodeInfo,
        _request: GenericHarnessRunRequest,
    ) -> Result<GenericHarnessRunResponse, DistributedError> {
        Err(DistributedError::ClusterError(
            "in-process control plane does not support remote run".to_string(),
        ))
    }

    async fn forward_resume(
        &self,
        _node: &NodeInfo,
        _request: GenericHarnessResumeRequest,
    ) -> Result<GenericHarnessResumeResponse, DistributedError> {
        Err(DistributedError::ClusterError(
            "in-process control plane does not support remote resume".to_string(),
        ))
    }

    async fn forward_signal(
        &self,
        node: &NodeInfo,
        harness_id: &str,
        signal: HarnessSignal,
    ) -> Result<(), DistributedError> {
        let manager = self
            .managers
            .read()
            .await
            .get(&node.id)
            .cloned()
            .ok_or_else(|| DistributedError::NodeNotFound(node.id.clone()))?
            .upgrade()
            .ok_or_else(|| DistributedError::NodeNotFound(node.id.clone()))?;

        manager.apply_local_signal(harness_id, signal).await
    }

    async fn forward_delete(
        &self,
        node: &NodeInfo,
        harness_id: &str,
    ) -> Result<(), DistributedError> {
        let manager = self
            .managers
            .read()
            .await
            .get(&node.id)
            .cloned()
            .ok_or_else(|| DistributedError::NodeNotFound(node.id.clone()))?
            .upgrade()
            .ok_or_else(|| DistributedError::NodeNotFound(node.id.clone()))?;

        manager.remove_harness(harness_id).await
    }
}
