use anyhow::Result;
use std::sync::Arc;
use crate::cognitive::router::CognitiveRouter;
use crate::config::Config;

pub async fn handle_serve_web(router: Arc<CognitiveRouter>, port: u16, config: &Config) -> Result<()> {
    println!("Starting Crablet Web UI on port {}...", port);
    
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
