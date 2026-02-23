use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
};
use crate::gateway::CrabletGateway;
use futures::{SinkExt, StreamExt};
use crate::gateway::types::RpcRequest;
use tokio::sync::mpsc;
use std::sync::Arc;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(gateway): State<Arc<CrabletGateway>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, gateway))
}

async fn handle_socket(socket: WebSocket, gateway: Arc<CrabletGateway>) {
    let (mut sender, mut receiver) = socket.split();
    
    // Create channel for sending responses to this client
    let (tx, mut rx) = mpsc::unbounded_channel();
    
    // Create session
    let session_id = gateway.session.create_session("anonymous".to_string(), tx);
    tracing::info!("New session: {}", session_id);
    
    // Spawn task to forward messages from channel to websocket
    let mut send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
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
                    let _ = gateway_clone.session.send_to_session(&session_id_clone, res);
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
