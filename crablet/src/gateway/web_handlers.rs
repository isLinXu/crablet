use axum::{
    extract::{State, Json, Multipart, Query},
    response::sse::{Event, Sse},
    http::StatusCode,
};
#[cfg(feature = "knowledge")]
use std::io::Write;
#[cfg(feature = "knowledge")]
use std::str::FromStr;
use std::sync::Arc;
use std::sync::OnceLock;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use futures::stream::StreamExt;
use futures::stream::BoxStream;
use crate::gateway::server::CrabletGateway;
use crate::agent::hitl::HumanDecision;
use crate::cognitive::router::RouterConfig;
use crate::cognitive::streaming_pipeline::{
    EmptyDeltaFilterMiddleware, FinalizeSummaryMiddleware, MetricsMiddleware, StreamChunk, StreamingPipeline,
};
use crate::cognitive::llm::{LlmClient, OpenAiClient};
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

fn resolve_env_file_path() -> PathBuf {
    if let Ok(v) = std::env::var("CRABLET_ENV_FILE") {
        let p = PathBuf::from(v);
        if p.exists() {
            return p;
        }
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidates = [
        cwd.join(".env"),
        cwd.join("crablet").join(".env"),
        cwd.join("../crablet").join(".env"),
    ];
    candidates
        .into_iter()
        .find(|p| p.exists())
        .unwrap_or_else(|| cwd.join(".env"))
}

fn env_value_from_file(key: &str) -> Option<String> {
    let content = fs::read_to_string(resolve_env_file_path()).ok()?;
    for line in content.lines() {
        if let Some((k, v)) = line.split_once('=') {
            if k.trim() == key {
                let value = v.trim().to_string();
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
    }
    None
}

fn llm_from_route(route: Option<&RouteSelection>) -> Option<Arc<Box<dyn LlmClient>>> {
    let base_url = route
        .and_then(|r| r.api_base_url.as_ref().map(|v| v.trim().to_string()))
        .filter(|v| !v.is_empty())
        .or_else(|| env_value_from_file("OPENAI_API_BASE"))?;
    let api_key = route
        .and_then(|r| r.api_key.as_ref().map(|v| v.trim().to_string()))
        .filter(|v| !v.is_empty())
        .or_else(|| env_value_from_file("DASHSCOPE_API_KEY"))
        .or_else(|| env_value_from_file("OPENAI_API_KEY"))?;
    if base_url.is_empty() || api_key.is_empty() {
        return None;
    }
    let model = route
        .and_then(|r| r.model.as_ref().map(|v| v.trim().to_string()))
        .filter(|v| !v.is_empty())
        .or_else(|| env_value_from_file("OPENAI_MODEL_NAME"))
        .unwrap_or_else(|| "qwen-plus".to_string());
    if model.is_empty() {
        return None;
    }
    let client = OpenAiClient {
        api_key,
        base_url: base_url.trim_end_matches('/').to_string(),
        model,
        client: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .ok()?,
    };
    Some(Arc::new(Box::new(client)))
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
    #[allow(unused_mut)]
    let mut refs: Vec<serde_json::Value> = Vec::new();
    let mut graph_entities: Vec<String> = Vec::new();
    #[allow(unused_mut)]
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
    if let Some(llm) = llm_from_route(payload.route.as_ref()) {
        let mut messages = Vec::new();
        let system_context = system_prompt_markdown();
        if !system_context.is_empty() {
            messages.push(Message::system(system_context));
        }
        messages.push(Message::user(payload.message.clone()));
        return match llm.chat_complete(&messages).await {
            Ok(response) => Json(serde_json::json!({
                "response": response,
                "traces": [],
                "cognitive_layer": "system2",
                "session_id": session_id
            })),
            Err(e) => Json(serde_json::json!({
                "error": e.to_string()
            })),
        };
    }
    
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

/// 根据输入内容推断认知层
fn infer_cognitive_layer_from_input(input: &str) -> &'static str {
    let input_lower = input.to_lowercase();
    
    // 问候语检测 - 应该使用 System 1
    let greeting_patterns = [
        "hi", "hello", "hey", "greetings", "good morning", "good afternoon", "good evening",
        "你好", "嗨", "您好", "早上好", "下午好", "晚上好", "在吗", "在么",
    ];
    for pattern in &greeting_patterns {
        if input_lower.trim() == *pattern || input_lower.starts_with(pattern) {
            return "system1";
        }
    }
    
    // 人设/身份查询 - 应该使用 System 1（闲聊类）
    let persona_patterns = [
        "who are you", "what are you", "your name", "introduce yourself", "tell me about yourself",
        "你是谁", "你是什么", "你叫什么", "介绍一下", "你是干嘛的", "你是做什么的",
        "你的身份", "你的角色", "你是ai吗", "你是人工智能吗", "你是机器人吗",
    ];
    for pattern in &persona_patterns {
        if input_lower.contains(pattern) {
            return "system1";
        }
    }
    
    // 闲聊/社交对话 - 应该使用 System 1
    let chat_patterns = [
        "how are you", "what's up", "how's it going", "nice to meet you", "thank you", "thanks",
        "你好吗", "最近怎么样", "很高兴认识你", "谢谢", "多谢", "哈哈", "呵呵", "嘿嘿",
        "好的", "ok", "okay", "嗯", "哦", "啊", "呢", "吧", "吗",
    ];
    for pattern in &chat_patterns {
        if input_lower.trim() == *pattern || input_lower.starts_with(pattern) {
            return "system1";
        }
    }
    
    // 简单个人问题 - 应该使用 System 1
    let personal_patterns = [
        "how old are you", "where are you from", "what do you like", "your favorite",
        "你多大了", "你几岁了", "你喜欢什么", "你的爱好", "你喜欢", "你的",
    ];
    for pattern in &personal_patterns {
        if input_lower.contains(pattern) {
            return "system1";
        }
    }
    
    // 简单帮助请求
    let help_patterns = [
        "help", "assist", "support", "how to", "what can you do",
        "帮助", "怎么用", "如何使用", "你能做什么", "有什么功能",
    ];
    for pattern in &help_patterns {
        if input_lower.contains(pattern) {
            return "system1";
        }
    }
    
    // 状态查询
    let status_patterns = [
        "status", "system info", "health", "check", "diagnostics",
        "状态", "系统信息", "健康检查", "诊断",
    ];
    for pattern in &status_patterns {
        if input_lower.contains(pattern) {
            return "system1";
        }
    }
    
    // 深度研究检测 - 应该使用 System 3
    let research_patterns = [
        "research", "deep research", "investigate", "explore in depth", "comprehensive analysis",
        "研究", "深度研究", "深入分析", "全面调查", "详细探讨",
    ];
    for pattern in &research_patterns {
        if input_lower.contains(pattern) {
            return "system3";
        }
    }
    
    // 多步骤任务检测
    let multistep_patterns = [
        "first", "then", "next", "after", "finally", "step by step",
        "首先", "然后", "接着", "最后", "一步步", "步骤",
    ];
    let multistep_count = multistep_patterns.iter().filter(|p| input_lower.contains(*p)).count();
    if multistep_count >= 2 {
        return "system3";
    }
    
    // 代码/分析任务 - 使用 System 2
    let code_patterns = [
        "code", "function", "implement", "program", "debug", "refactor", "algorithm",
        "代码", "编写", "实现", "函数", "调试", "程序", "算法",
        "analyze", "compare", "evaluate", "assess", "review", "examine",
        "分析", "比较", "评估", "评价", "优缺点",
    ];
    for pattern in &code_patterns {
        if input_lower.contains(pattern) {
            return "system2";
        }
    }
    
    // 默认使用 System 2
    "system2"
}

pub async fn chat_stream(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(payload): Json<ChatRequest>,
) -> Sse<BoxStream<'static, Result<Event, axum::Error>>> {
    let session_id = payload.session_id.clone().unwrap_or_else(|| "default".to_string());
    let session_id_for_event = session_id.clone();
    let message = payload.message.clone();
    // 调试日志：记录接收到的消息信息
    tracing::info!("[chat_stream] 收到消息，长度: {} 字符", message.len());
    if message.contains("[文件内容]") {
        let file_content_start = message.find("[文件内容]").unwrap_or(0);
        let preview = &message[file_content_start..message.len().min(file_content_start + 200)];
        tracing::info!("[chat_stream] 检测到文件内容: {}", preview);
    }
    if message.contains("[知识检索上下文]") {
        tracing::info!("[chat_stream] 检测到知识检索上下文");
    }

    // 首先尝试使用 CognitiveRouter 进行路由（支持 System 1 快速响应）
    // 注意：这里使用原始消息，不带 persona，让 router 自己决定
    let router_result = gateway.router.process(&message, &session_id).await;
    
    // 如果 System 1 成功处理（返回 Ok），直接返回快速响应
    if let Ok((response, traces)) = &router_result {
        let cognitive_layer = infer_cognitive_layer(response, traces);
        // 如果是 System 1 响应，直接返回，不走 LLM
        if cognitive_layer == "system1" {
            tracing::info!("[chat_stream] System 1 快速响应: {}", response.chars().take(50).collect::<String>());
            let response = response.clone();
            let traces = traces.clone();
            let session_id_for_stream = session_id_for_event.clone();
            let source_stream = async_stream::stream! {
                // 发送认知层事件
                yield StreamChunk {
                    chunk_type: "cognitive_layer".to_string(),
                    content: None,
                    payload: Some(serde_json::json!({ "layer": "system1" })),
                };
                // 发送 trace
                for (i, step) in traces.iter().enumerate() {
                    yield StreamChunk {
                        chunk_type: "trace".to_string(),
                        content: None,
                        payload: Some(serde_json::json!({ "step": step, "index": i })),
                    };
                }
                // 发送完整响应（System 1 响应通常较短，一次性发送）
                yield StreamChunk::delta(response);
            };
            let pipeline = StreamingPipeline::new(vec![
                Arc::new(EmptyDeltaFilterMiddleware),
                Arc::new(MetricsMiddleware),
                Arc::new(FinalizeSummaryMiddleware),
            ]);
            let stream: BoxStream<'static, Result<Event, axum::Error>> = pipeline.process(source_stream)
                .map(move |chunk| {
                    let data = serde_json::json!({
                        "type": chunk.chunk_type,
                        "content": chunk.content,
                        "payload": chunk.payload,
                        "session_id": session_id_for_stream.clone()
                    });
                    Ok(Event::default().data(data.to_string()))
                })
                .boxed();
            return Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default());
        }
    }
    
    // System 1 未匹配或返回错误，回退到 LLM 流式输出
    let cognitive_layer = infer_cognitive_layer_from_input(&message);
    tracing::info!("[chat_stream] 推断认知层: {}", cognitive_layer);

    let enhanced_input = with_identity_persona_input(&message);
    let llm = llm_from_route(payload.route.as_ref()).unwrap_or_else(|| gateway.router.sys2.llm.clone());
    let gateway_for_stream = gateway.clone();
    let pipeline = StreamingPipeline::new(vec![
        Arc::new(EmptyDeltaFilterMiddleware),
        Arc::new(MetricsMiddleware),
        Arc::new(FinalizeSummaryMiddleware),
    ]);
    let cognitive_layer_for_stream = cognitive_layer.to_string();
    let source_stream = async_stream::stream! {
        // 首先发送认知层事件
        yield StreamChunk {
            chunk_type: "cognitive_layer".to_string(),
            content: None,
            payload: Some(serde_json::json!({ "layer": cognitive_layer_for_stream })),
        };
        
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
    let stream: BoxStream<'static, Result<Event, axum::Error>> = pipeline.process(source_stream)
        .map(move |chunk| {
            let data = serde_json::json!({
                "type": chunk.chunk_type,
                "content": chunk.content,
                "payload": chunk.payload,
                "session_id": session_id_for_event.clone()
            });
            Ok(Event::default().data(data.to_string()))
        })
        .boxed();
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

// ==================== 新增 Skills API ====================

#[derive(Deserialize)]
pub struct SemanticSearchRequest {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default = "default_min_similarity")]
    pub min_similarity: f32,
}

fn default_limit() -> usize { 10 }
fn default_min_similarity() -> f32 { 0.5 }

#[derive(Serialize)]
pub struct SemanticSearchResult {
    pub skill_name: String,
    pub description: String,
    pub version: String,
    pub similarity_score: f32,
    pub match_type: String,
    pub tags: Vec<String>,
    pub author: String,
    pub category: String,
}

/// 语义搜索技能
pub async fn semantic_search_skills(
    State(gateway): State<Arc<CrabletGateway>>,
    Json(req): Json<SemanticSearchRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let _skills_dir = std::path::PathBuf::from("./skills");
    
    // 创建嵌入服务（使用简单的基于关键词的回退方案）
    let results = perform_semantic_search(&gateway, &req.query, req.limit, req.min_similarity).await;
    
    match results {
        Ok(items) => Ok(Json(serde_json::json!({
            "status": "ok",
            "query": req.query,
            "results": items
        }))),
        Err(e) => {
            tracing::warn!("Semantic search failed: {}, falling back to keyword search", e);
            // 回退到关键词搜索
            let fallback = perform_keyword_search(&gateway, &req.query, req.limit).await;
            Ok(Json(serde_json::json!({
                "status": "fallback",
                "query": req.query,
                "results": fallback,
                "note": "Semantic search unavailable, using keyword search"
            })))
        }
    }
}

async fn perform_semantic_search(
    gateway: &Arc<CrabletGateway>,
    query: &str,
    limit: usize,
    min_similarity: f32,
) -> anyhow::Result<Vec<SemanticSearchResult>> {
    let registry = gateway.router.shared_skills.read().await;
    let all_skills = registry.list_skills();
    let query_lower = query.to_lowercase();
    let query_words: Vec<&str> = query_lower.split_whitespace().collect();
    
    let mut results: Vec<(SemanticSearchResult, f32)> = Vec::new();
    
    for skill in all_skills {
        let name_lower = skill.name.to_lowercase();
        let desc_lower = skill.description.to_lowercase();
        
        // 计算关键词匹配分数
        let mut keyword_score = 0.0f32;
        let mut matched_keywords = 0;
        
        for word in &query_words {
            if name_lower.contains(word) {
                keyword_score += 0.4; // 名称匹配权重高
                matched_keywords += 1;
            }
            if desc_lower.contains(word) {
                keyword_score += 0.2; // 描述匹配权重中等
                matched_keywords += 1;
            }
        }
        
        // 计算语义相似度（基于字符 n-gram 的简单实现）
        let semantic_score = calculate_semantic_similarity(&query_lower, &desc_lower);
        
        // 综合分数
        let total_score = (keyword_score * 0.6 + semantic_score * 0.4).min(1.0);
        
        if total_score >= min_similarity || matched_keywords > 0 {
            results.push((SemanticSearchResult {
                skill_name: skill.name.clone(),
                description: skill.description.clone(),
                version: skill.version.clone(),
                similarity_score: total_score,
                match_type: if matched_keywords > 0 && semantic_score > 0.3 {
                    "hybrid".to_string()
                } else if matched_keywords > 0 {
                    "keyword".to_string()
                } else {
                    "semantic".to_string()
                },
                tags: Vec::new(), // 可以从 manifest 中提取
                author: skill.author.clone().unwrap_or_else(|| "Unknown".to_string()),
                category: "General".to_string(),
            }, total_score));
        }
    }
    
    // 按分数排序
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);
    
    Ok(results.into_iter().map(|(r, _)| r).collect())
}

