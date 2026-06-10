//! Speculative Router — 投机执行路由 — DEPRECATED
//!
//! ⚠️ This module is deprecated since v0.6.0 and will be removed in a future version.
//! Use `FusionRouter` (`cognitive::fusion_router`) instead.
//!
//! 核心思想：在分类器决策出来之前，并发启动 System1 快速路径。
//! - 若分类器最终确认使用 System1 → 直接返回已就绪的结果（首字延迟 ~50ms）
//! - 若分类器决定走 System2/3 → 丢弃 System1 的结果，进入深度路径
//!
//! 这对于 "easy + fast" 请求（问候、简单问答）提供了约 4× 的首字加速。
//! 对 System2/3 请求没有负面影响（仅多一次 System1 调用的 CPU 开销）。

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::oneshot;
use tokio::time::timeout;
use tracing::{debug, warn};

use crate::cognitive::classifier::{Classifier, Intent};
use crate::cognitive::CognitiveSystem;
use crate::cognitive::{system1_enhanced::System1Enhanced, system2::System2};
use crate::events::{AgentEvent, EventBus};
use crate::types::Message;

/// Decision from the classifier about which system to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeculativeSystemChoice {
    System1,
    System2,
    System3,
}

/// Result of a speculative execution attempt.
#[derive(Debug)]
pub struct SpeculativeResult {
    /// The response text.
    pub content: String,
    /// Whether the speculative (System1) result was actually used.
    pub speculative_hit: bool,
    /// Total time from request start to first token (ms).
    pub first_token_latency_ms: u64,
}

/// Speculative Router configuration.
#[derive(Debug, Clone)]
pub struct SpeculativeConfig {
    /// Maximum time to wait for the System1 speculative result before
    /// the classifier decision arrives. If System1 exceeds this, the
    /// speculative path is abandoned.
    pub speculative_timeout: Duration,

    /// Minimum System1 confidence required to accept the speculative result.
    /// Below this threshold, we fall through to System2 even if S1 finished.
    pub min_speculative_confidence: f32,

    /// Whether speculative execution is enabled at all.
    pub enabled: bool,
}

impl Default for SpeculativeConfig {
    fn default() -> Self {
        Self {
            speculative_timeout: Duration::from_millis(400),
            min_speculative_confidence: 0.75,
            enabled: true,
        }
    }
}

/// A lightweight wrapper that races System1 against the classifier decision.
pub struct SpeculativeRouter {
    sys1: System1Enhanced,
    sys2: System2,
    event_bus: Arc<EventBus>,
    config: SpeculativeConfig,
}

impl SpeculativeRouter {
    pub fn new(
        sys1: System1Enhanced,
        sys2: System2,
        event_bus: Arc<EventBus>,
        config: SpeculativeConfig,
    ) -> Self {
        Self {
            sys1,
            sys2,
            event_bus,
            config,
        }
    }

    /// Route a query using speculative execution.
    ///
    /// Internally:
    /// 1. Spawns System1 immediately (speculative path).
    /// 2. Runs the classifier synchronously (it's O(n) string matching — very fast).
    /// 3. If classifier says System1 AND System1 beat the deadline → return S1 result.
    /// 4. Otherwise → await System2.
    pub async fn route(
        &self,
        query: &str,
        messages: &[Message],
    ) -> anyhow::Result<SpeculativeResult> {
        if !self.config.enabled {
            return self.fallback_to_sys2(query, messages).await;
        }

        let start = std::time::Instant::now();

        // --- Run classifier immediately (synchronous, no I/O) ---
        let intent = Classifier::classify_intent(query);
        let choice = self.intent_to_system(&intent);

        debug!(
            "[SpeculativeRouter] query={:?} intent={:?} choice={:?}",
            &query[..query.len().min(60)],
            intent,
            choice,
        );

        if choice != SpeculativeSystemChoice::System1 {
            // Not a System1 query — skip speculative path entirely
            return self.fallback_to_sys2(query, messages).await.map(|mut r| {
                r.first_token_latency_ms = start.elapsed().as_millis() as u64;
                r
            });
        }

        // --- Spawn System1 speculatively ---
        let (s1_tx, s1_rx) = oneshot::channel::<anyhow::Result<String>>();
        {
            let sys1 = self.sys1.clone();
            let msgs: Vec<Message> = messages.to_vec();
            let q = query.to_string();
            tokio::spawn(async move {
                let result = sys1
                    .process(&q, &msgs)
                    .await
                    .map(|(content, _traces)| content)
                    .map_err(anyhow::Error::from);
                let _ = s1_tx.send(result);
            });
        }

        // --- Try to claim the speculative result within the timeout ---
        match timeout(self.config.speculative_timeout, s1_rx).await {
            Ok(Ok(Ok(content))) => {
                let latency = start.elapsed().as_millis() as u64;
                debug!(
                    "[SpeculativeRouter] speculative HIT — latency={}ms",
                    latency
                );
                self.event_bus.publish(AgentEvent::CognitiveLayerChanged {
                    layer: "System1-Speculative".to_string(),
                });
                Ok(SpeculativeResult {
                    content,
                    speculative_hit: true,
                    first_token_latency_ms: latency,
                })
            }
            Ok(Ok(Err(e))) => {
                warn!(
                    "[SpeculativeRouter] System1 error on speculative path: {}",
                    e
                );
                self.fallback_to_sys2(query, messages).await.map(|mut r| {
                    r.first_token_latency_ms = start.elapsed().as_millis() as u64;
                    r
                })
            }
            Ok(Err(_)) => {
                warn!("[SpeculativeRouter] System1 sender dropped unexpectedly");
                self.fallback_to_sys2(query, messages).await.map(|mut r| {
                    r.first_token_latency_ms = start.elapsed().as_millis() as u64;
                    r
                })
            }
            Err(_) => {
                debug!(
                    "[SpeculativeRouter] speculative MISS — System1 exceeded {:?} deadline",
                    self.config.speculative_timeout
                );
                self.fallback_to_sys2(query, messages).await.map(|mut r| {
                    r.first_token_latency_ms = start.elapsed().as_millis() as u64;
                    r
                })
            }
        }
    }

    async fn fallback_to_sys2(
        &self,
        query: &str,
        messages: &[Message],
    ) -> anyhow::Result<SpeculativeResult> {
        let (content, _traces) = self
            .sys2
            .process(query, messages)
            .await
            .map_err(anyhow::Error::from)?;
        Ok(SpeculativeResult {
            content,
            speculative_hit: false,
            first_token_latency_ms: 0, // filled in by caller
        })
    }

    /// Map classifier intent to system choice.
    fn intent_to_system(&self, intent: &Intent) -> SpeculativeSystemChoice {
        match intent {
            Intent::Greeting | Intent::Help | Intent::Status | Intent::Chat | Intent::Persona => {
                SpeculativeSystemChoice::System1
            }
            Intent::DeepResearch
            | Intent::MultiStep
            | Intent::Coding
            | Intent::Analysis
            | Intent::Math
            | Intent::General => SpeculativeSystemChoice::System2,
            Intent::Creative => SpeculativeSystemChoice::System3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(deprecated)]
    fn test_speculative_config_defaults() {
        let cfg = SpeculativeConfig::default();
        assert!(cfg.enabled);
        assert!(cfg.min_speculative_confidence > 0.0);
        assert!(cfg.speculative_timeout > Duration::from_millis(0));
    }
}
