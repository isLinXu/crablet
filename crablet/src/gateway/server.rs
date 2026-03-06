use axum::{
    routing::{get, post, delete},
    Router,
    extract::{State, Request},
    response::{sse::{Event, KeepAlive, Sse}, Response},
    middleware::{self, Next},
    http::{StatusCode, header},
    BoxError,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use crate::gateway::websocket::ws_handler;
use crate::gateway::rpc::RpcDispatcher;
use crate::gateway::auth::{AuthManager, AuthMode};
use crate::gateway::session::SessionManager;
use crate::events::EventBus;
use crate::gateway::types::GatewayConfig;
use crate::gateway::canvas_manager::CanvasManager;
use crate::gateway::web_handlers::{
    chat_handler, chat_stream, image_handler, list_skills, list_sessions, upload_knowledge, 
    list_documents, get_document_chunks, search_knowledge, list_api_keys, 
    create_api_key, revoke_api_key, list_audit_logs, get_routing_settings, update_routing_settings, get_routing_report,
    get_swarm_state, list_agents, cancel_task, delete_session, get_session_history,
    get_dashboard_stats, get_swarm_stats, get_swarm_tasks, toggle_skill,
    search_registry_skills, install_skill, batch_test_skills, get_mcp_overview, list_swarm_reviews, decide_swarm_review,
    get_skills_sh_top
};
use crate::gateway::ratelimit::{create_limiter, GlobalRateLimiter};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as _;
use futures::stream::Stream;
use tower_http::services::ServeDir;
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::TraceLayer;
use std::net::SocketAddr;
#[cfg(feature = "knowledge")]
use crate::knowledge::ingestion::IngestionService;
use axum::extract::ConnectInfo;

async fn rate_limit_middleware(
    State(gateway): State<Arc<CrabletGateway>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let ip = addr.ip();
    match gateway.rate_limiter.check_key(&ip) {
        Ok(_) => Ok(next.run(req).await),
        Err(_) => Err(StatusCode::TOO_MANY_REQUESTS),
    }
}

async fn auth_middleware(
    State(gateway): State<Arc<CrabletGateway>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req.headers().get(header::AUTHORIZATION);
    let token = auth_header.and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));

    if matches!(gateway.auth.mode(), AuthMode::Off) {
        return Ok(next.run(req).await);
    }

    if let Some(token) = token {
        if let Some(_user) = gateway.auth.validate_token_async(token).await {
            return Ok(next.run(req).await);
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

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

use crate::cognitive::router::CognitiveRouter;

#[derive(Clone)]
pub struct CrabletGateway {
    pub router: Arc<CognitiveRouter>,
    pub rpc: RpcDispatcher,
    pub auth: AuthManager,
    pub session: SessionManager,
    pub event_bus: EventBus,
    pub canvas: CanvasManager,
    pub config: GatewayConfig,
    pub rate_limiter: Arc<GlobalRateLimiter>,
    #[cfg(feature = "knowledge")]
    pub ingestion: Option<Arc<IngestionService>>,
}

impl CrabletGateway {
    pub fn new(config: GatewayConfig, router: Arc<CognitiveRouter>) -> Self {
        let event_bus = crate::events::EventBus::new(100);
        
        // Parse auth mode from config
        let auth_mode = match config.auth_mode.to_lowercase().as_str() {
            "off" => AuthMode::Off,
            "local" => AuthMode::Local,
            "token" => AuthMode::Token,
            "apikey" => AuthMode::ApiKey,
            "mtls" => AuthMode::MTLS,
            "jwt" => AuthMode::JWT,
            _ => AuthMode::Token, // Default to Token
        };
        
        // Extract pool from MemoryManager
        let pool = router.memory_mgr.episodic.as_ref().map(|m| m.pool.clone());
        
        // Start Audit Worker if pool is available
        if let Some(p) = &pool {
            crate::audit::start_audit_worker(p.clone(), Arc::new(event_bus.clone()));
        }

        let auth = AuthManager::new(auth_mode, pool);
        
        // Load existing keys
        // We need to spawn this as new() is sync but load is async.
        // Or we can just do it lazily or make new async.
        // For simplicity, let's spawn a task to load keys.
        let auth_clone = auth.clone();
        tokio::spawn(async move {
            if let Err(e) = auth_clone.load_keys_from_db().await {
                tracing::warn!("Failed to load API keys: {}", e);
            }
        });
        
        // Generate admin token if using Token auth and no tokens exist
        if matches!(auth.mode(), AuthMode::Token) && !auth.has_tokens() {
            let token = auth.generate_token("admin", "admin");
            tracing::info!("Generated Admin Token: {}", token);
            tracing::info!("Please save this token. Requests require 'Authorization: Bearer <token>' header.");
        }

        #[cfg(feature = "knowledge")]
        let ingestion = router.sys2.vector_store.as_ref().map(|vs| {
            Arc::new(IngestionService::new(vs.clone()))
        });

        Self {
            router,
            rpc: RpcDispatcher::new(),
            auth,
            session: SessionManager::new(),
            canvas: CanvasManager::new(event_bus.clone()),
            event_bus,
            config,
            rate_limiter: create_limiter(),
            #[cfg(feature = "knowledge")]
            ingestion,
        }
    }

    pub async fn start(self) -> Result<(), axum::BoxError> {
        let gateway = Arc::new(self);
        let port = gateway.config.port;

        // Separate routes for public and protected endpoints
        let public_routes = Router::new()
            .route("/health", get(|| async { "ok" }));
            
        let protected_routes = Router::new()
            .nest("/api/v1", Router::new()
                .route("/chat", post(chat_handler))
                .route("/images", post(image_handler))
                .route("/chat/stream", post(chat_stream))
                .route("/sessions", get(list_sessions))
                .route("/sessions/:id", delete(delete_session))
                .route("/sessions/:id/history", get(get_session_history))
                .route("/dashboard", get(get_dashboard_stats))
                .route("/swarm/stats", get(get_swarm_stats))
                .route("/swarm/tasks", get(get_swarm_tasks))
                .route("/swarm/state", get(get_swarm_state))
                .route("/swarm/agents", get(list_agents))
                .route("/swarm/tasks/:id", delete(cancel_task))
                .route("/swarm/reviews", get(list_swarm_reviews))
                .route("/swarm/reviews/:task_id/decision", post(decide_swarm_review))
                .route("/skills", get(list_skills))
                .route("/skills/:name/toggle", post(toggle_skill))
                .route("/skills/registry/search", get(search_registry_skills))
                .route("/skills/top", get(get_skills_sh_top))
                .route("/skills/install", post(install_skill))
                .route("/skills/test/batch", post(batch_test_skills))
                .route("/mcp/overview", get(get_mcp_overview))
                .route("/knowledge/upload", post(upload_knowledge))
                .route("/knowledge/documents", get(list_documents))
                .route("/knowledge/chunks", get(get_document_chunks))
                .route("/knowledge/search", get(search_knowledge))
                .route("/settings/keys", get(list_api_keys).post(create_api_key))
                .route("/settings/keys/:id", delete(revoke_api_key))
                .route("/settings/routing", get(get_routing_settings).put(update_routing_settings))
                .route("/settings/routing/report", get(get_routing_report))
                .route("/logs", get(list_audit_logs))
            )
            .route("/api/sessions", get(list_sessions))
            .route("/api/sessions/:id", delete(delete_session))
            .route("/api/sessions/:id/history", get(get_session_history))
            .route("/api/dashboard", get(get_dashboard_stats))
            .route("/api/swarm/stats", get(get_swarm_stats))
            .route("/api/swarm/tasks", get(get_swarm_tasks))
            .route("/api/swarm/state", get(get_swarm_state))
            .route("/api/swarm/agents", get(list_agents))
            .route("/api/swarm/reviews", get(list_swarm_reviews))
            .route("/api/swarm/reviews/:task_id/decision", post(decide_swarm_review))
            .route("/api/chat", post(chat_handler)) // Keep legacy for compatibility
            .route("/api/images", post(image_handler))
            // .route("/api/swarm/message", post(swarm_handler))
            .route("/ws", get(ws_handler))
            .route("/events", get(sse_handler))
            .layer(middleware::from_fn_with_state(gateway.clone(), auth_middleware))
            // Rate Limiting (Applied to protected routes + expensive ones)
            .layer(middleware::from_fn_with_state(gateway.clone(), rate_limit_middleware));

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        let app = public_routes
            .merge(protected_routes)
            .fallback_service(ServeDir::new("static"))
            .layer(cors) // Cors first (outermost)
            .layer(TraceLayer::new_for_http())
            .with_state(gateway);

        tracing::info!("Gateway listening on 0.0.0.0:{}", port);
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        
        // Use into_make_service_with_connect_info to provide IP for rate limiter
        axum::serve(
            listener, 
            app.into_make_service_with_connect_info::<SocketAddr>()
        ).await?;

        Ok(())
    }
}
