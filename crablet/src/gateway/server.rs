use axum::{
    routing::{get, post},
    Router,
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    BoxError,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use crate::gateway::websocket::ws_handler;
use crate::gateway::rpc::RpcDispatcher;
use crate::gateway::auth::AuthManager;
use crate::gateway::session::SessionManager;
use crate::gateway::events::EventBus;
use crate::gateway::types::GatewayConfig;
use crate::gateway::canvas_manager::CanvasManager;
use crate::gateway::web_handlers::{index, chat_handler, swarm_handler};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as _;
use futures::stream::Stream;
use tower_http::services::ServeDir;

async fn sse_handler(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Sse<impl Stream<Item = Result<Event, BoxError>>> {
    let rx = gateway.event_bus.subscribe();
    let stream = BroadcastStream::new(rx).map(|msg| {
        match msg {
            Ok(event) => {
                let data = serde_json::to_string(&event).unwrap_or_default();
                Ok(Event::default().data(data))
            }
            Err(_) => Ok(Event::default().comment("missed message")),
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(Clone)]
pub struct CrabletGateway {
    pub rpc: RpcDispatcher,
    pub auth: AuthManager,
    pub session: SessionManager,
    pub event_bus: EventBus,
    pub canvas: CanvasManager,
    pub config: GatewayConfig,
}

impl CrabletGateway {
    pub fn new(config: GatewayConfig) -> Self {
        let event_bus = EventBus::new(100);
        Self {
            rpc: RpcDispatcher::new(),
            auth: AuthManager::new(crate::gateway::auth::AuthMode::Off),
            session: SessionManager::new(),
            canvas: CanvasManager::new(Arc::new(event_bus.clone())),
            event_bus,
            config,
        }
    }

    pub async fn start(self) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let gateway = Arc::new(self);
        
        let app = Router::new()
            .route("/", get(index))
            .route("/api/chat", post(chat_handler))
            .route("/api/swarm/message", post(swarm_handler))
            .route("/ws", get(ws_handler))
            .route("/events", get(sse_handler))
            .nest_service("/assets", ServeDir::new("assets"))
            .with_state(gateway);
            
        let listener = TcpListener::bind(&addr).await?;
        tracing::info!("Gateway listening on http://{}", addr);
        axum::serve(listener, app).await?;
        Ok(())
    }
}
