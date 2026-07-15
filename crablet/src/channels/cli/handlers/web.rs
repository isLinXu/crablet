use crate::channels::cli::handlers::gateway::run_gateway;
use crate::cognitive::router::CognitiveRouter;
use crate::config::Config;
use anyhow::Result;
use std::sync::Arc;

pub async fn handle_serve_web(
    router: Arc<CognitiveRouter>,
    host: &str,
    port: u16,
    config: &Config,
) -> Result<()> {
    let gateway_host = host.to_string();
    let gateway_auth_mode =
        std::env::var("CRABLET_AUTH_MODE").unwrap_or_else(|_| "token".to_string());

    println!(
        "Starting unified Crablet Web Gateway on {}:{} (auth mode: {})...",
        gateway_host, port, gateway_auth_mode
    );

    run_gateway(
        &gateway_host,
        port,
        router,
        Some(config),
        crate::channels::cli::handlers::gateway::GatewayLaunchOverrides {
            auth_mode: Some(gateway_auth_mode),
            ..Default::default()
        },
    )
    .await
}
