use async_trait::async_trait;
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StreamChunk {
    pub chunk_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

impl StreamChunk {
    pub fn delta(content: impl Into<String>) -> Self {
        Self {
            chunk_type: "delta".to_string(),
            content: Some(content.into()),
            payload: None,
        }
    }

    pub fn trace(payload: serde_json::Value) -> Self {
        Self {
            chunk_type: "trace".to_string(),
            content: None,
            payload: Some(payload),
        }
    }

    pub fn done(payload: serde_json::Value) -> Self {
        Self {
            chunk_type: "done".to_string(),
            content: None,
            payload: Some(payload),
        }
    }
}

#[derive(Clone, Debug)]
pub struct StreamContext {
    pub started_at: Instant,
    pub chunk_count: usize,
    pub total_chars: usize,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Default for StreamContext {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamContext {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            chunk_count: 0,
            total_chars: 0,
            metadata: HashMap::new(),
        }
    }
}

#[async_trait]
pub trait StreamingMiddleware: Send + Sync {
    async fn process_chunk(&self, chunk: StreamChunk, ctx: &mut StreamContext) -> Option<StreamChunk>;
    async fn finalize(&self, _ctx: &mut StreamContext) -> Option<Vec<StreamChunk>> {
        None
    }
}

pub struct StreamingPipeline {
    middlewares: Vec<Arc<dyn StreamingMiddleware>>,
}

impl StreamingPipeline {
    pub fn new(middlewares: Vec<Arc<dyn StreamingMiddleware>>) -> Self {
        Self { middlewares }
    }

    pub fn process<S>(&self, input_stream: S) -> impl Stream<Item = StreamChunk>
    where
        S: Stream<Item = StreamChunk> + Send + 'static,
    {
        let middlewares = self.middlewares.clone();
        async_stream::stream! {
            let mut ctx = StreamContext::new();
            futures::pin_mut!(input_stream);
            while let Some(chunk) = input_stream.next().await {
                let mut current = Some(chunk);
                for mw in &middlewares {
                    if let Some(c) = current {
                        current = mw.process_chunk(c, &mut ctx).await;
                    } else {
                        break;
                    }
                }
                if let Some(c) = current {
                    yield c;
                }
            }
            for mw in &middlewares {
                if let Some(chunks) = mw.finalize(&mut ctx).await {
                    for c in chunks {
                        yield c;
                    }
                }
            }
        }
    }
}

pub struct MetricsMiddleware;

#[async_trait]
impl StreamingMiddleware for MetricsMiddleware {
    async fn process_chunk(&self, chunk: StreamChunk, ctx: &mut StreamContext) -> Option<StreamChunk> {
        ctx.chunk_count += 1;
        if let Some(c) = &chunk.content {
            ctx.total_chars += c.chars().count();
        }
        Some(chunk)
    }
}

pub struct EmptyDeltaFilterMiddleware;

#[async_trait]
impl StreamingMiddleware for EmptyDeltaFilterMiddleware {
    async fn process_chunk(&self, chunk: StreamChunk, _ctx: &mut StreamContext) -> Option<StreamChunk> {
        if chunk.chunk_type == "delta" && chunk.content.as_deref().unwrap_or("").is_empty() {
            return None;
        }
        Some(chunk)
    }
}

pub struct FinalizeSummaryMiddleware;

#[async_trait]
impl StreamingMiddleware for FinalizeSummaryMiddleware {
    async fn process_chunk(&self, chunk: StreamChunk, _ctx: &mut StreamContext) -> Option<StreamChunk> {
        Some(chunk)
    }

    async fn finalize(&self, ctx: &mut StreamContext) -> Option<Vec<StreamChunk>> {
        let elapsed_ms = ctx.started_at.elapsed().as_millis() as u64;
        Some(vec![StreamChunk::done(serde_json::json!({
            "elapsed_ms": elapsed_ms,
            "chunk_count": ctx.chunk_count,
            "total_chars": ctx.total_chars
        }))])
    }
}
