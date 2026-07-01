use super::LlmClient;
use crate::types::{ChatChunk, Message};
use anyhow::Result;
use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

/// Configuration for [`RetryLlmClient`] exponential backoff.
#[derive(Clone, Debug)]
pub struct RetryConfig {
    /// Maximum number of *retries* after the initial attempt.
    pub max_retries: u32,
    /// Base delay used for the first backoff window.
    pub base_delay: Duration,
    /// Upper bound for any single backoff window.
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(8),
        }
    }
}

/// Classify whether an LLM error is worth retrying.
///
/// Transient failures (timeouts, connection resets, 5xx, 429 rate limits) are
/// retryable. Deterministic client errors (401/403/400/404/422, invalid API
/// keys) are NOT — retrying only wastes backoff time and floods the logs, so we
/// fail fast and surface the real problem to the caller.
fn is_retryable_error(err: &anyhow::Error) -> bool {
    // Inspect the full error chain, lowercased, for non-retryable markers.
    let chain = err
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();

    const NON_RETRYABLE: &[&str] = &[
        "unauthorized",       // 401
        "forbidden",          // 403
        "bad request",        // 400
        "not found",          // 404
        "unprocessable",      // 422
        "invalid api key",
        "incorrect api key",
        "invalid_api_key",
        "authentication",
        "permission denied",
    ];

    !NON_RETRYABLE.iter().any(|marker| chain.contains(marker))
}

impl RetryConfig {
    /// Backoff window for a given (zero-based) retry attempt: `base * 2^attempt`,
    /// capped at `max_delay`. A small deterministic jitter is added to avoid
    /// thundering-herd alignment across concurrent callers.
    fn delay_for(&self, attempt: u32) -> Duration {
        let factor = 1u64 << attempt.min(16);
        let base_ms = self.base_delay.as_millis() as u64;
        let raw = base_ms.saturating_mul(factor);
        let capped = raw.min(self.max_delay.as_millis() as u64);
        // Jitter in [0, base_ms/2) derived from attempt to stay dependency-free.
        let jitter = (base_ms / 2).saturating_mul(attempt as u64 % 3) / 3;
        Duration::from_millis(capped.saturating_add(jitter))
    }
}

/// A transparent decorator that retries transient LLM failures with exponential
/// backoff. Streaming requests are delegated without retry because partial
/// streams cannot be safely replayed.
pub struct RetryLlmClient {
    inner: Arc<dyn LlmClient>,
    config: RetryConfig,
}

impl RetryLlmClient {
    pub fn new(inner: Arc<dyn LlmClient>) -> Self {
        Self {
            inner,
            config: RetryConfig::default(),
        }
    }

    pub fn with_config(inner: Arc<dyn LlmClient>, config: RetryConfig) -> Self {
        Self { inner, config }
    }

    /// Returns true if another attempt should be made for `err` at this
    /// `attempt` index, sleeping for the backoff window first when so.
    async fn should_retry(&self, attempt: u32, err: &anyhow::Error) -> bool {
        if attempt >= self.config.max_retries {
            return false;
        }
        if !is_retryable_error(err) {
            warn!("LLM call failed with non-retryable error: {}. Aborting.", err);
            return false;
        }
        let delay = self.config.delay_for(attempt);
        warn!(
            "LLM call failed (attempt {}/{}): {}. Retrying in {:?}.",
            attempt + 1,
            self.config.max_retries + 1,
            err,
            delay
        );
        tokio::time::sleep(delay).await;
        true
    }
}

