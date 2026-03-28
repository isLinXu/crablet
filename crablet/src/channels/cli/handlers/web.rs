use anyhow::Result;
use std::sync::Arc;
use crate::cognitive::router::CognitiveRouter;
use crate::config::Config;
use crate::channels::cli::handlers::gateway::run_gateway;

pub async fn handle_serve_web(router: Arc<CognitiveRouter>, port: u16, config: &Config) -> Result<()> {
    println!("Starting Crablet Web UI on port {}...", port);

    let auto_start_gateway = std::env::var("CRABLET_SERVE_WEB_START_GATEWAY")
        .map(|v| !matches!(v.as_str(), "0" | "false" | "FALSE" | "no" | "NO"))
        .unwrap_or(true);
    if auto_start_gateway {
        let gateway_router = router.clone();
        let gateway_host = std::env::var("CRABLET_GATEWAY_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let gateway_port = std::env::var("CRABLET_GATEWAY_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(18789);
        let gateway_auth_mode = std::env::var("CRABLET_AUTH_MODE").unwrap_or_else(|_| "off".to_string());

        println!(
            "Starting bundled Crablet Gateway on {}:{} (auth mode: {})...",
            gateway_host, gateway_port, gateway_auth_mode
        );

        tokio::spawn(async move {
            if let Err(e) = run_gateway(&gateway_host, gateway_port, gateway_router, Some(gateway_auth_mode)).await {
                tracing::warn!("Bundled gateway stopped: {}", e);
            }
        });
    }

    let auth_config = if let (Some(issuer), Some(id), Some(secret), Some(jwt_secret)) = (
        &config.oidc_issuer,
        &config.oidc_client_id,
        &config.oidc_client_secret,
        &config.jwt_secret
    ) {
        Some((issuer.clone(), id.clone(), secret.clone(), jwt_secret.clone()))
    } else {
        None
    };
    
    crate::channels::web::run(router, port, auth_config).await?;
    Ok(())
}
