//! Distributed harness control plane
//!
//! Abstraction for forwarding commands to remote nodes:
//! - `HarnessControlPlane` trait for cross-node operations
//! - `NoopHarnessControlPlane` for single-node / no-remote scenarios
//! - `HttpHarnessControlPlane` for HTTP/RPC-based remote forwarding
//! - `InProcessHarnessControlPlane` for in-process multi-node testing

use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Weak};
use tokio::sync::RwLock;

use super::distributed_types::{
    DistributedError, GenericHarnessResumeRequest, GenericHarnessResumeResponse,
    GenericHarnessRunRequest, GenericHarnessRunResponse, NodeId, NodeInfo,
};
use crate::agent::harness::HarnessConfig;
use crate::agent::harness::HarnessSignal;

// ---- Control Plane Trait ----

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
        signal: crate::agent::harness::HarnessSignal,
    ) -> Result<(), DistributedError>;

    async fn forward_delete(
        &self,
        node: &NodeInfo,
        harness_id: &str,
    ) -> Result<(), DistributedError>;
}

// ---- Noop Implementation ----

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

// ---- RPC Request/Response Types ----

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
struct ControlPlaneRpcRequest {
    jsonrpc: String,
    method: String,
    params: Option<serde_json::Value>,
    id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ControlPlaneRpcResponse {
    jsonrpc: String,
    id: Option<String>,
    result: Option<serde_json::Value>,
    error: Option<ControlPlaneRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ControlPlaneRpcError {
    code: i32,
    message: String,
    data: Option<serde_json::Value>,
}

// ---- HTTP Control Plane ----

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
        signal: crate::agent::harness::HarnessSignal,
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

// ---- In-Process Control Plane ----

/// In-process control plane used for tests or embedded multi-node runtimes.
#[derive(Default)]
pub struct InProcessHarnessControlPlane {
    managers: Arc<RwLock<HashMap<NodeId, Weak<super::DistributedHarnessManager>>>>,
}

impl InProcessHarnessControlPlane {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register_manager(&self, manager: &Arc<super::DistributedHarnessManager>) {
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
        signal: crate::agent::harness::HarnessSignal,
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
