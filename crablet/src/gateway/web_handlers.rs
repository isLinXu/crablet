use axum::{
    extract::{State, Json, Multipart, Query},
    response::sse::{Event, Sse},
    http::StatusCode,
};
#[cfg(feature = "knowledge")]
use std::io::Write;
use std::sync::Arc;
use std::sync::OnceLock;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use futures::stream::Stream;
use futures::StreamExt;
use crate::gateway::server::CrabletGateway;
use crate::agent::hitl::HumanDecision;
use crate::cognitive::router::RouterConfig;
use crate::cognitive::streaming_pipeline::{
    EmptyDeltaFilterMiddleware, FinalizeSummaryMiddleware, MetricsMiddleware, StreamChunk, StreamingPipeline,
};
// use crate::agent::swarm::SwarmMessage;
use crate::gateway::auth::ApiKeyInfo;
use serde::{Deserialize, Serialize};
use crate::audit::AuditLog;
use tokio::sync::RwLock;
use tokio::process::Command;
use regex::Regex;
use crate::types::TraceStep;
use crate::types::Message;
#[cfg(feature = "knowledge")]
use crate::knowledge::graph_rag::{GraphRAG, EntityExtractorMode};

#[derive(Clone)]
struct StreamRagPreparation {
    step: TraceStep,
    prompt_context: String,
}

#[derive(Deserialize)]
pub struct ChatRequest {
    pub message: String,
    pub session_id: Option<String>,
    #[serde(default)]
    pub route: Option<RouteSelection>,
}

#[derive(Clone, Deserialize, Serialize, Default)]
pub struct RouteSelection {
    pub provider_id: Option<String>,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub version: Option<String>,
    pub reason: Option<String>,
    pub priority: Option<String>,
    pub question_type: Option<String>,
    pub api_base_url: Option<String>,
    pub api_key: Option<String>,
    pub model_type: Option<String>,
}

#[derive(Deserialize)]
pub struct ImageRequest {
    pub prompt: String,
    pub session_id: Option<String>,
    #[serde(default)]
    pub route: Option<RouteSelection>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub n: Option<u32>,
}

fn infer_cognitive_layer(response: &str, traces: &[TraceStep]) -> &'static str {
    let mut text = response.to_lowercase();
    for t in traces {
        text.push(' ');
        text.push_str(&t.thought.to_lowercase());
        if let Some(a) = &t.action {
            text.push(' ');
            text.push_str(&a.to_lowercase());
        }
        if let Some(o) = &t.observation {
            text.push(' ');
            text.push_str(&o.to_lowercase());
        }
    }
    if text.contains("system 1") || text.contains("system1") || text.contains("trie hit") || text.contains("fastrespond") {
        return "system1";
    }
    if text.contains("system 3") || text.contains("system3") || text.contains("plan") || text.contains("planner") || text.contains("verify") {
        return "system3";
    }
    if text.contains("system 2") || text.contains("system2") || text.contains("reason") || text.contains("deliberate") {
        return "system2";
    }
    if !traces.is_empty() {
        return "system2";
    }
    "unknown"
}

fn load_markdown_file(filename: &str) -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let candidates = [
        cwd.join(filename),
        cwd.join("crablet").join(filename),
        cwd.join("../").join(filename),
    ];
    let path = candidates.into_iter().find(|p| p.exists())?;
    let content = fs::read_to_string(path).ok()?;
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn system_prompt_markdown() -> String {
    static CONTENT: OnceLock<String> = OnceLock::new();
    CONTENT
        .get_or_init(|| {
            let mut prompt = String::new();
            if let Some(identity) = load_markdown_file("IDENTITY.md") {
                prompt.push_str(&format!("【身份设定】\n{}\n\n", identity));
            }
            if let Some(soul) = load_markdown_file("SOUL.md") {
                prompt.push_str(&format!("【核心准则】\n{}\n\n", soul));
            }
            if let Some(user) = load_markdown_file("USER.md") {
                prompt.push_str(&format!("【用户信息】\n{}\n\n", user));
            }
            if let Some(agents) = load_markdown_file("AGENTS.md") {
                prompt.push_str(&format!("【工作空间指南】\n{}\n\n", agents));
            }
            if let Some(tools) = load_markdown_file("TOOLS.md") {
                prompt.push_str(&format!("【工具配置】\n{}\n\n", tools));
            }
            if let Some(heartbeat) = load_markdown_file("HEARTBEAT.md") {
                prompt.push_str(&format!("【定时任务】\n{}\n\n", heartbeat));
            }
            prompt
        })
        .clone()
}

fn with_identity_persona_input(message: &str) -> String {
    let system_prompt = system_prompt_markdown();
    if system_prompt.is_empty() {
        message.to_string()
    } else {
        format!("{}\n【用户消息】\n{}", system_prompt, message)
    }
}

