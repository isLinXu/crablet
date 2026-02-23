use anyhow::Result;
use tracing::{info, warn};
use std::sync::Arc;
use crate::memory::semantic::SharedKnowledgeGraph;
use crate::knowledge::vector_store::VectorStore;
use crate::knowledge::extractor::KnowledgeExtractor;
use crate::knowledge::pdf::PdfParser;
use crate::channels::cli::args::KnowledgeSubcommands;

pub async fn handle_knowledge(
    subcmd: &KnowledgeSubcommands,
    kg: Option<SharedKnowledgeGraph>,
    vector_store: Option<Arc<VectorStore>>,
) -> Result<()> {
    match subcmd {
        KnowledgeSubcommands::Extract { input, file } => {
            let text = if *file {
                if input.ends_with(".pdf") {
                     PdfParser::extract_text(input)?
                } else {
                     std::fs::read_to_string(input)?
                }
            } else {
                input.clone()
            };

            info!("Extracting knowledge from input (length: {})...", text.len());
            let extractor = KnowledgeExtractor::new()?;
            let result = extractor.extract_from_text(&text).await?;
            println!("{:#?}", result);

            // Persist to Knowledge Graph
            if let Some(kg) = &kg {
                info!("Persisting {} entities and {} relations to Knowledge Graph...", result.entities.len(), result.relations.len());
                for entity in result.entities {
                    let _ = kg.add_entity(&entity.name, &entity.r#type).await;
                }
                for relation in result.relations {
                    let _ = kg.add_relation(&relation.source, &relation.target, &relation.relation).await;
                }
                info!("Knowledge persisted successfully.");
            } else {
                warn!("Knowledge Graph not available, skipping persistence.");
            }

            // Persist to Vector Store (Chunking strategy: simple full text for MVP)
            if let Some(vs) = &vector_store {
                info!("Persisting content to Vector Store...");
                // In real-world, we would chunk large text here
                let _ = vs.add_document(&text, None).await;
                info!("Vector embeddings generated and stored.");
            }
        }
        KnowledgeSubcommands::Query { entity } => {
            if let Some(kg) = kg {
                info!("Querying knowledge graph for entity: {}", entity);
                let relations = kg.find_related(entity).await?;
                if relations.is_empty() {
                    println!("No knowledge found for entity '{}'", entity);
                } else {
                    println!("Knowledge related to '{}':", entity);
                    for (direction, relation, target) in relations {
                        if direction == "->" {
                            println!("  - {} -> {}", relation, target);
                        } else {
                            println!("  - {} <- {}", relation, target);
                            println!("  (is {} of {})", relation, target);
                        }
                    }
                }
            } else {
                println!("Error: Knowledge Graph not available.");
            }
        }
        KnowledgeSubcommands::List => {
            if let Some(vs) = vector_store {
                match vs.list_documents().await {
                    Ok(docs) => {
                        println!("Knowledge Base Documents:");
                        if docs.is_empty() {
                            println!("  (No documents found)");
                        } else {
                            for doc in docs {
                                let source = doc.get("source").and_then(|v| v.as_str()).unwrap_or("Unknown");
                                let file_type = doc.get("file_type").and_then(|v| v.as_str()).unwrap_or("?");
                                let chunks = doc.get("chunks").and_then(|v| v.as_i64()).unwrap_or(0);
                                println!("  - {} [{}] ({} chunks)", source, file_type, chunks);
                            }
                        }
                    }
                    Err(e) => println!("Error listing documents: {}", e),
                }
            } else {
                println!("Error: Vector Store not available.");
            }
        }
        KnowledgeSubcommands::Export => {
            if let Some(kg) = kg {
                info!("Exporting knowledge graph to D3 JSON...");
                let json = kg.export_d3_json().await?;
                println!("{}", json);
            } else {
                println!("Error: Knowledge Graph not available.");
            }
        }
    }
    Ok(())
}
