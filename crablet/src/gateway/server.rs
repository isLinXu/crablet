use axum::{
    routing::{get, post, delete},
    Router,
    extract::{State, Request},
    response::{sse::{Event, KeepAlive, Sse}, Response},
    middleware::{self, Next},
    http::{StatusCode, header, HeaderValue, Method},
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
    chat_handler, chat_stream, image_handler, list_skills, upload_knowledge,
    list_documents, get_document_chunks, search_knowledge, list_api_keys,
    create_api_key, revoke_api_key, list_audit_logs, get_routing_settings, update_routing_settings, get_routing_report,
    get_swarm_state, list_agents, cancel_task,
    get_dashboard_stats, get_swarm_stats, get_swarm_tasks, toggle_skill,
    search_registry_skills, install_skill, batch_test_skills, get_mcp_overview, list_swarm_reviews, decide_swarm_review,
    get_skills_sh_top, get_system_config, update_system_config,
    semantic_search_skills, run_skill, get_skill_logs, get_all_skill_logs
};
use crate::gateway::session_handlers::{
    list_sessions as list_chat_sessions,
    delete_session as delete_chat_session,
    get_session_history as get_chat_session_history,
    compress_session,
};
use crate::gateway::chat_enhancement_handlers::{
    get_token_usage,
    star_message,
    unstar_message,
    list_stars,
    is_starred,
    get_star_count,
    dual_search,
    topk_recommend,
};
use crate::gateway::observability_handlers as obs;
use crate::gateway::workflow_handlers;
use crate::gateway::ratelimit::{create_limiter, GlobalRateLimiter};
use crate::storage::{HybridStorage, StorageConfig};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt as _;
use futures::stream::Stream;
use tower_http::services::ServeDir;
use tower_http::cors::{CorsLayer, Any};
use tower_http::set_header::SetResponseHeaderLayer;
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
use crate::workflow::engine::WorkflowEngine;
use crate::workflow::registry::WorkflowRegistry;
use crate::workflow::executor::NodeExecutorRegistry;
use crate::heartbeat::HeartbeatEngine;

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
    pub storage: Arc<HybridStorage>,
    pub workflow_engine: Arc<WorkflowEngine>,
    pub workflow_registry: Arc<WorkflowRegistry>,
    pub heartbeat: Arc<HeartbeatEngine>,
    #[cfg(feature = "knowledge")]
    pub ingestion: Option<Arc<IngestionService>>,
}

impl CrabletGateway {
    pub async fn new(config: GatewayConfig, router: Arc<CognitiveRouter>, cancel_token: tokio_util::sync::CancellationToken) -> anyhow::Result<Self> {
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

        let auth = AuthManager::new(auth_mode, pool.clone());

        let sqlite_pool = match &pool {
            Some(existing) => existing.clone(),
            None => sqlx::sqlite::SqlitePool::connect("sqlite::memory:").await?,
        };
        let redis_url = std::env::var("REDIS_URL")
            .ok()
            .or_else(|| std::env::var("CRABLET_REDIS_URL").ok());
        let storage = Arc::new(
            HybridStorage::new(StorageConfig {
                redis_url,
                sqlite_pool,
            }).await?
        );

        // Load existing keys
        let auth_clone = auth.clone();
        tokio::spawn(async move {
            if let Err(e) = auth_clone.load_keys_from_db().await {
                tracing::warn!("Failed to load API keys: {}", e);
            }
        });

        // Generate admin token if using Token auth and no tokens exist
        if matches!(auth.mode(), AuthMode::Token) && !auth.has_tokens() {
            let token = auth.generate_token("admin", "admin");
            let allow_plaintext_bootstrap_token = std::env::var("CRABLET_PRINT_BOOTSTRAP_TOKEN")
                .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
                .unwrap_or(false);
            if allow_plaintext_bootstrap_token {
                tracing::warn!("Generated bootstrap admin token: {}", token);
            } else {
                let suffix_len = token.len().min(6);
                let suffix = &token[token.len() - suffix_len..];
                tracing::warn!(
                    "Generated bootstrap admin token ending with ...{}. Set CRABLET_PRINT_BOOTSTRAP_TOKEN=true to print the full token in trusted local development.",
                    suffix
                );
            }
            tracing::info!("Requests require 'Authorization: Bearer <token>' header.");
        }

        #[cfg(feature = "knowledge")]
        let ingestion = router.sys2.vector_store.as_ref().map(|vs| {
            Arc::new(IngestionService::new(vs.clone()))
        });

        // Initialize workflow system
        let executor_registry = Arc::new(NodeExecutorRegistry::new());
        let workflow_engine = Arc::new(WorkflowEngine::new(executor_registry));
        let workflow_registry = Arc::new(WorkflowRegistry::new());

        // Initialize Heartbeat Engine for Draft Mode and proactive tasks
        let mut heartbeat = HeartbeatEngine::new(
            router.memory_mgr.clone(),
            router.sys3.coordinator.llm.clone(),
            router.shared_skills.clone(),
        );
        
        if let Some(orch) = &router.sys3.orchestrator {
            heartbeat = heartbeat.with_swarm(orch.clone());
        }
        
        let heartbeat_arc = Arc::new(heartbeat);
        let heartbeat_clone = heartbeat_arc.clone();
        let cancel_token_clone = cancel_token.clone();
        tokio::spawn(async move {
            heartbeat_clone.start(cancel_token_clone).await;
        });

        Ok(Self {
            router,
            rpc: RpcDispatcher::new(),
            auth,
            session: SessionManager::new(),
            canvas: CanvasManager::new(event_bus.clone()),
            event_bus,
            config,
            rate_limiter: create_limiter(),
            storage,
            workflow_engine,
            workflow_registry,
            heartbeat: heartbeat_arc,
            #[cfg(feature = "knowledge")]
            ingestion,
        })
    }