async fn perform_keyword_search(
    gateway: &Arc<CrabletGateway>,
    query: &str,
    limit: usize,
) -> Vec<SemanticSearchResult> {
    let registry = gateway.router.shared_skills.read().await;
    let all_skills = registry.list_skills();
    let query_lower = query.to_lowercase();
    
    let mut results: Vec<(SemanticSearchResult, f32)> = Vec::new();
    
    for skill in all_skills {
        let name_lower = skill.name.to_lowercase();
        let desc_lower = skill.description.to_lowercase();
        
        let mut score = 0.0f32;
        if name_lower.contains(&query_lower) {
            score += 1.0;
        }
        if desc_lower.contains(&query_lower) {
            score += 0.5;
        }
        
        // 部分匹配
        for word in query_lower.split_whitespace() {
            if name_lower.contains(word) {
                score += 0.3;
            }
            if desc_lower.contains(word) {
                score += 0.15;
            }
        }
        
        if score > 0.0 {
            results.push((SemanticSearchResult {
                skill_name: skill.name.clone(),
                description: skill.description.clone(),
                version: skill.version.clone(),
                similarity_score: score.min(1.0),
                match_type: "keyword".to_string(),
                tags: Vec::new(),
                author: skill.author.clone().unwrap_or_else(|| "Unknown".to_string()),
                category: "General".to_string(),
            }, score));
        }
    }
    
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);
    
    results.into_iter().map(|(r, _)| r).collect()
}

