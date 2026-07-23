use anyhow::Result;
use axum::body::to_bytes;
use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{Request, StatusCode},
};
use crablet::{
    channels::cli::context::AppContext,
    config::Config,
    gateway::{types::GatewayConfig, CrabletGateway},
};
use std::{net::SocketAddr, sync::Arc};
use tempfile::TempDir;
use tower::util::ServiceExt;

fn build_test_config(skills_dir: std::path::PathBuf) -> Config {
    let mut config = Config::default();
    config.database_url = "sqlite::memory:".to_string();
    config.skills_dir = skills_dir;
    config.model_name = "gpt-4o-mini".to_string();
    config.log_level = "debug".to_string();
    config.mcp_servers = std::collections::HashMap::new();
    config.channels = vec![];
    config.semantic_cache_threshold = 0.9;
    config.openai_api_key = Some("sk-test".to_string());
    config.port = 3001;
    config.providers = std::collections::HashMap::new();
    config.ollama_model = "llama3".to_string();
    config.serper_api_key = None;
    config.feishu_app_id = None;
    config.feishu_app_secret = None;
    config.oidc_issuer = None;
    config.oidc_client_id = None;
    config.oidc_client_secret = None;
    config.jwt_secret = Some("test-secret".to_string());
    config.bandit_exploration = 0.55;
    config.deliberate_threshold = 0.58;
    config.enable_adaptive_routing = false;
    config.enable_hierarchical_reasoning = true;
    config.meta_reasoning_threshold = 0.82;
    config.mcts_simulations = 24;
    config.mcts_exploration_weight = 1.2;
    config.graph_rag_entity_mode = "hybrid".to_string();
    config.system2_threshold = 0.3;
    config.system3_threshold = 0.7;
    config
}

async fn build_gateway() -> Result<(TempDir, CrabletGateway)> {
    let temp_dir = TempDir::new()?;
    let skills_dir = temp_dir.path().join("skills");
    tokio::fs::create_dir_all(&skills_dir).await?;

    let config = build_test_config(skills_dir);
    let app = Arc::new(AppContext::new(config.clone()).await?);
    let cancel_token = tokio_util::sync::CancellationToken::new();

    let gateway = CrabletGateway::new(
        GatewayConfig {
            host: "127.0.0.1".to_string(),
            port: config.port,
            auth_mode: "off".to_string(),
        },
        app.router.clone(),
        cancel_token,
    )
    .await?;

    Ok((temp_dir, gateway))
}

#[tokio::test]
async fn gateway_exposes_unified_health_and_api_routes() -> Result<()> {
    let (_temp_dir, gateway) = build_gateway().await?;
    let app = gateway.into_router();

    let health_response = app
        .clone()
        .oneshot(Request::get("/health").body(Body::empty())?)
        .await?;
    assert_eq!(health_response.status(), StatusCode::OK);
    let health_body = to_bytes(health_response.into_body(), usize::MAX).await?;
    let health_json: serde_json::Value = serde_json::from_slice(&health_body)?;
    assert_eq!(health_json["status"], "ok");
    assert_eq!(health_json["fusion_memory_active"], true);
    assert_eq!(health_json["legacy_gateway_api_enabled"], false);

    let request = {
        let mut request = Request::get("/api/v1/chat/stream").body(Body::empty())?;
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 18_790))));
        request
    };
    let stream_response = app.clone().oneshot(request).await?;
    assert_eq!(stream_response.status(), StatusCode::METHOD_NOT_ALLOWED);

    let request = {
        let mut request = Request::get("/api/v1/rpc").body(Body::empty())?;
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 18_790))));
        request
    };
    let rpc_response = app.clone().oneshot(request).await?;
    assert_eq!(rpc_response.status(), StatusCode::METHOD_NOT_ALLOWED);

    let legacy_request = {
        let mut request = Request::get("/api/dashboard").body(Body::empty())?;
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 18_790))));
        request
    };
    let legacy_response = app.oneshot(legacy_request).await?;
    assert_eq!(legacy_response.status(), StatusCode::NOT_FOUND);

    Ok(())
}