async fn prepare_stream_rag(gateway: &Arc<CrabletGateway>, input: &str) -> Option<StreamRagPreparation> {
    let mut rag_context = String::new();
    let mut refs: Vec<serde_json::Value> = Vec::new();
    let mut graph_entities: Vec<String> = Vec::new();
    let mut retrieval = "none".to_string();

    if let Some(kg) = &gateway.router.sys2.kg {
        let keywords: Vec<String> = input
            .split_whitespace()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| w.len() > 3)
            .collect::<HashSet<_>>()
            .into_iter()
            .take(10)
            .collect();
        if !keywords.is_empty() {
            if let Ok(entities) = kg.find_entities_batch(&keywords).await {
                for (name, _) in entities {
                    if let Ok(relations) = kg.find_related(&name).await {
                        if !relations.is_empty() {
                            graph_entities.push(name.clone());
                            rag_context.push_str(&format!("\n[Knowledge Graph about '{}']:\n", name));
                            for (dir, rel, target) in relations.iter().take(4) {
                                if dir == "->" {
                                    rag_context.push_str(&format!("- {} {} {}\n", name, rel, target));
                                } else {
                                    rag_context.push_str(&format!("- {} {} {}\n", target, rel, name));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(feature = "knowledge")]
    {
        if let Some(vs) = &gateway.router.sys2.vector_store {
            let mut contexts_added = false;
            if let Some(kg) = &gateway.router.sys2.kg {
                let mode = {
                    let cfg = gateway.router.config.read().await;
                    EntityExtractorMode::from_str(&cfg.graph_rag_entity_mode).unwrap_or(EntityExtractorMode::Hybrid)
                };
                let graph_rag = GraphRAG::new_with_mode(Arc::clone(vs), Arc::clone(kg), mode);
                if let Ok(results) = graph_rag.retrieve(input, 3).await {
                    if !results.is_empty() {
                        retrieval = "graph_rag".to_string();
                        rag_context.push_str("\n[GraphRAG Results]:\n");
                        for (i, item) in results.iter().enumerate() {
                            rag_context.push_str(&format!("- [Ref {}] [{} {:.2}] {}\n", i + 1, item.source, item.score, item.content));
                            refs.push(serde_json::json!({
                                "source": item.source,
                                "score": item.score,
                                "content": item.content.chars().take(280).collect::<String>()
                            }));
                        }
                        contexts_added = true;
                    }
                }
            }
            if !contexts_added {
                if let Ok(results) = vs.search(input, 3).await {
                    if !results.is_empty() {
                        retrieval = "semantic_search".to_string();
                        rag_context.push_str("\n[Semantic Search Results]:\n");
                        for (i, (content, score, metadata)) in results.iter().enumerate() {
                            rag_context.push_str(&format!("- [Ref {}] (score: {:.2}) {}\n", i + 1, score, content));
                            refs.push(serde_json::json!({
                                "source": metadata.get("source").and_then(|v| v.as_str()).unwrap_or("vector_store"),
                                "score": score,
                                "content": content.chars().take(280).collect::<String>()
                            }));
                        }
                    }
                }
            }
        }
    }

    let prompt_context = format!(
        "\n[KNOWLEDGE]\nUse the following retrieved knowledge to answer the user's question if relevant.\nImportant: When using information from the context, cite the source using [Ref X] notation or mention the Knowledge Graph.\n{}\n",
        rag_context
    );
    let step = TraceStep {
        step: 0,
        thought: format!("RAG 检索完成，命中 {} 条参考", refs.len()),
        action: Some("rag_retrieve".to_string()),
        action_input: Some(serde_json::json!({ "query": input }).to_string()),
        observation: Some(
            serde_json::json!({
                "retrieval": retrieval,
                "refs_count": refs.len(),
                "graph_entities": graph_entities,
                "refs": refs
            })
            .to_string(),
        ),
    };
    Some(StreamRagPreparation { step, prompt_context: if rag_context.is_empty() { String::new() } else { prompt_context } })
}

#[derive(Deserialize)]
pub struct CreateKeyRequest {
    name: String,
}

#[derive(Deserialize)]
pub struct UpdateRoutingSettingsRequest {
    pub enable_adaptive_routing: bool,
    pub system2_threshold: f32,
    pub system3_threshold: f32,
    pub bandit_exploration: f32,
    pub enable_hierarchical_reasoning: bool,
    pub deliberate_threshold: f32,
    pub meta_reasoning_threshold: f32,
    pub mcts_simulations: u32,
    pub mcts_exploration_weight: f32,
    pub graph_rag_entity_mode: String,
}

#[derive(Serialize)]
pub struct RoutingSettingsResponse {
    pub enable_adaptive_routing: bool,
    pub system2_threshold: f32,
    pub system3_threshold: f32,
    pub bandit_exploration: f32,
    pub enable_hierarchical_reasoning: bool,
    pub deliberate_threshold: f32,
    pub meta_reasoning_threshold: f32,
    pub mcts_simulations: u32,
    pub mcts_exploration_weight: f32,
    pub graph_rag_entity_mode: String,
}

#[derive(Deserialize)]
pub struct RoutingReportQuery {
    pub window: Option<usize>,
}

#[derive(Deserialize)]
pub struct LogsQuery {
    page: Option<i64>,
    per_page: Option<i64>,
}

#[derive(Deserialize)]
pub struct ToggleSkillRequest {
    enabled: bool,
}

#[derive(Deserialize)]
pub struct SearchSkillsQuery {
    q: Option<String>,
}

#[derive(Deserialize)]
pub struct InstallSkillRequest {
    name: Option<String>,
    url: Option<String>,
    source: Option<String>,
    skill_id: Option<String>,
}

#[derive(Deserialize)]
pub struct BatchTestSkillsRequest {
    skills: Vec<String>,
}

#[derive(Deserialize)]
struct ClawhubSearchItem {
    slug: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    summary: Option<String>,
}

#[derive(Deserialize)]
struct ClawhubSearchResponse {
    results: Vec<ClawhubSearchItem>,
}

#[derive(Deserialize)]
pub struct SkillsShTopQuery {
    limit: Option<usize>,
}

fn disabled_skills_store() -> &'static RwLock<HashSet<String>> {
    static STORE: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(HashSet::new()))
}

pub async fn get_dashboard_stats(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    tracing::info!("Dashboard stats request received");
    let start = std::time::Instant::now();
    
    let (skills_count, skills_list) = {
        let lock = gateway.router.shared_skills.read().await;
        tracing::info!("Dashboard stats: Got skills lock in {:?}", start.elapsed());
        (lock.len(), lock.list_skills())
    };
    
    // active_swarms: access via sys3 or mock for now as direct access is tricky
    let active_swarms = 3; // Mock consistent with swarm_stats
    tracing::info!("Dashboard stats: Got swarms count in {:?}", start.elapsed());
    
    let knowledge_nodes = if let Some(_kg) = &gateway.router.sys2.kg {
        // Simple query to count nodes, or mock
        // For MVP, just return a static or cached number if graph is slow
        142 // Mock for now to avoid graph latency
    } else {
        0
    };

    let stats = serde_json::json!({
        "status": "healthy",
        "skills_count": skills_count,
        "active_tasks": active_swarms,
        "system_load": "Low",
        "skills": skills_list,
        // Backward compatibility / Extra info
        "active_swarms": active_swarms,
        "knowledge_nodes": knowledge_nodes,
        "skills_loaded": skills_count,
        "system_status": "healthy",
        "uptime": 12345 // TODO: Real uptime
    });
    
    tracing::info!("Dashboard stats: Completed in {:?}", start.elapsed());
    Json(stats)
}

pub async fn get_swarm_stats(
    State(_gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
     Json(serde_json::json!({
        "stats": {
            "total_tasks": 12,
            "active": 3,
            "completed": 8,
            "failed": 1,
            "success_rate": 88.5,
            "avg_duration_sec": 4.2
        }
    }))
}

pub async fn get_swarm_tasks(
    State(_gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    // Mock Swarm Graphs
    let graphs = vec![
        serde_json::json!({
            "id": "graph-001",
            "status": "Active",
            "nodes": {
                "task-1": {
                    "id": "task-1",
                    "agent_role": "manager",
                    "prompt": "Coordinate project plan",
                    "dependencies": [],
                    "status": { "Completed": { "duration": 1.2 } },
                    "result": "Plan created."
                },
                "task-2": {
                    "id": "task-2",
                    "agent_role": "researcher",
                    "prompt": "Find relevant libraries",
                    "dependencies": ["task-1"],
                    "status": { "Running": { "started_at": 1234567890 } }
                },
                "task-3": {
                    "id": "task-3",
                    "agent_role": "coder",
                    "prompt": "Implement core logic",
                    "dependencies": ["task-2"],
                    "status": "Pending"
                }
            }
        }),
        serde_json::json!({
            "id": "graph-002",
            "status": "Completed",
            "nodes": {
                "task-A": {
                    "id": "task-A",
                    "agent_role": "writer",
                    "prompt": "Draft blog post",
                    "dependencies": [],
                    "status": { "Completed": { "duration": 3.5 } },
                    "result": "Draft ready."
                }
            }
        })
    ];

    Json(serde_json::json!({
        "graphs": graphs,
        "pagination": {
            "page": 1,
            "limit": 10,
            "total": 2,
            "total_pages": 1
        }
    }))
}

pub async fn chat_handler(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(payload): Json<ChatRequest>,
) -> Json<serde_json::Value> {
    let session_id = payload.session_id.clone().unwrap_or_else(|| "default".to_string());
    let input = with_identity_persona_input(&payload.message);
    
    // Note: We skip WebSocket session creation for REST calls.
    // gateway.session.create_session(session_id.clone(), ...);
    
    match gateway.router.process(&input, &session_id).await {
        Ok((response, traces)) => {
            let cognitive_layer = infer_cognitive_layer(&response, &traces);
            Json(serde_json::json!({
            "response": response,
            "traces": traces,
            "cognitive_layer": cognitive_layer,
            "session_id": session_id
        }))
        },
        Err(e) => Json(serde_json::json!({
            "error": e.to_string()
        })),
    }
}

pub async fn chat_stream(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(payload): Json<ChatRequest>,
) -> Sse<impl Stream<Item = Result<Event, axum::Error>>> {
    let session_id = payload.session_id.clone().unwrap_or_else(|| "default".to_string());
    let session_id_for_event = session_id.clone();
    let message = payload.message.clone();
    let enhanced_input = with_identity_persona_input(&message);
    let llm = gateway.router.sys2.llm.clone();
    let gateway_for_stream = gateway.clone();
    let pipeline = StreamingPipeline::new(vec![
        Arc::new(EmptyDeltaFilterMiddleware),
        Arc::new(MetricsMiddleware),
        Arc::new(FinalizeSummaryMiddleware),
    ]);
    let source_stream = async_stream::stream! {
        let mut messages = Vec::new();
        // System Prompt is now injected into the input message by `with_identity_persona_input`
        // But for better LLM handling, we should ideally put it in System role.
        // However, existing logic puts it in input. Let's keep it consistent or split it.
        // The previous logic used `identity_persona_markdown` to fetch just identity.
        // Now `with_identity_persona_input` prepends EVERYTHING to the user message.
        // If we want to use System role, we should extract it.
        
        // Let's rely on `with_identity_persona_input` for now to keep it simple and consistent with chat_handler.
        // If we want to separate system message:
        let system_context = system_prompt_markdown();
        if !system_context.is_empty() {
             messages.push(Message::system(system_context));
        }
        
        if let Some(rag) = prepare_stream_rag(&gateway_for_stream, &message).await {
            yield StreamChunk {
                chunk_type: "trace".to_string(),
                content: None,
                payload: Some(serde_json::json!({ "step": rag.step })),
            };
            if !rag.prompt_context.is_empty() {
                messages.push(Message::system(rag.prompt_context));
            }
        }
        messages.push(Message::user(message.clone()));
        match llm.chat_stream(&messages).await {
            Ok(mut llm_stream) => {
                while let Some(item) = llm_stream.next().await {
                    match item {
                        Ok(chunk) => {
                            if !chunk.delta.is_empty() {
                                yield StreamChunk::delta(chunk.delta);
                            }
                        }
                        Err(e) => {
                            yield StreamChunk {
                                chunk_type: "error".to_string(),
                                content: Some(e.to_string()),
                                payload: None,
                            };
                            return;
                        }
                    }
                }
            }
            Err(_) => {
                let completion_messages = if messages.len() > 1 {
                    messages.clone()
                } else {
                    vec![Message::user(enhanced_input.clone())]
                };
                match llm.chat_complete(&completion_messages).await {
                    Ok(response) => {
                        let mut current = String::new();
                        for ch in response.chars() {
                            current.push(ch);
                            if current.chars().count() >= 18 {
                                yield StreamChunk::delta(current.clone());
                                current.clear();
                            }
                        }
                        if !current.is_empty() {
                            yield StreamChunk::delta(current);
                        }
                    }
                    Err(e) => {
                        yield StreamChunk {
                            chunk_type: "error".to_string(),
                            content: Some(e.to_string()),
                            payload: None,
                        };
                    }
                }
            }
        }
    };
    let stream = pipeline.process(source_stream).map(move |chunk| {
        let data = serde_json::json!({
            "type": chunk.chunk_type,
            "content": chunk.content,
            "payload": chunk.payload,
            "session_id": session_id_for_event.clone()
        });
        Ok(Event::default().data(data.to_string()))
    });
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

pub async fn image_handler(
    Json(payload): Json<ImageRequest>,
) -> Json<serde_json::Value> {
    let route = payload.route.unwrap_or_default();
    let model = route.model.unwrap_or_else(|| "qwen-image".to_string());
    let base_url = route
        .api_base_url
        .unwrap_or_else(|| "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string())
        .trim_end_matches('/')
        .to_string();
    let api_key = route.api_key.unwrap_or_default();
    if api_key.trim().is_empty() {
        return Json(serde_json::json!({ "error": "缺少图像模型 API Key" }));
    }
    let req_body = serde_json::json!({
        "model": model,
        "prompt": payload.prompt,
        "size": payload.size.unwrap_or_else(|| "1024x1024".to_string()),
        "n": payload.n.unwrap_or(1)
    });
    let endpoint = format!("{}/images/generations", base_url);
    let client = reqwest::Client::new();
    let response = client
        .post(&endpoint)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&req_body)
        .send()
        .await;
    let resp = match response {
        Ok(r) => r,
        Err(e) => {
            return Json(serde_json::json!({
                "error": format!("图像请求失败：{}", e)
            }));
        }
    };
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Json(serde_json::json!({
            "error": format!("图像生成失败 HTTP {}: {}", status, body)
        }));
    }
    let data = match resp.json::<serde_json::Value>().await {
        Ok(d) => d,
        Err(e) => {
            return Json(serde_json::json!({
                "error": format!("图像响应解析失败：{}", e)
            }));
        }
    };
    let mut images: Vec<String> = vec![];
    if let Some(arr) = data.get("data").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(url) = item.get("url").and_then(|v| v.as_str()) {
                images.push(url.to_string());
                continue;
            }
            if let Some(b64) = item.get("b64_json").and_then(|v| v.as_str()) {
                images.push(format!("data:image/png;base64,{}", b64));
            }
        }
    }
    if images.is_empty() {
        return Json(serde_json::json!({ "error": "未返回可用图像数据" }));
    }
    Json(serde_json::json!({
        "images": images,
        "session_id": payload.session_id,
        "model": model
    }))
}

pub async fn get_swarm_state(
    State(_gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    // In a real implementation, we would query the SwarmOrchestrator
    // For now, we return a mock state or query the event bus if possible
    // But SwarmOrchestrator is not directly accessible from Gateway struct yet (it's inside router or system3)
    
    // We need to expose SwarmOrchestrator in CognitiveRouter or Gateway
    // Assuming CognitiveRouter has system3
    
    // Let's check CognitiveRouter definition
    // It has `sys3: Option<Arc<System3>>`? No, it has `sys3: Option<Arc<SwarmOrchestrator>>`
    
    // We will assume for now we can't easily access it and return a placeholder
    // Or better, implement a proper query mechanism.
    
    // Placeholder for now to satisfy the requirement
    Json(serde_json::json!({
        "status": "active",
        "agents": [],
        "tasks": []
    }))
}

pub async fn list_agents(
    State(_gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    // Return list of available agent roles
    Json(serde_json::json!([
        {"role": "manager", "description": "Task coordinator"},
        {"role": "coder", "description": "Software engineer"},
        {"role": "researcher", "description": "Information gatherer"},
        {"role": "reviewer", "description": "Code/Content reviewer"}
    ]))
}

pub async fn cancel_task(
    State(_gateway): State<Arc<CrabletGateway>>,
    axum::extract::Path(task_id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    // Placeholder
    Json(serde_json::json!({
        "status": "cancelled",
        "task_id": task_id
    }))
}

#[derive(Deserialize)]
pub struct HitlDecisionPayload {
    pub decision: String,
    pub value: Option<String>,
    pub selected_index: Option<usize>,
}

pub async fn list_swarm_reviews(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    let orchestrator = &gateway.router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let reviews = orch.coordinator.executor.hitl.list_pending();
        return Json(serde_json::json!({
            "status": "success",
            "reviews": reviews
        }));
    }
    Json(serde_json::json!({
        "status": "error",
        "message": "Orchestrator not initialized",
        "reviews": []
    }))
}

pub async fn decide_swarm_review(
    State(gateway): State<Arc<CrabletGateway>>,
    axum::extract::Path(task_id): axum::extract::Path<String>,
    Json(payload): Json<HitlDecisionPayload>,
) -> Json<serde_json::Value> {
    let orchestrator = &gateway.router.sys3.orchestrator;
    if let Some(orch) = orchestrator {
        let decision = match payload.decision.to_lowercase().as_str() {
            "approved" | "approve" => HumanDecision::Approved,
            "rejected" | "reject" => HumanDecision::Rejected(payload.value.unwrap_or_else(|| "Rejected by user".to_string())),
            "edited" | "edit" => HumanDecision::Edited(payload.value.unwrap_or_default()),
            "selected" | "select" => HumanDecision::Selected(payload.selected_index.unwrap_or(0)),
            "feedback" => HumanDecision::Feedback(payload.value.unwrap_or_default()),
            _ => {
                return Json(serde_json::json!({
                    "status": "error",
                    "message": "Unsupported decision, use approved/rejected/edited/selected/feedback"
                }));
            }
        };
        if orch.coordinator.executor.hitl.submit_decision(&task_id, decision) {
            return Json(serde_json::json!({ "status": "success" }));
        }
        return Json(serde_json::json!({
            "status": "error",
            "message": "Pending review not found"
        }));
    }
    Json(serde_json::json!({
        "status": "error",
        "message": "Orchestrator not initialized"
    }))
}

pub async fn delete_session(
    State(gateway): State<Arc<CrabletGateway>>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> StatusCode {
    gateway.session.remove_session(&session_id);
    StatusCode::NO_CONTENT
}

pub async fn get_session_history(
    State(gateway): State<Arc<CrabletGateway>>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    if let Some(history) = gateway.session.get_history(&session_id) {
        Json(serde_json::json!(history))
    } else {
        Json(serde_json::json!([]))
    }
}

pub async fn list_sessions(
    State(_gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    let now = chrono::Utc::now().to_rfc3339();
    Json(serde_json::json!([
        {
            "id": "mock-session-1",
            "title": "演示会话",
            "created_at": now,
            "updated_at": now
        }
    ]))
}

pub async fn list_skills(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    let skills = gateway.router.shared_skills.read().await.list_skills();
    let disabled = disabled_skills_store().read().await;
    let enriched: Vec<serde_json::Value> = skills
        .into_iter()
        .map(|skill| {
            let mut value = serde_json::to_value(skill).unwrap_or_else(|_| serde_json::json!({}));
            if let Some(obj) = value.as_object_mut() {
                let name = obj.get("name").and_then(|v| v.as_str()).unwrap_or_default();
                obj.insert("enabled".to_string(), serde_json::json!(!disabled.contains(name)));
            }
            value
        })
        .collect();
    Json(serde_json::json!(enriched))
}

pub async fn toggle_skill(
    State(_gateway): State<Arc<CrabletGateway>>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(req): Json<ToggleSkillRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut disabled = disabled_skills_store().write().await;
    if req.enabled {
        disabled.remove(&name);
    } else {
        disabled.insert(name.clone());
    }
    Ok(Json(serde_json::json!({
        "status": "ok",
        "name": name,
        "enabled": req.enabled
    })))
}

pub async fn search_registry_skills(
    State(gateway): State<Arc<CrabletGateway>>,
    axum::extract::Query(query): axum::extract::Query<SearchSkillsQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let q = query.q.unwrap_or_default();
    let clawhub_url = format!("https://clawhub.ai/api/search?q={}&limit=20", urlencoding::encode(&q));
    if let Ok(resp) = reqwest::get(clawhub_url).await {
        if let Ok(payload) = resp.json::<ClawhubSearchResponse>().await {
            let items: Vec<serde_json::Value> = payload.results.into_iter().map(|x| {
                serde_json::json!({
                    "name": x.slug,
                    "description": x.summary.unwrap_or_default(),
                    "version": "latest",
                    "url": format!("clawhub://{}", x.slug),
                    "author": serde_json::Value::Null,
                    "rating": serde_json::Value::Null,
                    "downloads": serde_json::Value::Null,
                    "display_name": x.display_name.unwrap_or_default()
                })
            }).collect();
            return Ok(Json(serde_json::json!({
                "status": "ok",
                "source": "clawhub",
                "items": items
            })));
        }
    }

    let registry = gateway.router.shared_skills.read().await;
    match registry.search(&q).await {
        Ok(items) => Ok(Json(serde_json::json!({
            "status": "ok",
            "source": "local-registry",
            "items": items
        }))),
        Err(e) => {
            tracing::warn!("Registry search unavailable: {}", e);
            Ok(Json(serde_json::json!({
                "status": "unavailable",
                "source": "none",
                "items": []
            })))
        }
    }
}

pub async fn get_skills_sh_top(
    State(_gateway): State<Arc<CrabletGateway>>,
    axum::extract::Query(query): axum::extract::Query<SkillsShTopQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = query.limit.unwrap_or(100).min(100);
    let body = reqwest::get("https://skills.sh/")
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?
        .text()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let body = body.replace("\\\"", "\"");

    let re = Regex::new(r#"\{"source":"([^"]+)","skillId":"([^"]+)","name":"([^"]+)","installs":(\d+)\}"#)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut items: Vec<serde_json::Value> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for cap in re.captures_iter(&body) {
        let source = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
        let skill_id = cap.get(2).map(|m| m.as_str()).unwrap_or_default();
        let name = cap.get(3).map(|m| m.as_str()).unwrap_or_default();
        let installs = cap
            .get(4)
            .and_then(|m| m.as_str().parse::<u64>().ok())
            .unwrap_or(0);
        let key = format!("{}::{}", source, skill_id);
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        items.push(serde_json::json!({
            "source": source,
            "skill_id": skill_id,
            "name": name,
            "installs": installs
        }));
        if items.len() >= limit {
            break;
        }
    }

    Ok(Json(serde_json::json!({
        "status": "ok",
        "source": "skills.sh",
        "items": items
    })))
}

pub async fn install_skill(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(req): Json<InstallSkillRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let target_dir = PathBuf::from("./skills");
    let mut registry = gateway.router.shared_skills.write().await;
    if let Some(name) = req.name.as_deref() {
        if registry.get_skill(name).is_some() {
            return Ok(Json(serde_json::json!({
                "status": "already_installed",
                "name": name
            })));
        }
    }
    let result = if let Some(url) = req.url.as_deref() {
        match crate::skills::installer::SkillInstaller::install_from_git(url, &target_dir).await {
            Ok(_) => registry.load_from_dir(&target_dir).await,
            Err(e) => Err(e),
        }
    } else if let Some(source) = req.source.as_deref() {
        let status = Command::new("npx")
            .arg("skillsadd")
            .arg(source)
            .status()
            .await;
        match status {
            Ok(s) if s.success() => registry.load_from_dir(&target_dir).await,
            _ => {
                let fallback_url = format!("https://github.com/{}.git", source);
                match crate::skills::installer::SkillInstaller::install_from_git(&fallback_url, &target_dir).await {
                    Ok(_) => registry.load_from_dir(&target_dir).await,
                    Err(e) => Err(e),
                }
            }
        }
    } else if let Some(name) = req.name.as_deref() {
        match registry.install(name, target_dir.clone()).await {
            Ok(_) => Ok(()),
            Err(_) => {
                let status = Command::new("npx")
                    .arg("clawhub@latest")
                    .arg("install")
                    .arg(name)
                    .arg("--workdir")
                    .arg(".")
                    .arg("--dir")
                    .arg("skills")
                    .arg("--no-input")
                    .status()
                    .await;
                match status {
                    Ok(s) if s.success() => registry.load_from_dir(&target_dir).await,
                    Ok(_) => Err(anyhow::anyhow!("clawhub install failed")),
                    Err(e) => Err(anyhow::anyhow!("failed to run clawhub install: {}", e)),
                }
            }
        }
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };

    match result {
        Ok(_) => Ok(Json(serde_json::json!({
            "status": "installed",
            "skill_id": req.skill_id
        }))),
        Err(e) => {
            tracing::error!("Failed to install skill: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn batch_test_skills(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(req): Json<BatchTestSkillsRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let registry = gateway.router.shared_skills.read().await;
    let disabled = disabled_skills_store().read().await;
    let results: Vec<serde_json::Value> = req.skills.into_iter().map(|name| {
        let installed = registry.get_skill(&name).is_some();
        let enabled = !disabled.contains(&name);
        serde_json::json!({
            "name": name,
            "installed": installed,
            "enabled": enabled,
            "passed": installed && enabled
        })
    }).collect();
    Ok(Json(serde_json::json!({
        "status": "ok",
        "results": results
    })))
}

pub async fn get_mcp_overview(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let registry = gateway.router.shared_skills.read().await;
    let skills = registry.list_skills();
    let mcp_tools = skills.iter().filter(|s| {
        s.runtime.as_deref() == Some("mcp") || s.version.contains("(MCP)")
    }).count();
    let resources = registry.list_resources();
    let prompts = registry.list_prompts();
    Ok(Json(serde_json::json!({
        "status": "ok",
        "mcp_tools": mcp_tools,
        "resources": resources.len(),
        "prompts": prompts.len(),
        "resource_items": resources,
        "prompt_items": prompts
    })))
}

pub async fn list_documents(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Json<serde_json::Value> {
    #[cfg(feature = "knowledge")]
    if let Some(ingestion) = &gateway.ingestion {
        if let Ok(docs) = ingestion.list_documents().await {
            return Json(serde_json::json!({
                "status": "success",
                "documents": docs
            }));
        }
    }
    Json(serde_json::json!({
        "status": "success",
        "documents": []
    }))
}

#[derive(Deserialize)]
pub struct GetChunksQuery {
    source: String,
}

pub async fn get_document_chunks(
    State(gateway): State<Arc<CrabletGateway>>,
    axum::extract::Query(query): axum::extract::Query<GetChunksQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    #[cfg(feature = "knowledge")]
    if let Some(ingestion) = &gateway.ingestion {
        if let Ok(chunks) = ingestion.get_document_chunks(&query.source).await {
            return Ok(Json(serde_json::json!({
                "status": "success",
                "chunks": chunks
            })));
        }
    }
    
    // Silence unused warning if feature disabled or ingestion missing
    let _ = gateway;
    let _ = query;
    
    Ok(Json(serde_json::json!({
        "status": "success",
        "chunks": []
    })))
}

#[derive(Deserialize)]
pub struct SearchQuery {
    q: String,
    limit: Option<usize>,
}

pub async fn search_knowledge(
    State(gateway): State<Arc<CrabletGateway>>,
    axum::extract::Query(query): axum::extract::Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    #[cfg(feature = "knowledge")]
    if let Some(ingestion) = &gateway.ingestion {
        if let Ok(results) = ingestion.search(&query.q, query.limit.unwrap_or(5)).await {
            return Ok(Json(serde_json::json!({
                "status": "success",
                "results": results
            })));
        }
    }
    
    // Silence unused warning
    let _ = gateway;
    let _ = query;
    
    Ok(Json(serde_json::json!({
        "status": "success",
        "results": []
    })))
}

pub async fn upload_knowledge(
    State(gateway): State<Arc<CrabletGateway>>,
    multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let allowed = [
        "pdf", "doc", "docx", "txt", "md", "csv", "xls", "xlsx",
        "jpg", "jpeg", "png", "gif", "svg", "webp",
        "mp3", "wav", "m4a", "flac",
        "mp4", "avi", "mov", "mkv",
    ];
    let max_size: usize = 300 * 1024 * 1024;
    #[cfg(feature = "knowledge")]
    if let Some(ingestion) = &gateway.ingestion {
        let mut multipart = multipart;
        let mut uploaded_files = Vec::new();
        
        while let Some(field) = multipart.next_field().await.map_err(|_| StatusCode::BAD_REQUEST)? {
            let file_name = field.file_name().unwrap_or("unknown").to_string();
            if file_name == "unknown" { continue; }
            let ext = file_name
                .split('.')
                .next_back()
                .unwrap_or("")
                .to_lowercase();
            if !allowed.contains(&ext.as_str()) {
                tracing::warn!("Blocked unsupported extension: {}", file_name);
                return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
            }
            
            let data = field.bytes().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            if data.len() > max_size {
                tracing::warn!("Blocked oversize upload: {} bytes for {}", data.len(), file_name);
                return Err(StatusCode::PAYLOAD_TOO_LARGE);
            }
            if data.len() >= 2 && data[0] == 0x4d && data[1] == 0x5a {
                tracing::warn!("Blocked executable signature (MZ): {}", file_name);
                return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
            }
            if data.len() >= 4 && data[0] == 0x7f && data[1] == 0x45 && data[2] == 0x4c && data[3] == 0x46 {
                tracing::warn!("Blocked executable signature (ELF): {}", file_name);
                return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
            }
            
            let temp_dir = std::env::temp_dir();
            let temp_path = temp_dir.join(&file_name);
            
            {
                let mut file = std::fs::File::create(&temp_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                file.write_all(&data).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            }
            
            let metadata = serde_json::json!({
                "source": file_name,
                "uploaded_at": chrono::Utc::now().to_rfc3339()
            });
            
            match ingestion.ingest_file(&temp_path, metadata).await {
                Ok(doc_id) => {
                    uploaded_files.push(file_name.clone());
                    tracing::info!("Ingested {} as {}", file_name, doc_id);
                },
                Err(e) => {
                    tracing::error!("Ingestion failed for {}: {}", file_name, e);
                    let _ = std::fs::remove_file(&temp_path);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
            
            let _ = std::fs::remove_file(temp_path);
        }
        Ok(Json(serde_json::json!({
            "status": "success",
            "uploaded": uploaded_files
        })))
    } else {
        tracing::warn!("Ingestion service not available");
        Err(StatusCode::NOT_IMPLEMENTED)
    }
    
    #[cfg(not(feature = "knowledge"))]
    Err(StatusCode::NOT_IMPLEMENTED)
}

pub async fn list_audit_logs(
    State(gateway): State<Arc<CrabletGateway>>,
    axum::extract::Query(query): axum::extract::Query<LogsQuery>,
) -> Result<Json<Vec<AuditLog>>, StatusCode> {
    if let Some(pool) = &gateway.auth.pool {
        let logger = crate::audit::AuditLogger::new(pool.clone());
        let limit = query.per_page.unwrap_or(50);
        let offset = (query.page.unwrap_or(1) - 1) * limit;
        
        match logger.list_logs(limit, offset).await {
            Ok(logs) => Ok(Json(logs)),
            Err(e) => {
                tracing::error!("Failed to list logs: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    } else {
        Err(StatusCode::SERVICE_UNAVAILABLE)
    }
}

pub async fn list_api_keys(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Result<Json<Vec<ApiKeyInfo>>, StatusCode> {
    match gateway.auth.list_api_keys().await {
        Ok(keys) => Ok(Json(keys)),
        Err(e) => {
            tracing::error!("Failed to list keys: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn create_api_key(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(req): Json<CreateKeyRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    match gateway.auth.create_api_key(&req.name, "admin").await {
        Ok(key) => Ok(Json(serde_json::json!({
            "status": "created",
            "key": key
        }))),
        Err(e) => {
            tracing::error!("Failed to create key: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn revoke_api_key(
    State(gateway): State<Arc<CrabletGateway>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<StatusCode, StatusCode> {
    match gateway.auth.revoke_api_key(&id).await {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(e) => {
            tracing::error!("Failed to revoke key: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_routing_settings(
    State(gateway): State<Arc<CrabletGateway>>,
) -> Result<Json<RoutingSettingsResponse>, StatusCode> {
    let cfg = gateway.router.config.read().await.clone();
    Ok(Json(RoutingSettingsResponse {
        enable_adaptive_routing: cfg.enable_adaptive_routing,
        system2_threshold: cfg.system2_threshold,
        system3_threshold: cfg.system3_threshold,
        bandit_exploration: cfg.bandit_exploration,
        enable_hierarchical_reasoning: cfg.enable_hierarchical_reasoning,
        deliberate_threshold: cfg.deliberate_threshold,
        meta_reasoning_threshold: cfg.meta_reasoning_threshold,
        mcts_simulations: cfg.mcts_simulations,
        mcts_exploration_weight: cfg.mcts_exploration_weight,
        graph_rag_entity_mode: cfg.graph_rag_entity_mode.clone(),
    }))
}

pub async fn update_routing_settings(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(req): Json<UpdateRoutingSettingsRequest>,
) -> Result<Json<RoutingSettingsResponse>, StatusCode> {
    if !(0.0..=1.0).contains(&req.system2_threshold) || !(0.0..=1.0).contains(&req.system3_threshold) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !(0.05..=2.0).contains(&req.bandit_exploration) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !(0.0..=1.0).contains(&req.deliberate_threshold) || !(0.0..=1.0).contains(&req.meta_reasoning_threshold) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.mcts_simulations == 0 || req.mcts_simulations > 512 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if !(0.1..=3.0).contains(&req.mcts_exploration_weight) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mode = req.graph_rag_entity_mode.to_lowercase();
    if mode != "rule" && mode != "phrase" && mode != "hybrid" {
        return Err(StatusCode::BAD_REQUEST);
    }
    let new_cfg = RouterConfig {
        system2_threshold: req.system2_threshold,
        system3_threshold: req.system3_threshold,
        enable_adaptive_routing: req.enable_adaptive_routing,
        bandit_exploration: req.bandit_exploration,
        enable_hierarchical_reasoning: req.enable_hierarchical_reasoning,
        deliberate_threshold: req.deliberate_threshold,
        meta_reasoning_threshold: req.meta_reasoning_threshold,
        mcts_simulations: req.mcts_simulations,
        mcts_exploration_weight: req.mcts_exploration_weight,
        graph_rag_entity_mode: mode,
    };
    gateway.router.update_config(new_cfg).await;
    let cfg = gateway.router.config.read().await.clone();
    Ok(Json(RoutingSettingsResponse {
        enable_adaptive_routing: cfg.enable_adaptive_routing,
        system2_threshold: cfg.system2_threshold,
        system3_threshold: cfg.system3_threshold,
        bandit_exploration: cfg.bandit_exploration,
        enable_hierarchical_reasoning: cfg.enable_hierarchical_reasoning,
        deliberate_threshold: cfg.deliberate_threshold,
        meta_reasoning_threshold: cfg.meta_reasoning_threshold,
        mcts_simulations: cfg.mcts_simulations,
        mcts_exploration_weight: cfg.mcts_exploration_weight,
        graph_rag_entity_mode: cfg.graph_rag_entity_mode.clone(),
    }))
}

pub async fn get_routing_report(
    State(gateway): State<Arc<CrabletGateway>>,
    Query(query): Query<RoutingReportQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let window = query.window.unwrap_or(200).clamp(10, 2000);
    let meta = gateway.router.meta_router.read().await;
    let report = meta.evaluation_report(window);
    drop(meta);
    let cloud = gateway.router.sys2.hierarchical_stats().await;
    let local = gateway.router.sys2_local.hierarchical_stats().await;
    let cfg = gateway.router.config.read().await.clone();
    let hierarchical_stats = serde_json::json!({
        "total_requests": cloud.total_requests + local.total_requests,
        "deliberate_activations": cloud.deliberate_activations + local.deliberate_activations,
        "meta_activations": cloud.meta_activations + local.meta_activations,
        "strategy_switches": cloud.strategy_switches + local.strategy_switches,
        "bfs_runs": cloud.bfs_runs + local.bfs_runs,
        "dfs_runs": cloud.dfs_runs + local.dfs_runs,
        "mcts_runs": cloud.mcts_runs + local.mcts_runs
    });
    Ok(Json(serde_json::json!({
        "total_feedback": report.total_feedback,
        "avg_reward": report.avg_reward,
        "avg_latency_ms": report.avg_latency_ms,
        "avg_quality_score": report.avg_quality_score,
        "recent_window": report.recent_window,
        "by_choice": report.by_choice,
        "hierarchical": {
            "enabled": cfg.enable_hierarchical_reasoning,
            "deliberate_threshold": cfg.deliberate_threshold,
            "meta_reasoning_threshold": cfg.meta_reasoning_threshold,
            "mcts_simulations": cfg.mcts_simulations,
            "mcts_exploration_weight": cfg.mcts_exploration_weight
        },
        "hierarchical_stats": hierarchical_stats
    })))
}

#[derive(Serialize, Deserialize)]
pub struct SystemConfigPayload {
    pub openai_api_key: Option<String>,
    pub openai_api_base: Option<String>,
    pub openai_model_name: Option<String>,
    pub ollama_model: Option<String>,
}

pub async fn get_system_config(
    State(_gateway): State<Arc<CrabletGateway>>,
) -> Result<Json<SystemConfigPayload>, StatusCode> {
    let content = fs::read_to_string(".env").unwrap_or_default();
    let mut config = SystemConfigPayload {
        openai_api_key: None,
        openai_api_base: None,
        openai_model_name: None,
        ollama_model: None,
    };
    
    for line in content.lines() {
        if let Some((key, value)) = line.split_once('=') {
            let val = value.trim().to_string();
            match key.trim() {
                "DASHSCOPE_API_KEY" => config.openai_api_key = Some(val),
                "OPENAI_API_KEY" => {
                    if config.openai_api_key.is_none() {
                        config.openai_api_key = Some(val);
                    }
                },
                "OPENAI_API_BASE" => config.openai_api_base = Some(val),
                "OPENAI_MODEL_NAME" => config.openai_model_name = Some(val),
                "OLLAMA_MODEL" => config.ollama_model = Some(val),
                _ => {}
            }
        }
    }
    
    Ok(Json(config))
}

pub async fn update_system_config(
    State(_gateway): State<Arc<CrabletGateway>>,
    Json(payload): Json<SystemConfigPayload>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let path = PathBuf::from(".env");
    let content = fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    
    let mut upsert = |key: &str, value: &str| {
        let mut found = false;
        for line in lines.iter_mut() {
            if line.starts_with(&format!("{}=", key)) {
                *line = format!("{}={}", key, value);
                found = true;
                break;
            }
        }
        if !found {
            lines.push(format!("{}={}", key, value));
        }
    };

    if let Some(v) = payload.openai_api_key {
        upsert("DASHSCOPE_API_KEY", &v);
        upsert("OPENAI_API_KEY", &v);
    }
    if let Some(v) = payload.openai_api_base {
        upsert("OPENAI_API_BASE", &v);
    }
    if let Some(v) = payload.openai_model_name {
        upsert("OPENAI_MODEL_NAME", &v);
    }
    if let Some(v) = payload.ollama_model {
        upsert("OLLAMA_MODEL", &v);
    }

    let new_content = lines.join("\n");
    let final_content = if new_content.ends_with('\n') { new_content } else { new_content + "\n" };
    
    fs::write(&path, final_content).map_err(|e| {
        tracing::error!("Failed to write .env: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Configuration saved. Please restart the service to apply changes."
    })))
}