    pub async fn start(self, cancel_token: tokio_util::sync::CancellationToken) -> Result<(), axum::BoxError> {
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
                .route("/sessions", get(list_chat_sessions))
                .route("/sessions/:id", delete(delete_chat_session))
                .route("/sessions/:id/history", get(get_chat_session_history))
                .route("/chat/sessions/:id/token-usage", get(get_token_usage))
                .route("/chat/sessions/:id/compress", post(compress_session))
                .route("/chat/sessions/:id/stars", get(list_stars).post(star_message))
                .route("/chat/sessions/:id/stars/:message_id", get(is_starred).delete(unstar_message))
                .route("/chat/sessions/:id/star-count", get(get_star_count))
                .route("/rag/topk-recommend", get(topk_recommend))
                .route("/rag/search", get(dual_search))
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
                .route("/skills/:name/run", post(run_skill))
                .route("/skills/:name/logs", get(get_skill_logs))
                .route("/skills/logs", get(get_all_skill_logs))
                .route("/skills/registry/search", get(search_registry_skills))
                .route("/skills/semantic-search", post(semantic_search_skills))
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
                .route("/settings/system/config", get(get_system_config).post(update_system_config))
                .route("/logs", get(list_audit_logs))
                // Workflow routes
                .route("/workflows", get(workflow_handlers::list_workflows).post(workflow_handlers::create_workflow))
                .route("/workflows/:id", get(workflow_handlers::get_workflow).put(workflow_handlers::update_workflow).delete(workflow_handlers::delete_workflow))
                .route("/workflows/:id/execute", post(workflow_handlers::execute_workflow))
                .route("/workflows/:id/run", post(workflow_handlers::run_workflow_stream))
                .route("/workflows/:id/executions", get(workflow_handlers::list_executions))
                .route("/workflows/validate", post(workflow_handlers::validate_workflow))
                .route("/workflows/node-types", get(workflow_handlers::get_node_types))
                .route("/executions/:id", get(workflow_handlers::get_execution).delete(workflow_handlers::cancel_execution))
                // Observability routes
                .route("/observability/sessions", get(obs::list_sessions))
                .route("/observability/sessions/:id", get(obs::get_session).delete(obs::delete_session))
                .route("/observability/sessions/:id/events", get(obs::stream_session_events))
                .route("/observability/sessions/:id/metrics", get(obs::get_metrics))
                .route("/observability/breakpoints", get(obs::list_breakpoints).post(obs::create_breakpoint))
                .route("/observability/breakpoints/:id", delete(obs::delete_breakpoint))
                .route("/observability/paused", get(obs::get_paused_sessions))
                .route("/observability/intervene/:id", post(obs::intervene))
                .route("/observability/events", get(obs::stream_events))
            )
            .route("/api/sessions", get(list_chat_sessions))
            .route("/api/sessions/:id", delete(delete_chat_session))
            .route("/api/sessions/:id/history", get(get_chat_session_history))
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

        let allow_any_origin = std::env::var("CRABLET_ALLOW_ANY_ORIGIN")
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false);

        let cors = if allow_any_origin {
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
        } else {
            CorsLayer::new()
                .allow_origin([
                    HeaderValue::from_static("http://localhost:3000"),
                    HeaderValue::from_static("http://127.0.0.1:3000"),
                    HeaderValue::from_static("http://localhost:3333"),
                    HeaderValue::from_static("http://127.0.0.1:3333"),
                    HeaderValue::from_static("http://localhost:5173"),
                    HeaderValue::from_static("http://127.0.0.1:5173"),
                    HeaderValue::from_static("http://localhost:8080"),
                    HeaderValue::from_static("http://127.0.0.1:8080"),
                ])
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
        };

        let app = public_routes
            .merge(protected_routes)
            .fallback_service(ServeDir::new("static"))
            // Security response headers
            .layer(SetResponseHeaderLayer::overriding(
                header::X_CONTENT_TYPE_OPTIONS,
                header::HeaderValue::from_static("nosniff"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::X_FRAME_OPTIONS,
                header::HeaderValue::from_static("DENY"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::HeaderName::from_static("x-xss-protection"),
                header::HeaderValue::from_static("1; mode=block"),
            ))
            .layer(SetResponseHeaderLayer::overriding(
                header::HeaderName::from_static("referrer-policy"),
                header::HeaderValue::from_static("strict-origin-when-cross-origin"),
            ))
            .layer(SetResponseHeaderLayer::if_not_present(
                header::STRICT_TRANSPORT_SECURITY,
                header::HeaderValue::from_static("max-age=31536000; includeSubDomains"),
            ))
            .layer(cors) // Cors first (outermost)
            .layer(TraceLayer::new_for_http())
            .with_state(gateway);

        tracing::info!("Gateway listening on 0.0.0.0:{}", port);
        let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        
        // Use into_make_service_with_connect_info to provide IP for rate limiter
        axum::serve(
            listener, 
            app.into_make_service_with_connect_info::<SocketAddr>()
        )
        .with_graceful_shutdown(async move {
            cancel_token.cancelled().await;
            tracing::info!("Gateway server shutting down gracefully...");
        })
        .await?;

        Ok(())
    }
}
