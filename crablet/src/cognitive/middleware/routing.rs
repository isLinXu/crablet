use async_trait::async_trait;
use anyhow::Result;
use crate::types::{Message, TraceStep};
use super::{CognitiveMiddleware, MiddlewareState, MiddlewarePipeline};
#[cfg(feature = "knowledge")]
use tracing::info;
use crate::cognitive::classifier::{Classifier, Intent};
#[cfg(feature = "knowledge")]
use tokio::sync::OnceCell;
#[cfg(feature = "knowledge")]
use crate::knowledge::vector_store::VectorStore;
use std::sync::Arc;
use crate::cognitive::llm::LlmClient;

pub struct RoutingMiddleware {
    // classifier: IntentClassifier, // Removed old LLM classifier for now, rely on heuristic or future implementation
    #[cfg(feature = "knowledge")]
    prototypes: OnceCell<Vec<(Intent, Vec<f32>)>>,
}

impl RoutingMiddleware {
    pub fn new(_llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self {
            // classifier: IntentClassifier::new(llm),
            #[cfg(feature = "knowledge")]
            prototypes: OnceCell::new(),
        }
    }

    #[cfg(feature = "knowledge")]
    async fn get_prototypes(&self, vs: &VectorStore) -> Option<&Vec<(Intent, Vec<f32>)>> {
        self.prototypes.get_or_try_init(|| async {
            let mut protos = Vec::new();
            // Define prototypes
            let seeds = vec![
                (Intent::Greeting, "hello hi greetings good morning how are you who are you"),
                (Intent::DeepResearch, "research find information search web deep dive look up news"),
                (Intent::Coding, "write code python rust function script programming implement fix bug"),
                // (Intent::Reasoning, "plan analyze think step by step reason logic solve problem strategy"), // Replaced by General/Analysis
                (Intent::Analysis, "plan analyze think step by step reason logic solve problem strategy"),
            ];

            for (intent, text) in seeds {
                if let Ok(emb) = vs.embed_query(text).await {
                    protos.push((intent, emb));
                }
            }
            if protos.is_empty() {
                Err(anyhow::anyhow!("Failed to embed prototypes"))
            } else {
                Ok(protos)
            }
        }).await.ok()
    }
}

#[async_trait]
impl CognitiveMiddleware for RoutingMiddleware {
    fn name(&self) -> &str {
        "Intent Routing"
    }

    async fn execute(&self, input: &str, context: &mut Vec<Message>, state: &MiddlewareState) -> Result<Option<(String, Vec<TraceStep>)>> {
        let mut intent = Intent::General; // Default
        
        // 1. Try Semantic Classification (Zero-Shot) if VectorStore is available
        #[cfg(feature = "knowledge")]
        if let Some(vs) = &state.vector_store {
            if let Some(protos) = self.get_prototypes(vs).await {
                if let Ok(query_emb) = vs.embed_query(input).await {
                    let mut max_score = -1.0;
                    let mut best_intent = Intent::General;
                    
                    for (proto_intent, proto_emb) in protos {
                        let score = cosine_similarity(&query_emb, proto_emb);
                        if score > max_score {
                            max_score = score;
                            best_intent = proto_intent.clone();
                        }
                    }
                    
                    if max_score > 0.82 { // High confidence threshold
                        info!("Semantic Router: Matched {:?} with score {:.2}", best_intent, max_score);
                        intent = best_intent;
                    }
                }
            }
        }

        // 2. Fallback to Heuristic Classifier if Semantic failed or not available
        if matches!(intent, Intent::General) {
             intent = Classifier::classify_intent(input);
        }

        match intent {
            Intent::Greeting => {
                // Inject system prompt for concise, friendly response
                MiddlewarePipeline::ensure_system_prompt(context, "You are Crablet, a helpful AI assistant. The user is engaging in small talk. Be friendly, concise, and helpful. Do not use tools unless explicitly asked.");
            },
            Intent::DeepResearch => {
                MiddlewarePipeline::ensure_system_prompt(context, "You are Crablet. The user wants deep research. Use available search tools extensively to provide a comprehensive answer.");
            },
            Intent::Coding => {
                MiddlewarePipeline::ensure_system_prompt(context, "You are Crablet. The user is asking for code. Provide clean, well-commented code blocks. Use the file tool if you need to read existing files.");
            },
            Intent::Creative => {
                MiddlewarePipeline::ensure_system_prompt(context, "You are Crablet. The user wants creative content. Be imaginative and engaging.");
            },
            Intent::Math => {
                MiddlewarePipeline::ensure_system_prompt(context, "You are Crablet. The user has a mathematical query. Be precise and show your work step-by-step.");
            },
            Intent::Analysis | Intent::MultiStep | Intent::General | Intent::Status | Intent::Help => {
                // Default behavior or specific prompts if needed
                MiddlewarePipeline::ensure_system_prompt(context, "You are Crablet. Use your reasoning capabilities to solve the user's problem step-by-step.");
            },
        }
        
        Ok(None)
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { return 0.0; }
    dot_product / (norm_a * norm_b)
}
