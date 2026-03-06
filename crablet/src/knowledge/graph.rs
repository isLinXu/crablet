use anyhow::{Result, anyhow};
use std::sync::Arc;
#[cfg(feature = "knowledge")]
use neo4rs::{Graph, query};
use crate::cognitive::llm::LlmClient;
use crate::types::Message;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

#[cfg(feature = "knowledge")]
#[derive(Clone)]
pub struct KnowledgeGraph {
    graph: Arc<Graph>,
    llm: Arc<Box<dyn LlmClient>>,
}

#[cfg(not(feature = "knowledge"))]
#[derive(Clone)]
pub struct KnowledgeGraph {
    llm: Arc<Box<dyn LlmClient>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExtractedTriple {
    source: String,
    relation: String,
    target: String,
}

impl KnowledgeGraph {
    #[cfg(feature = "knowledge")]
    pub async fn new(uri: &str, user: &str, pass: &str, llm: Arc<Box<dyn LlmClient>>) -> Result<Self> {
        let graph = Graph::new(uri, user, pass).await
            .map_err(|e| anyhow!("Failed to connect to Neo4j: {}", e))?;
            
        Ok(Self {
            graph: Arc::new(graph),
            llm,
        })
    }
    
    #[cfg(not(feature = "knowledge"))]
    pub async fn new(_uri: &str, _user: &str, _pass: &str, llm: Arc<Box<dyn LlmClient>>) -> Result<Self> {
        Ok(Self {
            llm,
        })
    }
    
    /// Ingest text, extract entities and relations, and store in Neo4j
    pub async fn ingest_text(&self, text: &str) -> Result<()> {
        // 1. Extract Triples via LLM
        let triples = self.extract_triples(text).await?;
        
        if triples.is_empty() {
            info!("No knowledge triples extracted from text.");
            return Ok(());
        }
        
        info!("Extracted {} triples. Writing to Neo4j...", triples.len());
        
        #[cfg(feature = "knowledge")]
        {
            // 2. Write to Neo4j
            for triple in triples {
                let rel_type = self.sanitize_relation(&triple.relation);
                if rel_type.is_empty() { continue; }
                
                let cypher = format!(
                    "MERGE (s:Entity {{name: $source}}) \
                     MERGE (t:Entity {{name: $target}}) \
                     MERGE (s)-[r:{}]->(t) \
                     RETURN count(r)", 
                    rel_type
                );
                
                let result = self.graph.execute(
                    query(&cypher)
                        .param("source", triple.source)
                        .param("target", triple.target)
                ).await;
                
                if let Err(e) = result {
                    warn!("Failed to write triple to Neo4j: {}", e);
                }
            }
        }
        #[cfg(not(feature = "knowledge"))]
        {
            warn!("Knowledge graph feature disabled, skipping Neo4j write.");
        }
        
        Ok(())
    }
    
    /// Retrieve context by querying the graph for related entities (2-hop neighborhood)
    pub async fn query_context(&self, query_text: &str) -> Result<String> {
        // 1. Extract entities from query
        let entities = self.extract_entities(query_text).await?;
        if entities.is_empty() {
            return Ok(String::new());
        }
        
        let mut context_parts = Vec::new();
        
        #[cfg(feature = "knowledge")]
        {
            for entity in entities {
                // 2. Query 1-hop or 2-hop neighborhood
                let cypher = "MATCH (s:Entity {name: $name})-[r]->(t:Entity) \
                              RETURN type(r) as rel, t.name as target \
                              LIMIT 10";
                
                let mut result = self.graph.execute(
                    query(cypher).param("name", entity.clone())
                ).await?;
                
                while let Ok(Some(row)) = result.next().await {
                    let rel: String = row.get("rel").unwrap_or_default();
                    let target: String = row.get("target").unwrap_or_default();
                    context_parts.push(format!("({}) -[{}]-> ({})", entity, rel, target));
                }
            }
        }
        
        Ok(context_parts.join("\n"))
    }
    
    async fn extract_triples(&self, text: &str) -> Result<Vec<ExtractedTriple>> {
        let prompt = format!(
            "Extract knowledge triples (Source, Relation, Target) from the following text.\n\
             Text: {}\n\
             \n\
             Rules:\n\
             1. Entities should be singular, capitalized concepts.\n\
             2. Relations should be SCREAMING_SNAKE_CASE (e.g., HAS_PART, LOCATED_IN).\n\
             3. Output JSON list of objects: [{{ \"source\": \"...\", \"relation\": \"...\", \"target\": \"...\" }}]",
            text
        );
        
        let response = self.llm.chat_complete(&[Message::user(&prompt)]).await?;
        let json_str = self.extract_json(&response).unwrap_or("[]");
        
        let triples: Vec<ExtractedTriple> = serde_json::from_str(json_str).unwrap_or_default();
        Ok(triples)
    }
    
    async fn extract_entities(&self, text: &str) -> Result<Vec<String>> {
         let prompt = format!(
            "Extract key entities (nouns, concepts) from the following query for graph lookup.\n\
             Query: {}\n\
             \n\
             Output JSON list of strings.",
            text
        );
        
        let response = self.llm.chat_complete(&[Message::user(&prompt)]).await?;
        let json_str = self.extract_json(&response).unwrap_or("[]");
        
        let entities: Vec<String> = serde_json::from_str(json_str).unwrap_or_default();
        Ok(entities)
    }
    
    fn sanitize_relation(&self, rel: &str) -> String {
        rel.chars()
           .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
           .collect::<String>()
           .to_uppercase()
    }
    
    fn extract_json<'a>(&self, text: &'a str) -> Option<&'a str> {
        let start = text.find('[')?;
        let end = text.rfind(']')?;
        if start <= end {
            Some(&text[start..=end])
        } else {
            None
        }
    }
}
