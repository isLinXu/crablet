use crate::gateway::server::CrabletGateway;
use crate::gateway::types::{RpcRequest, RpcResponse};
use axum::{extract::State, Json};
use std::sync::Arc;

pub async fn rpc_handler(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(request): Json<RpcRequest>,
) -> Json<RpcResponse> {
    Json(gateway.rpc.dispatch(request).await)
}
