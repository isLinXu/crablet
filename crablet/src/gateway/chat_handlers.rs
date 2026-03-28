//! Chat-related web handlers
//!
//! Handles chat, streaming, and image generation endpoints.

use std::sync::Arc;
use axum::{
    extract::{State, Json},
    response::sse::{Event, Sse},
};
use futures::stream::StreamExt;
use futures::stream::BoxStream;
use std::collections::HashSet;
use serde::{Deserialize, Serialize};

use crate::gateway::server::CrabletGateway;
use crate::cognitive::llm::{LlmClient, OpenAiClient};
use crate::cognitive::streaming_pipeline::{
    EmptyDeltaFilterMiddleware, FinalizeSummaryMiddleware, MetricsMiddleware, StreamChunk, StreamingPipeline,
};
use crate::types::TraceStep;
use crate::types::Message;

#[cfg(feature = "knowledge")]
use crate::knowledge::graph_rag::{GraphRAG, EntityExtractorMode};

use super::handlers_shared::{
    env_value_from_file, with_identity_persona_input,
    system_prompt_markdown, infer_cognitive_layer,
};

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
    /// @deprecated API Key should only be provided via environment variables
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

#[derive(Clone)]
struct StreamRagPreparation {
    step: TraceStep,
    prompt_context: String,
}

fn llm_from_route(route: Option<&RouteSelection>) -> Option<Arc<Box<dyn LlmClient>>> {
    let base_url = route
        .and_then(|r| r.api_base_url.as_ref().map(|v| v.trim().to_string()))
        .filter(|v| !v.is_empty())
        .or_else(|| env_value_from_file("OPENAI_API_BASE"))?;

    // API Key 仅通过环境变量传递
    let api_key = env_value_from_file("DASHSCOPE_API_KEY")
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

async fn prepare_stream_rag(gateway: &Arc<CrabletGateway>, input: &str) -> Option<StreamRagPreparation> {
    let mut rag_context = String::new();
    let refs: Vec<serde_json::Value> = Vec::new();
    let mut graph_entities: Vec<String> = Vec::new();
    let retrieval = "none".to_string();

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
) -> Sse<BoxStream<'static, Result<Event, axum::Error>>> {
    let session_id = payload.session_id.clone().unwrap_or_else(|| "default".to_string());
    let session_id_for_event = session_id.clone();
    let message = payload.message.clone();

    tracing::info!("[chat_stream] 收到消息，长度: {} 字符", message.len());
    if message.contains("[文件内容]") {
        let file_content_start = message.find("[文件内容]").unwrap_or(0);
        let preview = &message[file_content_start..message.len().min(file_content_start + 200)];
        tracing::info!("[chat_stream] 检测到文件内容: {}", preview);
    }
    if message.contains("[知识检索上下文]") {
        tracing::info!("[chat_stream] 检测到知识检索上下文");
    }

    // Try CognitiveRouter first (supports System 1 fast response)
    let router_result = gateway.router.process(&message, &session_id).await;

    // If System 1 succeeded, return fast response
    if let Ok((response, traces)) = &router_result {
        let cognitive_layer = infer_cognitive_layer(response, traces);
        if cognitive_layer == "system1" {
            tracing::info!("[chat_stream] System 1 快速响应: {}", response.chars().take(50).collect::<String>());
            let response = response.clone();
            let traces = traces.clone();
            let session_id_for_stream = session_id_for_event.clone();
            let source_stream = async_stream::stream! {
                yield StreamChunk {
                    chunk_type: "cognitive_layer".to_string(),
                    content: None,
                    payload: Some(serde_json::json!({ "layer": "system1" })),
                };
                for (i, step) in traces.iter().enumerate() {
                    yield StreamChunk {
                        chunk_type: "trace".to_string(),
                        content: None,
                        payload: Some(serde_json::json!({ "step": step, "index": i })),
                    };
                }
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

    // System 1 not matched or returned error, fallback to LLM streaming
    let cognitive_layer = crate::gateway::handlers_shared::infer_cognitive_layer_from_input(&message);
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
        yield StreamChunk {
            chunk_type: "cognitive_layer".to_string(),
            content: None,
            payload: Some(serde_json::json!({ "layer": cognitive_layer_for_stream })),
        };

        let mut messages = Vec::new();
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