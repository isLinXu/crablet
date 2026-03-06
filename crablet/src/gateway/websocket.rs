use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
};
use crate::gateway::CrabletGateway;
use futures::{SinkExt, StreamExt};
use crate::gateway::types::RpcRequest;
use tokio::sync::mpsc;
use std::sync::Arc;
use crate::events::AgentEvent;
use serde_json::json;

fn event_to_ws_json(event: &AgentEvent) -> serde_json::Value {
    match event {
        AgentEvent::UserInput(content) => json!({ "UserInput": { "content": content } }),
        AgentEvent::ThoughtGenerated(thought) => json!({ "ThoughtGenerated": thought }),
        AgentEvent::ToolExecutionStarted { tool, args } => {
            json!({ "ToolExecutionStarted": { "tool": tool, "args": args } })
        }
        AgentEvent::ToolExecutionFinished { output, .. } => {
            json!({ "ToolExecutionFinished": { "output": output } })
        }
        AgentEvent::SwarmActivity { task_id, from, to, message_type, content, .. } => json!({
            "SwarmActivity": {
                "task_id": task_id,
                "from": from,
                "to": to,
                "message_type": message_type,
                "content": content
            }
        }),
        AgentEvent::GraphRagEntityModeChanged { from_mode, to_mode } => json!({
            "GraphRagEntityModeChanged": {
                "from_mode": from_mode,
                "to_mode": to_mode
            }
        }),
        AgentEvent::ResponseGenerated(content) => json!({ "ResponseGenerated": { "content": content } }),
        AgentEvent::Error(message) => json!({ "Error": { "message": message } }),
        AgentEvent::CognitiveLayerChanged { layer } => json!({ "CognitiveLayer": { "layer": layer } }),
        _ => json!({ "type": "noop" }),
    }
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(gateway): State<Arc<CrabletGateway>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, gateway))
}

async fn handle_socket(socket: WebSocket, gateway: Arc<CrabletGateway>) {
    let (mut sender, mut receiver) = socket.split();
    
    // Create channel for sending responses to this client
    // Use bounded channel to prevent DoS (backpressure)
    let (tx, mut rx) = mpsc::channel(1024);
    let mut event_rx = gateway.event_bus.subscribe();
    
    // Create session
    let session_id = gateway.session.create_session("anonymous".to_string(), tx);
    tracing::info!("New session: {}", session_id);
    
    // Spawn task to forward messages from channel to websocket
    let mut send_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                maybe_rpc = rx.recv() => {
                    if let Some(msg) = maybe_rpc {
                        if let Ok(json) = serde_json::to_string(&msg) {
                            if sender.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
                evt = event_rx.recv() => {
                    match evt {
                        Ok(envelope) => {
                            let payload = event_to_ws_json(&envelope.payload);
                            if payload.get("type").and_then(|v| v.as_str()) == Some("noop") {
                                continue;
                            }
                            if sender.send(Message::Text(payload.to_string())).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    });
    let _send_task_handle = &mut send_task; // Keep handle if needed, but select! consumes future.

    // Handle incoming messages
    let gateway_clone = gateway.clone();
    let session_id_clone = session_id.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                // Parse RPC request
                if let Ok(req) = serde_json::from_str::<RpcRequest>(&text) {
                    let res = gateway_clone.rpc.dispatch(req).await;
                    // Now async
                    let _ = gateway_clone.session.send_to_session(&session_id_clone, res).await;
                }
            }
        }
        // Cleanup session
        gateway_clone.session.remove_session(&session_id_clone);
        tracing::info!("Session closed: {}", session_id_clone);
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };
}
