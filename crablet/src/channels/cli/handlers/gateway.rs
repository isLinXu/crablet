use crate::agent::distributed_harness::{
    create_backend, BackendType, DistributedConfig, DistributedHarnessManager,
    HttpHarnessControlPlane,
};
use crate::cognitive::router::CognitiveRouter;
use crate::config::{Config, DistributedHarnessSettings};
use crate::events::AgentEvent;
use crate::gateway::auth::AuthMode;
use crate::gateway::{types::GatewayConfig, CrabletGateway};
use anyhow::{anyhow, Result};
use std::sync::Arc;
use tracing::{info, warn};

#[derive(Debug, Clone, Default)]
pub struct DistributedHarnessOverrides {
    pub enabled: bool,
    pub node_id: Option<String>,
    pub node_address: Option<String>,
    pub node_port: Option<u16>,
    pub backend: Option<String>,
    pub backend_uri: Option<String>,
    pub lock_ttl_secs: Option<u64>,
    pub heartbeat_interval_secs: Option<u64>,
    pub node_timeout_secs: Option<u64>,
    pub rpc_path: Option<String>,
    pub rpc_bearer_token: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct GatewayLaunchOverrides {
    pub auth_mode: Option<String>,
    pub distributed: DistributedHarnessOverrides,
}

#[derive(Debug, Clone)]
struct ResolvedDistributedHarness {
    config: DistributedConfig,
    rpc_path: String,
    rpc_bearer_token: Option<String>,
}

fn normalize_rpc_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        "/rpc".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    }
}

fn apply_distributed_overrides(
    mut settings: DistributedHarnessSettings,
    overrides: &DistributedHarnessOverrides,
) -> DistributedHarnessSettings {
    if overrides.enabled {
        settings.enabled = true;
    }
    if let Some(node_id) = &overrides.node_id {
        settings.node_id = Some(node_id.clone());
    }
    if let Some(node_address) = &overrides.node_address {
        settings.node_address = Some(node_address.clone());
    }
    if let Some(node_port) = overrides.node_port {
        settings.node_port = Some(node_port);
    }
    if let Some(backend) = &overrides.backend {
        settings.backend = Some(backend.clone());
    }
    if let Some(backend_uri) = &overrides.backend_uri {
        settings.backend_uri = Some(backend_uri.clone());
    }
    if let Some(lock_ttl_secs) = overrides.lock_ttl_secs {
        settings.lock_ttl_secs = lock_ttl_secs;
    }
    if let Some(heartbeat_interval_secs) = overrides.heartbeat_interval_secs {
        settings.heartbeat_interval_secs = heartbeat_interval_secs;
    }
    if let Some(node_timeout_secs) = overrides.node_timeout_secs {
        settings.node_timeout_secs = node_timeout_secs;
    }
    if let Some(rpc_path) = &overrides.rpc_path {
        settings.rpc_path = rpc_path.clone();
    }
    if let Some(rpc_bearer_token) = &overrides.rpc_bearer_token {
        settings.rpc_bearer_token = Some(rpc_bearer_token.clone());
    }

    settings
}

fn resolve_distributed_harness(
    config: Option<&Config>,
    host: &str,
    port: u16,
    overrides: &DistributedHarnessOverrides,
) -> Result<Option<ResolvedDistributedHarness>> {
    let base_settings = config
        .map(|cfg| cfg.distributed_harness.clone())
        .unwrap_or_default();
    let settings = apply_distributed_overrides(base_settings, overrides);

    if !settings.is_enabled() {
        return Ok(None);
    }

    let node_id = settings
        .node_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .ok_or_else(|| {
            anyhow!(
                "Distributed harness is enabled but no node_id was provided. Set distributed_harness.node_id, CRABLET_DISTRIBUTED_NODE_ID, or --distributed-node-id."
            )
        })?;

    let backend_type = settings
        .backend
        .as_deref()
        .unwrap_or("redis")
        .parse::<BackendType>()
        .map_err(|err| anyhow!(err.to_string()))?;
    let node_address = settings.node_address.unwrap_or_else(|| host.to_string());
    let node_port = settings.node_port.unwrap_or(port);
    let backend_uri = settings
        .backend_uri
        .unwrap_or_else(|| backend_type.default_backend_uri().to_string());

    Ok(Some(ResolvedDistributedHarness {
        config: DistributedConfig {
            node_id,
            node_address,
            node_port,
            backend_type,
            backend_uri,
            lock_ttl_secs: settings.lock_ttl_secs,
            heartbeat_interval_secs: settings.heartbeat_interval_secs,
            node_timeout_secs: settings.node_timeout_secs,
        },
        rpc_path: normalize_rpc_path(&settings.rpc_path),
        rpc_bearer_token: settings.rpc_bearer_token,
    }))
}

async fn maybe_enable_distributed_harness(
    gateway: &CrabletGateway,
    resolved: Option<ResolvedDistributedHarness>,
) -> Result<Option<Arc<DistributedHarnessManager>>> {
    let Some(resolved) = resolved else {
        return Ok(None);
    };

    let backend = create_backend(&resolved.config).await?;
    let mut manager = DistributedHarnessManager::new(backend, resolved.config.clone());

    if matches!(resolved.config.backend_type, BackendType::InMemory) {
        warn!("Distributed harness enabled with InMemory backend; this only works within a single process");
    } else {
        let mut control_plane = HttpHarnessControlPlane::new().with_rpc_path(resolved.rpc_path);
        if let Some(token) = resolved.rpc_bearer_token {
            control_plane = control_plane.with_bearer_token(token);
        } else if !matches!(gateway.auth.mode(), AuthMode::Off) {
            warn!(
                "Distributed harness RPC is enabled but CRABLET_DISTRIBUTED_RPC_BEARER_TOKEN is not set; remote forwarding may fail against authenticated nodes"
            );
        }
        manager = manager.with_control_plane(Arc::new(control_plane));
    }

    let manager = Arc::new(manager);
    DistributedHarnessManager::register_rpc_handlers(&manager, &gateway.rpc).await?;
    crate::gateway::harness_handlers::register_execution_rpc_handlers(
        &gateway.rpc,
        manager.clone(),
        gateway.router.clone(),
    )
    .await?;
    manager.register_local_node().await?;
    info!(
        "Distributed harness RPC enabled for node {} at {}:{} using {}",
        manager.node_id(),
        resolved.config.node_address,
        resolved.config.node_port,
        resolved.config.backend_type
    );

    Ok(Some(manager))
}