fn calculate_semantic_similarity(a: &str, b: &str) -> f32 {
    // 简单的基于字符 n-gram 的相似度计算
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    
    if a_chars.len() < 2 || b_chars.len() < 2 {
        return if a == b { 1.0 } else { 0.0 };
    }
    
    // 生成 2-gram
    let mut a_ngrams: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut b_ngrams: std::collections::HashSet<String> = std::collections::HashSet::new();
    
    for i in 0..a_chars.len() - 1 {
        let ngram: String = a_chars[i..i + 2].iter().collect();
        a_ngrams.insert(ngram);
    }
    
    for i in 0..b_chars.len() - 1 {
        let ngram: String = b_chars[i..i + 2].iter().collect();
        b_ngrams.insert(ngram);
    }
    
    // 计算 Jaccard 相似度
    let intersection: std::collections::HashSet<_> = a_ngrams.intersection(&b_ngrams).collect();
    let union: std::collections::HashSet<_> = a_ngrams.union(&b_ngrams).collect();
    
    if union.is_empty() {
        0.0
    } else {
        intersection.len() as f32 / union.len() as f32
    }
}

#[derive(Deserialize)]
pub struct RunSkillRequest {
    pub args: Option<serde_json::Value>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 { 30 }

#[derive(Serialize)]
pub struct RunSkillResult {
    pub skill_name: String,
    pub success: bool,
    pub output: String,
    pub execution_time_ms: u64,
    pub timestamp: String,
}

/// 执行单个技能
pub async fn run_skill(
    State(gateway): State<Arc<CrabletGateway>>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(req): Json<RunSkillRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let start_time = std::time::Instant::now();
    let args = req.args.unwrap_or_else(|| serde_json::json!({}));
    
    // 检查技能是否存在且启用
    let registry = gateway.router.shared_skills.read().await;
    let skill = registry.get_skill(&name);
    
    if skill.is_none() {
        return Ok(Json(serde_json::json!({
            "status": "error",
            "error": format!("Skill '{}' not found", name)
        })));
    }
    drop(registry); // 释放读锁
    
    // 执行技能
    let registry = gateway.router.shared_skills.read().await;
    let result = registry.execute(&name, args).await;
    let execution_time = start_time.elapsed().as_millis() as u64;
    
    // 记录执行日志
    let log_entry = SkillExecutionLog {
        skill_name: name.clone(),
        timestamp: chrono::Utc::now(),
        success: result.is_ok(),
        output: result.as_ref().ok().cloned().unwrap_or_default(),
        error: result.as_ref().err().map(|e| e.to_string()),
        execution_time_ms: execution_time,
    };
    
    // 存储日志
    store_execution_log(log_entry).await;
    
    match result {
        Ok(output) => Ok(Json(serde_json::json!({
            "status": "ok",
            "result": RunSkillResult {
                skill_name: name,
                success: true,
                output,
                execution_time_ms: execution_time,
                timestamp: chrono::Utc::now().to_rfc3339(),
            }
        }))),
        Err(e) => Ok(Json(serde_json::json!({
            "status": "error",
            "error": e.to_string(),
            "result": RunSkillResult {
                skill_name: name,
                success: false,
                output: e.to_string(),
                execution_time_ms: execution_time,
                timestamp: chrono::Utc::now().to_rfc3339(),
            }
        }))),
    }
}

#[derive(Clone, Serialize)]
struct SkillExecutionLog {
    skill_name: String,
    timestamp: chrono::DateTime<chrono::Utc>,
    success: bool,
    output: String,
    error: Option<String>,
    execution_time_ms: u64,
}

// 全局执行日志存储（使用内存存储，生产环境应使用数据库）
static EXECUTION_LOGS: OnceLock<RwLock<Vec<SkillExecutionLog>>> = OnceLock::new();

async fn store_execution_log(log: SkillExecutionLog) {
    let logs = EXECUTION_LOGS.get_or_init(|| RwLock::new(Vec::new()));
    let mut logs_guard = logs.write().await;
    logs_guard.push(log);
    // 限制日志数量，保留最近 1000 条
    if logs_guard.len() > 1000 {
        logs_guard.remove(0);
    }
}

/// 获取技能执行日志
pub async fn get_skill_logs(
    State(_gateway): State<Arc<CrabletGateway>>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Query(query): Query<GetSkillLogsQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = query.limit.unwrap_or(50).min(100);
    
    let logs = EXECUTION_LOGS.get_or_init(|| RwLock::new(Vec::new()));
    let logs_guard = logs.read().await;
    
    let skill_logs: Vec<&SkillExecutionLog> = logs_guard
        .iter()
        .rev() // 最新的在前
        .filter(|log| log.skill_name == name)
        .take(limit)
        .collect();
    
    Ok(Json(serde_json::json!({
        "status": "ok",
        "skill_name": name,
        "logs": skill_logs,
        "total": skill_logs.len()
    })))
}

#[derive(Deserialize)]
pub struct GetSkillLogsQuery {
    limit: Option<usize>,
}

/// 获取所有执行日志（用于日志查看器）
pub async fn get_all_skill_logs(
    State(_gateway): State<Arc<CrabletGateway>>,
    Query(query): Query<GetSkillLogsQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let limit = query.limit.unwrap_or(100).min(200);
    
    let logs = EXECUTION_LOGS.get_or_init(|| RwLock::new(Vec::new()));
    let logs_guard = logs.read().await;
    
    let all_logs: Vec<&SkillExecutionLog> = logs_guard
        .iter()
        .rev()
        .take(limit)
        .collect();
    
    Ok(Json(serde_json::json!({
        "status": "ok",
        "logs": all_logs,
        "total": all_logs.len()
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
    let _ = gateway;
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
    let _ = gateway;
    let _ = multipart;
    let allowed = [
        "pdf", "doc", "docx", "txt", "md", "csv", "xls", "xlsx",
        "jpg", "jpeg", "png", "gif", "svg", "webp",
        "mp3", "wav", "m4a", "flac",
        "mp4", "avi", "mov", "mkv",
    ];
    let max_size: usize = 300 * 1024 * 1024;
    let _ = allowed;
    let _ = max_size;
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
    pub llm_vendor: Option<String>,
}

pub async fn get_system_config(
    State(_gateway): State<Arc<CrabletGateway>>,
) -> Result<Json<SystemConfigPayload>, StatusCode> {
    let content = fs::read_to_string(resolve_env_file_path()).unwrap_or_default();
    let mut config = SystemConfigPayload {
        openai_api_key: None,
        openai_api_base: None,
        openai_model_name: None,
        ollama_model: None,
        llm_vendor: None,
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
                "LLM_VENDOR" => config.llm_vendor = Some(val),
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
    let path = resolve_env_file_path();
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
    if let Some(v) = payload.llm_vendor {
        upsert("LLM_VENDOR", &v);
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