#[async_trait]
impl LlmClient for RetryLlmClient {
    async fn chat_complete(&self, messages: &[Message]) -> Result<String> {
        let mut attempt = 0;
        loop {
            match self.inner.chat_complete(messages).await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    if self.should_retry(attempt, &e).await {
                        attempt += 1;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    async fn chat_complete_with_tools(
        &self,
        messages: &[Message],
        tools: &[serde_json::Value],
    ) -> Result<Message> {
        let mut attempt = 0;
        loop {
            match self.inner.chat_complete_with_tools(messages, tools).await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    if self.should_retry(attempt, &e).await {
                        attempt += 1;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    async fn chat_complete_with_reasoning(&self, messages: &[Message]) -> Result<(String, String)> {
        let mut attempt = 0;
        loop {
            match self.inner.chat_complete_with_reasoning(messages).await {
                Ok(v) => return Ok(v),
                Err(e) => {
                    if self.should_retry(attempt, &e).await {
                        attempt += 1;
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk>> + Send>>> {
        // Streaming responses cannot be replayed safely; delegate directly.
        self.inner.chat_stream(messages).await
    }

    fn model_name(&self) -> &str {
        self.inner.model_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct FlakyClient {
        fail_times: usize,
        calls: Arc<AtomicUsize>,
        error_message: String,
    }

    #[async_trait]
    impl LlmClient for FlakyClient {
        async fn chat_complete(&self, _messages: &[Message]) -> Result<String> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if n < self.fail_times {
                Err(anyhow::anyhow!("{} {}", self.error_message, n))
            } else {
                Ok("ok".to_string())
            }
        }

        async fn chat_complete_with_tools(
            &self,
            messages: &[Message],
            _tools: &[serde_json::Value],
        ) -> Result<Message> {
            let text = self.chat_complete(messages).await?;
            Ok(Message::new("assistant", &text))
        }

        fn model_name(&self) -> &str {
            "flaky"
        }
    }

    #[tokio::test]
    async fn retries_until_success() {
        let calls = Arc::new(AtomicUsize::new(0));
        let inner: Arc<dyn LlmClient> = Arc::new(FlakyClient {
            fail_times: 2,
            calls: calls.clone(),
            error_message: "transient failure".to_string(),
        });
        let client = RetryLlmClient::with_config(
            inner,
            RetryConfig {
                max_retries: 3,
                base_delay: Duration::from_millis(1),
                max_delay: Duration::from_millis(5),
            },
        );

        let result = client.chat_complete(&[Message::new("user", "hi")]).await;
        assert_eq!(result.unwrap(), "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 3); // 2 failures + 1 success
    }

    #[tokio::test]
    async fn gives_up_after_max_retries() {
        let calls = Arc::new(AtomicUsize::new(0));
        let inner: Arc<dyn LlmClient> = Arc::new(FlakyClient {
            fail_times: 100,
            calls: calls.clone(),
            error_message: "transient failure".to_string(),
        });
        let client = RetryLlmClient::with_config(
            inner,
            RetryConfig {
                max_retries: 2,
                base_delay: Duration::from_millis(1),
                max_delay: Duration::from_millis(5),
            },
        );

        let result = client.chat_complete(&[Message::new("user", "hi")]).await;
        assert!(result.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 3); // initial + 2 retries
    }

    #[tokio::test]
    async fn does_not_retry_non_retryable_errors() {
        let calls = Arc::new(AtomicUsize::new(0));
        let inner: Arc<dyn LlmClient> = Arc::new(FlakyClient {
            fail_times: 100,
            calls: calls.clone(),
            // Simulates "OpenAI API returned error: 401 Unauthorized"
            error_message: "OpenAI API returned error: 401 Unauthorized".to_string(),
        });
        let client = RetryLlmClient::with_config(
            inner,
            RetryConfig {
                max_retries: 5,
                base_delay: Duration::from_millis(1),
                max_delay: Duration::from_millis(5),
            },
        );

        let result = client.chat_complete(&[Message::new("user", "hi")]).await;
        assert!(result.is_err());
        // Should fail fast on auth error: exactly one attempt, no retries.
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn classifies_errors_correctly() {
        assert!(is_retryable_error(&anyhow::anyhow!("connection timed out")));
        assert!(is_retryable_error(&anyhow::anyhow!(
            "OpenAI API returned error: 503 Service Unavailable"
        )));
        assert!(is_retryable_error(&anyhow::anyhow!(
            "OpenAI API returned error: 429 Too Many Requests"
        )));
        assert!(!is_retryable_error(&anyhow::anyhow!(
            "OpenAI API returned error: 401 Unauthorized"
        )));
        assert!(!is_retryable_error(&anyhow::anyhow!(
            "OpenAI API returned error: 400 Bad Request"
        )));
        assert!(!is_retryable_error(&anyhow::anyhow!("invalid api key")));
    }
}