pub async fn run_gateway(
    host: &str,
    port: u16,
    router: Arc<CognitiveRouter>,
    config: Option<&Config>,
    overrides: GatewayLaunchOverrides,
) -> Result<()> {
    let gateway_config = GatewayConfig {
        host: host.to_string(),
        port,
        auth_mode: overrides.auth_mode.unwrap_or_else(|| {
            std::env::var("CRABLET_AUTH_MODE").unwrap_or_else(|_| "token".to_string())
        }),
    };

    let cancel_token = tokio_util::sync::CancellationToken::new();

    let gateway = CrabletGateway::new(gateway_config, router, cancel_token.clone()).await?;

    gateway
        .rpc
        .register("ping", |_| async { Ok(Some(serde_json::json!("pong"))) })
        .await;

    let event_bus = gateway.event_bus.clone();
    gateway
        .rpc
        .register("broadcast", move |params| {
            let event_bus = event_bus.clone();
            async move {
                let msg = params
                    .and_then(|p| {
                        p.get("message")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_else(|| "default message".to_string());

                event_bus.publish(AgentEvent::SystemLog(msg));
                Ok(Some(serde_json::json!("broadcast_sent")))
            }
        })
        .await;

    let distributed = resolve_distributed_harness(config, host, port, &overrides.distributed)?;
    let mut gateway = gateway;
    if let Some(distributed_manager) =
        maybe_enable_distributed_harness(&gateway, distributed).await?
    {
        gateway = gateway.with_distributed_harness(distributed_manager);
    }

    if let Err(e) = gateway.start(cancel_token.clone()).await {
        tracing::error!("Gateway failed: {}", e);
        return Err(anyhow::anyhow!("Gateway error: {}", e));
    }

    // Wait for ctrl-c
    match tokio::signal::ctrl_c().await {
        Ok(()) => {
            tracing::info!("Received Ctrl-C, shutting down gateway...");
            cancel_token.cancel();
            tokio::time::sleep(std::time::Duration::from_millis(500)).await; // Give time to clean up
        }
        Err(err) => {
            tracing::error!("Unable to listen for shutdown signal: {}", err);
        }
    }

    Ok(())
}

pub async fn handle_gateway(
    host: &str,
    port: u16,
    router: Arc<CognitiveRouter>,
    config: &Config,
    overrides: GatewayLaunchOverrides,
) -> Result<()> {
    println!("Starting Crablet Gateway on {}:{}...", host, port);
    run_gateway(host, port, router, Some(config), overrides).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_distributed_harness_uses_config_defaults() {
        let mut config = Config::default();
        config.distributed_harness.enabled = true;
        config.distributed_harness.node_id = Some("node-config".to_string());

        let resolved = resolve_distributed_harness(
            Some(&config),
            "127.0.0.1",
            18790,
            &DistributedHarnessOverrides::default(),
        )
        .unwrap()
        .unwrap();

        assert_eq!(resolved.config.node_id, "node-config");
        assert_eq!(resolved.config.node_address, "127.0.0.1");
        assert_eq!(resolved.config.node_port, 18790);
        assert_eq!(resolved.config.backend_type, BackendType::Redis);
        assert_eq!(resolved.config.backend_uri, "redis://127.0.0.1/");
        assert_eq!(resolved.rpc_path, "/rpc");
    }

    #[test]
    fn test_resolve_distributed_harness_cli_overrides_config() {
        let mut config = Config::default();
        config.distributed_harness.enabled = true;
        config.distributed_harness.node_id = Some("node-config".to_string());
        config.distributed_harness.node_address = Some("10.0.0.1".to_string());
        config.distributed_harness.rpc_path = "rpc".to_string();

        let resolved = resolve_distributed_harness(
            Some(&config),
            "127.0.0.1",
            18790,
            &DistributedHarnessOverrides {
                node_id: Some("node-cli".to_string()),
                node_address: Some("10.0.0.2".to_string()),
                node_port: Some(19090),
                backend: Some("memory".to_string()),
                backend_uri: Some("memory://".to_string()),
                rpc_path: Some("api/v1/rpc".to_string()),
                ..Default::default()
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(resolved.config.node_id, "node-cli");
        assert_eq!(resolved.config.node_address, "10.0.0.2");
        assert_eq!(resolved.config.node_port, 19090);
        assert_eq!(resolved.config.backend_type, BackendType::InMemory);
        assert_eq!(resolved.config.backend_uri, "memory://");
        assert_eq!(resolved.rpc_path, "/api/v1/rpc");
    }

    #[test]
    fn test_resolve_distributed_harness_requires_node_id() {
        let mut config = Config::default();
        config.distributed_harness.enabled = true;

        let err = resolve_distributed_harness(
            Some(&config),
            "127.0.0.1",
            18790,
            &DistributedHarnessOverrides::default(),
        )
        .unwrap_err();

        assert!(err.to_string().contains("no node_id was provided"));
    }
}
