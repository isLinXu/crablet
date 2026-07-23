use crate::config::Config;
use crate::error::Result;
use crate::types::{Message, TraceStep};
use async_trait::async_trait;
use std::sync::Arc;

pub mod audit_log;
pub mod classifier;
pub mod context_handler;
pub mod intent_classifier;
pub mod lane;
pub mod llm;
pub mod logical_expression;
pub mod mcts_tot;
pub mod meta_router;
pub mod middleware;
pub mod multimodal;
pub mod pattern_matcher;
pub mod planner;
pub mod react;
pub mod react_observable;
pub mod router;
pub mod routing;
pub mod streaming_pipeline;
pub mod system1;
pub mod system1_dynamic;
pub mod system1_enhanced;
pub mod system2;
pub mod system3;
pub mod system4;
pub mod thought_graph;
pub mod tot;

// Fusion Memory System integration
pub mod fusion_router;

// Meta-Cognitive System
pub mod meta_controller;

// Re-export fusion router types
pub use fusion_router::{FusionRouter, FusionRoutingContext, RouterConfig, SessionFusionRouter};

// Re-export meta-cognitive types
pub use meta_controller::{
    ExecutionRequest, ExecutionResult, MetaCognitiveController, MetaConfig, MetaStatistics,
};

// Re-export System 1 dynamic types
pub use system1_dynamic::{CommandMatch, ContextSnapshot, DynamicCommandRule, System1Dynamic};

#[async_trait]
pub trait CognitiveSystem: Send + Sync {
    /// Process the input and return a response with traces
    async fn process(&self, input: &str, context: &[Message]) -> Result<(String, Vec<TraceStep>)>;

    /// The name of this system (for logging/debugging)
    fn name(&self) -> &str;
}

pub async fn create_llm_client(config: &Config) -> Result<Arc<dyn llm::LlmClient>> {
    let configured_chain = configured_model_chain(config);
    let configured = configured_chain.first().cloned();
    let vendor = configured
        .as_ref()
        .map(|(vendor, _)| vendor.to_lowercase())
        .or_else(|| config.llm_vendor.as_deref().map(str::to_lowercase))
        .unwrap_or_else(|| "openai".to_string());
    // Keep the original Ollama setting authoritative when the provider registry
    // is not configured. Older installations often leave model_name at its
    // OpenAI default while selecting Ollama through llm_vendor.
    let model_name = configured
        .as_ref()
        .map(|(_, model)| model.as_str())
        .unwrap_or_else(|| {
            if matches!(vendor.as_str(), "ollama" | "local") {
                config.ollama_model.as_str()
            } else {
                config.model_name.as_str()
            }
        });

    if !configured_chain.is_empty() {
        let fallback_config = llm::FallbackConfig {
            primary: model_config(config, &configured_chain[0]),
            fallbacks: configured_chain
                .iter()
                .skip(1)
                .map(|candidate| model_config(config, candidate))
                .collect(),
            ..llm::FallbackConfig::default()
        };
        let fallback = llm::FallbackLlmClient::new(fallback_config)
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        return Ok(Arc::new(llm::cache::CachedLlmClient::new(
            Arc::new(fallback),
            100,
        )));
    }

    let client: Arc<dyn llm::LlmClient> = match vendor.as_str() {
        "mock" => Arc::new(llm::MockClient),
        "kimi" | "moonshot" => {
            Arc::new(llm::KimiClient::new(model_name)?) as Arc<dyn llm::LlmClient>
        }
        "zhipu" | "glm" => Arc::new(llm::ZhipuClient::new(model_name)?) as Arc<dyn llm::LlmClient>,
        "ollama" | "local" => {
            Arc::new(llm::OllamaClient::new(model_name)) as Arc<dyn llm::LlmClient>
        }
        "aliyun" | "dashscope" => {
            Arc::new(llm::OpenAiClient::new(model_name)?) as Arc<dyn llm::LlmClient>
        }
        _ => {
            if model_name.contains("mock") {
                Arc::new(llm::MockClient) as Arc<dyn llm::LlmClient>
            } else if model_name.contains("kimi") {
                Arc::new(llm::KimiClient::new(model_name)?) as Arc<dyn llm::LlmClient>
            } else if model_name.contains("glm") {
                Arc::new(llm::ZhipuClient::new(model_name)?) as Arc<dyn llm::LlmClient>
            } else if model_name.starts_with("ollama:") {
                let model = model_name.trim_start_matches("ollama:");
                Arc::new(llm::OllamaClient::new(model)) as Arc<dyn llm::LlmClient>
            } else {
                Arc::new(llm::OpenAiClient::new(model_name)?) as Arc<dyn llm::LlmClient>
            }
        }
    };

    // Wrap network-backed clients with exponential-backoff retry so transient
    // API/network failures are recovered automatically. MockClient is left bare
    // to keep test behavior deterministic.
    let resilient: Arc<dyn llm::LlmClient> = if vendor == "mock" || model_name.contains("mock") {
        client
    } else {
        Arc::new(llm::RetryLlmClient::new(client)) as Arc<dyn llm::LlmClient>
    };

    // Wrap in cache (cache sits on top, so cache hits skip retries entirely)
    let cached: Arc<dyn llm::LlmClient> =
        Arc::new(llm::cache::CachedLlmClient::new(resilient, 100)) as Arc<dyn llm::LlmClient>;
    Ok(cached)
}

