use super::LlmClient;
use crate::types::Message;
use anyhow::Result;
use async_trait::async_trait;
use lru::LruCache;
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use std::num::NonZeroUsize;
use std::sync::Arc;
use tracing::info;

pub struct CachedLlmClient {
    inner: Box<dyn LlmClient>,
    cache: Arc<Mutex<LruCache<String, String>>>,
}

impl CachedLlmClient {
    pub fn new(inner: Box<dyn LlmClient>, capacity: usize) -> Self {
        let normalized_capacity = if capacity == 0 { 100 } else { capacity };
        let cap = match NonZeroUsize::new(normalized_capacity) {
            Some(capacity) => capacity,
            None => NonZeroUsize::MIN,
        };
        Self {
            inner,
            cache: Arc::new(Mutex::new(LruCache::new(cap))),
        }
    }
}

#[async_trait]
impl LlmClient for CachedLlmClient {
    async fn chat_complete(&self, messages: &[Message]) -> Result<String> {
        // Use SHA256 for stable cache key
        let mut hasher = Sha256::new();
        let json = serde_json::to_vec(messages).unwrap_or_default();
        hasher.update(&json);
        // Include model name in cache key to avoid collisions between models
        hasher.update(self.inner.model_name().as_bytes());
        let result = hasher.finalize();
        let cache_key = data_encoding::HEXLOWER.encode(&result);

        {
            let mut cache = self.cache.lock();
            if let Some(cached) = cache.get(&cache_key) {
                info!("LLM Cache Hit!");
                return Ok(cached.clone());
            }
        }

        let response = self.inner.chat_complete(messages).await?;

        {
            let mut cache = self.cache.lock();
            cache.put(cache_key, response.clone());
        }

        Ok(response)
    }

    async fn chat_complete_with_tools(
        &self,
        messages: &[Message],
        tools: &[serde_json::Value],
    ) -> Result<Message> {
        // We don't cache tool calls for now as they are dynamic and side-effect prone
        self.inner.chat_complete_with_tools(messages, tools).await
    }

    fn model_name(&self) -> &str {
        self.inner.model_name()
    }
}
