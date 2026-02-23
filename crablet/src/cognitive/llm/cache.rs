use anyhow::Result;
use async_trait::async_trait;
use lru::LruCache;
use std::sync::{Arc, Mutex};
use std::num::NonZeroUsize;
use crate::types::Message;
use super::LlmClient;
use tracing::info;

pub struct CachedLlmClient {
    inner: Box<dyn LlmClient>,
    cache: Arc<Mutex<LruCache<String, String>>>,
}

impl CachedLlmClient {
    pub fn new(inner: Box<dyn LlmClient>, capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(100).unwrap());
        Self {
            inner,
            cache: Arc::new(Mutex::new(LruCache::new(cap))),
        }
    }
}

#[async_trait]
impl LlmClient for CachedLlmClient {
    async fn chat_complete(&self, messages: &[Message]) -> Result<String> {
        // Simple cache key based on debug representation of messages
        // In production, a stable hash would be better
        let cache_key = format!("{:?}", messages);
        
        {
            let mut cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.get(&cache_key) {
                info!("LLM Cache Hit!");
                return Ok(cached.clone());
            }
        }
        
        let response = self.inner.chat_complete(messages).await?;
        
        {
            let mut cache = self.cache.lock().unwrap();
            cache.put(cache_key, response.clone());
        }
        
        Ok(response)
    }

    async fn chat_complete_with_tools(&self, messages: &[Message], tools: &[serde_json::Value]) -> Result<Message> {
        // We don't cache tool calls for now as they are dynamic and side-effect prone
        self.inner.chat_complete_with_tools(messages, tools).await
    }
}