/// Select a configured provider/model deterministically when provider capability
/// declarations are present. Legacy single-model configuration remains the fallback.
#[allow(dead_code)]
fn select_configured_model(config: &Config) -> Option<(String, String)> {
    configured_model_chain(config).into_iter().next()
}

/// Build the ordered provider/model chain used both for initial selection and
/// for the existing runtime fallback client.
fn configured_model_chain(config: &Config) -> Vec<(String, String)> {
    use llm::capability::{route_models, CapabilityRequirements, FallbackPolicy, ModelCandidate};

    if config.providers.is_empty() {
        return Vec::new();
    }

    let requested_vendor = config.llm_vendor.as_deref().map(str::to_lowercase);
    let mut provider_names: Vec<_> = config.providers.keys().collect();
    provider_names.sort_unstable();

    let mut candidates = Vec::new();
    let mut fallback_order = Vec::new();
    for provider_name in provider_names {
        if requested_vendor
            .as_deref()
            .is_some_and(|vendor| vendor != provider_name.to_lowercase())
        {
            continue;
        }
        let provider = &config.providers[provider_name];
        // Preserve TOML model order unless the user explicitly supplies a
        // fallback_order. The provider map itself is sorted for stability.
        let models = provider.models.clone();
        for model in models {
            let id = format!("{}/{}", provider_name, model);
            candidates.push(ModelCandidate {
                id: id.clone(),
                capabilities: provider
                    .capabilities
                    .get(&model)
                    .cloned()
                    .unwrap_or_default(),
            });
        }
        for model in &provider.fallback_order {
            fallback_order.push(format!("{}/{}", provider_name, model));
        }
    }

    if candidates.is_empty() {
        return Vec::new();
    }

    let preferred = candidates
        .iter()
        .find(|candidate| candidate.id == config.model_name)
        .or_else(|| {
            candidates.iter().find(|candidate| {
                candidate
                    .id
                    .rsplit_once('/')
                    .is_some_and(|(_, model)| model == config.model_name)
            })
        })
        .map(|candidate| candidate.id.as_str());
    let routed = match route_models(
        &candidates,
        &CapabilityRequirements::default(),
        preferred,
        &FallbackPolicy {
            order: fallback_order,
        },
    ) {
        Ok(routed) => routed,
        Err(_) => return Vec::new(),
    };
    routed
        .into_iter()
        .filter_map(|candidate| {
            let (vendor, model) = candidate.id.rsplit_once('/')?;
            Some((vendor.to_string(), model.to_string()))
        })
        .collect()
}

fn model_config(config: &Config, candidate: &(String, String)) -> llm::ModelConfig {
    let provider = config.providers.get(&candidate.0);
    let api_key = provider
        .and_then(|provider| provider.api_key_env.as_deref())
        .and_then(|env_name| std::env::var(env_name).ok());
    llm::ModelConfig {
        provider: candidate.0.clone(),
        model: candidate.1.clone(),
        api_key,
        api_base: provider.and_then(|provider| provider.base_url.clone()),
        ..llm::ModelConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::select_configured_model;
    use crate::config::{Config, ProviderConfig};
    use std::collections::HashMap;

    #[test]
    fn configured_provider_route_prefers_requested_model_then_fallback_order() {
        let mut config = Config::default();
        config.model_name = "missing-model".to_string();
        config.providers = HashMap::from([
            (
                "zhipu".to_string(),
                ProviderConfig {
                    api_key_env: None,
                    base_url: None,
                    models: vec!["glm-4".to_string(), "glm-4-air".to_string()],
                    capabilities: HashMap::new(),
                    fallback_order: vec!["glm-4-air".to_string(), "glm-4".to_string()],
                },
            ),
            (
                "openai".to_string(),
                ProviderConfig {
                    api_key_env: None,
                    base_url: None,
                    models: vec!["gpt-4o-mini".to_string()],
                    capabilities: HashMap::new(),
                    fallback_order: Vec::new(),
                },
            ),
        ]);

        assert_eq!(
            select_configured_model(&config),
            Some(("zhipu".to_string(), "glm-4-air".to_string()))
        );
    }

    #[test]
    fn configured_vendor_limits_route_candidates() {
        let mut config = Config::default();
        config.llm_vendor = Some("openai".to_string());
        config.providers = HashMap::from([
            (
                "kimi".to_string(),
                ProviderConfig {
                    api_key_env: None,
                    base_url: None,
                    models: vec!["moonshot-v1".to_string()],
                    capabilities: HashMap::new(),
                    fallback_order: Vec::new(),
                },
            ),
            (
                "openai".to_string(),
                ProviderConfig {
                    api_key_env: None,
                    base_url: None,
                    models: vec!["gpt-4o-mini".to_string()],
                    capabilities: HashMap::new(),
                    fallback_order: Vec::new(),
                },
            ),
        ]);

        assert_eq!(
            select_configured_model(&config),
            Some(("openai".to_string(), "gpt-4o-mini".to_string()))
        );
    }
}
