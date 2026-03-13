use async_trait::async_trait;
use anyhow::Result;
use crate::types::{Message, TraceStep};
use super::{CognitiveMiddleware, MiddlewareState, MiddlewarePipeline, RagTraceItem, RagTracePayload};
use tracing::info;
#[cfg(feature = "knowledge")]
use std::sync::Arc;
#[cfg(feature = "knowledge")]
use crate::knowledge::graph_rag::GraphRAG;

pub struct RagMiddleware;

#[async_trait]
impl CognitiveMiddleware for RagMiddleware {
    fn name(&self) -> &str {
        "RAG Retrieval"
    }

    async fn execute(&self, input: &str, context: &mut Vec<Message>, state: &MiddlewareState) -> Result<Option<(String, Vec<TraceStep>)>> {
        let mut rag_context = String::new();
        #[allow(unused_mut)]
        let mut retrieval = "none".to_string();
        #[allow(unused_mut)]
        let mut refs: Vec<RagTraceItem> = Vec::new();
        let mut graph_entities: Vec<String> = Vec::new();
        
        // Graph Retrieval
        if let Some(kg) = &state.kg {
            // Extract keywords (top 10 unique words > 3 chars)
            let keywords: Vec<String> = input.split_whitespace()
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
                .filter(|w| w.len() > 3)
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .take(10)
                .collect();
                
            if !keywords.is_empty() {
                // Batch query
                if let Ok(entities) = kg.find_entities_batch(&keywords).await {
                    for (name, _type) in entities {
                        let relations_result: Result<Vec<(String, String, String)>> = kg.find_related(&name).await;
                        if let Ok(relations) = relations_result {
                            if !relations.is_empty() {
                                graph_entities.push(name.clone());
                                rag_context.push_str(&format!("\n[Knowledge Graph about '{}']:\n", name));
                                for (dir, rel, target) in relations.iter().take(5) {
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
        if let Some(vs) = &state.vector_store {
            let mut contexts_added = false;
            if let Some(kg) = &state.kg {
                let graph_rag = GraphRAG::new_with_mode(
                    Arc::clone(vs),
                    Arc::clone(kg),
                    state.graph_rag_entity_mode,
                );
                if let Ok(results) = graph_rag.retrieve(input, 3).await {
                    if !results.is_empty() {
                        retrieval = "graph_rag".to_string();
                        rag_context.push_str("\n[GraphRAG Results]:\n");
                        for (i, item) in results.iter().enumerate() {
                            refs.push(RagTraceItem {
                                source: item.source.clone(),
                                score: item.score,
                                content: item.content.chars().take(280).collect(),
                            });
                            rag_context.push_str(&format!("- [Ref {}] [{} {:.2}] {}\n", i + 1, item.source, item.score, item.content));
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
                        for (i, (content, score, _metadata)) in results.iter().enumerate() {
                            let source = _metadata.get("source").and_then(|v| v.as_str()).unwrap_or("vector_store");
                            refs.push(RagTraceItem {
                                source: source.to_string(),
                                score: *score,
                                content: content.chars().take(280).collect(),
                            });
                            rag_context.push_str(&format!("- [Ref {}] (score: {:.2}) {}\n", i + 1, score, content));
                        }
                    }
                }
            }
        }
        
        if !rag_context.is_empty() {
            info!("Injecting RAG context");
            let msg = format!("\n[KNOWLEDGE]\nUse the following retrieved knowledge to answer the user's question if relevant.\nImportant: When using information from the context, cite the source using [Ref X] notation or mention the Knowledge Graph.\n{}\n", rag_context);
            MiddlewarePipeline::ensure_system_prompt(context, &msg);
        }
        let mut rag_trace = state.rag_trace.write().await;
        *rag_trace = Some(RagTracePayload {
            retrieval,
            refs,
            graph_entities,
        });
        
        Ok(None)
    }
}
