use anyhow::Result;
use std::sync::Arc;
use crate::cognitive::router::CognitiveRouter;

pub async fn handle_serve_web(router: Arc<CognitiveRouter>, port: u16) -> Result<()> {
    println!("Starting Crablet Web UI on port {}...", port);
    crate::channels::web::run(router, port).await?;
    Ok(())
}
