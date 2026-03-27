use anyhow::Result;
use crate::gateway::{CrabletGateway, types::GatewayConfig};
use crate::cognitive::router::CognitiveRouter;
use std::sync::Arc;

use crate::events::AgentEvent;

pub async fn handle_gateway(host: &str, port: u16, router: Arc<CognitiveRouter>) -> Result<()> {
    println!("Starting Crablet Gateway on {}:{}...", host, port);
    
    let gateway_config = GatewayConfig {
        host: host.to_string(),
        port,
        auth_mode: std::env::var("CRABLET_AUTH_MODE").unwrap_or_else(|_| "token".to_string()),
    };
    
    let gateway = CrabletGateway::new(gateway_config, router).await?;
    
    // Register a ping method for testing
    gateway.rpc.register("ping", |_| async { 
        Ok(Some(serde_json::json!("pong"))) 
    }).await;

    // Register broadcast for SSE testing
    let event_bus = gateway.event_bus.clone();
    gateway.rpc.register("broadcast", move |params| {
        let event_bus = event_bus.clone();
        async move {
            let msg = params.and_then(|p| p.get("message").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .unwrap_or_else(|| "default message".to_string());
            
            event_bus.publish(AgentEvent::SystemLog(msg));
            Ok(Some(serde_json::json!("broadcast_sent")))
        }
    }).await;

    if let Err(e) = gateway.start().await {
         tracing::error!("Gateway failed: {}", e);
         return Err(anyhow::anyhow!("Gateway error: {}", e));
    }
    
    Ok(())
}
