use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::knowledge::vector_store::VectorStore;
use crate::memory::semantic::SharedKnowledgeGraph;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedContext {
    pub content: String,
    pub source: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEntity {
    pub name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EntityExtractorMode {
    Rule,
    Phrase,
    Hybrid,
}

use std::str::FromStr;

impl FromStr for EntityExtractorMode {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_lowercase().as_str() {
            "rule" => Ok(Self::Rule),
            "phrase" => Ok(Self::Phrase),
            _ => Ok(Self::Hybrid),
        }
    }
}

impl EntityExtractorMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Rule => "rule",
            Self::Phrase => "phrase",
            Self::Hybrid => "hybrid",
        }
    }
}

pub struct EntityExtractor {
    mode: EntityExtractorMode,
}

impl EntityExtractor {
    pub fn new(mode: EntityExtractorMode) -> Self {
        Self { mode }
    }

    pub async fn extract_batch(&self, texts: &[&str]) -> Result<Vec<ExtractedEntity>> {
        let mut set: HashSet<String> = HashSet::new();
        match self.mode {
            EntityExtractorMode::Rule => {
                set.extend(self.extract_rule_entities(texts));
            }
            EntityExtractorMode::Phrase => {
                set.extend(self.extract_phrase_entities(texts));
            }
            EntityExtractorMode::Hybrid => {
                set.extend(self.extract_rule_entities(texts));
                set.extend(self.extract_phrase_entities(texts));
            }
        }
        let mut entities: Vec<ExtractedEntity> = set.into_iter().map(|name| ExtractedEntity { name }).collect();
        entities.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entities)
    }

    fn extract_rule_entities(&self, texts: &[&str]) -> HashSet<String> {
        let mut set: HashSet<String> = HashSet::new();
        for text in texts {
            for token in text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-') {
                let t = token.trim().to_lowercase();
                if t.len() >= 4 && !is_stopword(&t) {
                    set.insert(t);
                }
            }
        }
        set
    }

    fn extract_phrase_entities(&self, texts: &[&str]) -> HashSet<String> {
        let mut set: HashSet<String> = HashSet::new();
        for text in texts {
            let lowered = text.to_lowercase();
            let words: Vec<&str> = lowered
                .split_whitespace()
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '-'))
                .filter(|w| w.len() >= 3 && !is_stopword(w))
                .collect();
            for win in words.windows(2) {
                if let [a, b] = win {
                    let phrase = format!("{} {}", a, b);
                    if phrase.len() >= 7 {
                        set.insert(phrase);
                    }
                }
            }
        }
        set
    }
}

use moka::future::Cache as MokaCache;
use std::time::Duration;

pub struct RetrievalCache {
    pub query_cache: MokaCache<String, Vec<RetrievedContext>>,
    pub embedding_cache: MokaCache<String, Vec<f32>>,
    pub subgraph_cache: MokaCache<String, Vec<(String, String, String)>>,
    // Hash-based embedding cache to reduce memory usage and improve hit rate
    pub embedding_hash_cache: MokaCache<u64, Vec<f32>>,
}

impl RetrievalCache {
    pub fn new() -> Self {
        Self {
            query_cache: MokaCache::builder()
                .max_capacity(1000)
                .time_to_live(Duration::from_secs(300))
                .build(),
            embedding_cache: MokaCache::builder()
                .max_capacity(2000)
                .time_to_live(Duration::from_secs(600))
                .build(),
            subgraph_cache: MokaCache::builder()
                .max_capacity(500)
                .time_to_live(Duration::from_secs(120))
                .build(),
            embedding_hash_cache: MokaCache::builder()
                .max_capacity(5000)
                .time_to_live(Duration::from_secs(900))
                .build(),
        }
    }
}

pub struct GraphRAG {
    vector_store: Arc<VectorStore>,
    knowledge_graph: SharedKnowledgeGraph,
    entity_extractor: Arc<EntityExtractor>,
    cache: Arc<RetrievalCache>,
}

impl GraphRAG {
    pub fn new(vector_store: Arc<VectorStore>, knowledge_graph: SharedKnowledgeGraph) -> Self {
        Self::new_with_mode(vector_store, knowledge_graph, EntityExtractorMode::Hybrid)
    }

    pub fn new_with_mode(
        vector_store: Arc<VectorStore>,
        knowledge_graph: SharedKnowledgeGraph,
        mode: EntityExtractorMode,
    ) -> Self {
        Self {
            vector_store,
            knowledge_graph,
            entity_extractor: Arc::new(EntityExtractor::new(mode)),
            cache: Arc::new(RetrievalCache::new()),
        }
    }

    pub async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<RetrievedContext>> {
        // 1. Check Query Cache
        if let Some(cached) = self.cache.query_cache.get(query).await {
            return Ok(cached);
        }

        // 2. Parallel Vector Search and Entity Extraction
        let query_batch = vec![query.to_string()];
        let query_refs: Vec<&str> = query_batch.iter().map(|s| s.as_str()).collect();
        let (search_res, query_entities_res): (Result<(Vec<(String, f32, serde_json::Value)>, Vec<f32>)>, Result<Vec<ExtractedEntity>>) = tokio::join!(
            self.vector_store.search_with_embedding(query, top_k.saturating_mul(2).max(top_k)),
            self.entity_extractor.extract_batch(&query_refs)
        );
        
        let (raw_results, query_embedding) = search_res?;
        let mut query_entities = query_entities_res?;
        
        // 3. Extract entities from results
        let contents: Vec<&str> = raw_results.iter().map(|(content, _, _)| content.as_str()).collect();
        let mut entities = self.entity_extractor.extract_batch(&contents).await?;
        entities.append(&mut query_entities);
        
        let mut uniq: HashMap<String, ExtractedEntity> = HashMap::new();
        for e in entities {
            uniq.entry(e.name.clone()).or_insert(e);
        }
        let mut entities: Vec<ExtractedEntity> = uniq.into_values().collect();
        entities.sort_by(|a, b| a.name.cmp(&b.name));
        entities.truncate(10);

        // 4. Optimized Batch Graph Traversal with Subgraph Cache
        let entity_names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();

        // Check cache for entities that are not cached
        let mut uncached_entities = Vec::new();
        for name in &entity_names {
            if self.cache.subgraph_cache.get(name).await.is_none() {
                uncached_entities.push(name.clone());
            }
        }

        // Batch fetch relations for uncached entities (optimized: single DB query instead of N queries)
        let batch_relations = if !uncached_entities.is_empty() {
            self.knowledge_graph.find_related_batch(&uncached_entities).await.unwrap_or_default()
        } else {
            HashMap::new()
        };

        // Update cache with batch results
        for (name, relations) in &batch_relations {
            self.cache.subgraph_cache.insert(name.clone(), relations.clone()).await;
        }

        // Collect all relations (from cache + batch fetch)
        let mut graph_context_lines = Vec::new();
        let mut relation_hits: HashMap<String, usize> = HashMap::new();
        let mut centrality: HashMap<String, f32> = HashMap::new();

        for name in &entity_names {
            let relations = self.cache.subgraph_cache.get(name).await.unwrap_or_default();
            relation_hits.insert(name.clone(), relations.len());

            for (direction, relation, target) in relations {
                graph_context_lines.push(format!("{} {} {} {}", name, direction, relation, target));
                let src = name.clone();
                let dst = target.to_lowercase();
                match direction.as_str() {
                    "->" | "<-" => {
                        *centrality.entry(src).or_insert(0.0) += 1.0;
                        *centrality.entry(dst).or_insert(0.0) += 1.0;
                    }
                    _ => {
                        *centrality.entry(src).or_insert(0.0) += 0.5;
                        *centrality.entry(dst).or_insert(0.0) += 0.5;
                    }
                }
            }
        }
        
        normalize_centrality(&mut centrality);

        // 5. Calculate Graph Signal with optimized caching (hash-based + similarity cache)
        let graph_context_text = graph_context_lines.join("\n");
        let graph_signal = if graph_context_text.is_empty() {
            0.0
        } else {
            // Try hash-based cache first (more efficient memory usage)
            let graph_hash = compute_text_hash(&graph_context_text);
            let graph_embedding = if let Some(cached) = self.cache.embedding_hash_cache.get(&graph_hash).await {
                cached
            } else if let Some(cached) = self.cache.embedding_cache.get(&graph_context_text).await {
                // Fallback to string-based cache for backward compatibility
                cached
            } else {
                let emb = self.vector_store.embed_query(&graph_context_text).await?;
                // Store in both caches
                self.cache.embedding_cache.insert(graph_context_text.clone(), emb.clone()).await;
                self.cache.embedding_hash_cache.insert(graph_hash, emb.clone()).await;
                emb
            };
            cosine_similarity(&query_embedding, &graph_embedding).clamp(0.0, 1.0)
        };

        // 6. Rerank
        let mut final_results: Vec<(f32, RetrievedContext)> = raw_results
            .into_iter()
            .map(|(content, score, metadata)| {
                let graph_boost = self.calculate_graph_boost(&content, &entities, &relation_hits, &centrality, graph_signal);
                let final_score = score * 0.7 + graph_boost * 0.3;
                let source = metadata
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("vector_store")
                    .to_string();
                (
                    final_score,
                    RetrievedContext {
                        content: content.clone(),
                        source,
                        score: final_score,
                    },
                )
            })
            .collect();

        final_results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut results: Vec<RetrievedContext> = final_results
            .into_iter()
            .take(top_k)
            .map(|(_, r)| r)
            .collect();

        if !graph_context_lines.is_empty() {
            results.push(RetrievedContext {
                content: format!("Relevant knowledge graph relations:\n{}", graph_context_lines.join("\n")),
                source: "knowledge_graph".to_string(),
                score: 0.0,
            });
        }

        // 7. Update Query Cache
        self.cache.query_cache.insert(query.to_string(), results.clone()).await;

        Ok(results)
    }

    fn calculate_graph_boost(
        &self,
        content: &str,
        entities: &[ExtractedEntity],
        relation_hits: &HashMap<String, usize>,
        centrality: &HashMap<String, f32>,
        graph_signal: f32,
    ) -> f32 {
        let lc = content.to_lowercase();
        let mut entity_coverage = 0.0f32;
        let mut relation_weight = 0.0f32;
        let mut centrality_weight = 0.0f32;
        for entity in entities {
            if lc.contains(&entity.name) {
                entity_coverage += 1.0;
                relation_weight += relation_hits
                    .get(&entity.name)
                    .copied()
                    .unwrap_or(0) as f32;
                centrality_weight += centrality.get(&entity.name).copied().unwrap_or(0.0);
            }
        }
        let coverage_score = if entities.is_empty() { 0.0 } else { entity_coverage / entities.len() as f32 };
        let relation_score = (relation_weight.ln_1p() * 0.18).clamp(0.0, 1.0);
        let centrality_score = centrality_weight.clamp(0.0, 1.0);
        (coverage_score * 0.4 + relation_score * 0.25 + centrality_score * 0.15 + graph_signal * 0.2).clamp(0.0, 1.0)
    }
}

fn normalize_centrality(centrality: &mut HashMap<String, f32>) {
    let max = centrality.values().copied().fold(0.0f32, f32::max);
    if max <= 0.0 {
        return;
    }
    for v in centrality.values_mut() {
        *v = (*v / max).clamp(0.0, 1.0);
    }
}

fn compute_text_hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn is_stopword(token: &str) -> bool {
    matches!(
        token,
        "the"
            | "and"
            | "with"
            | "from"
            | "that"
            | "this"
            | "for"
            | "into"
            | "about"
            | "using"
            | "have"
            | "been"
            | "will"
            | "your"
            | "then"
            | "when"
            | "where"
            | "what"
            | "which"
            | "while"
            | "also"
            | "there"
            | "their"
            | "them"
            | "生产"
            | "功能"
            | "可以"
    )
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot_product: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot_product / (norm_a * norm_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::memory::semantic::KnowledgeGraph;

    struct MockKg;

    #[async_trait]
    impl KnowledgeGraph for MockKg {
        async fn add_entity(&self, _name: &str, _type_: &str) -> Result<String> {
            Ok("e".to_string())
        }
        async fn add_relation(&self, _source: &str, _target: &str, _relation: &str) -> Result<()> {
            Ok(())
        }
        async fn find_related(&self, entity_name: &str) -> Result<Vec<(String, String, String)>> {
            if entity_name.contains("rust") {
                Ok(vec![("->".to_string(), "uses".to_string(), "tokio".to_string())])
            } else {
                Ok(vec![])
            }
        }
        async fn find_related_batch(&self, entity_names: &[String]) -> Result<HashMap<String, Vec<(String, String, String)>>> {
            let mut results = HashMap::new();
            for name in entity_names {
                if name.contains("rust") {
                    results.insert(name.clone(), vec![("->".to_string(), "uses".to_string(), "tokio".to_string())]);
                } else {
                    results.insert(name.clone(), vec![]);
                }
            }
            Ok(results)
        }
        async fn find_entities_batch(&self, names: &[String]) -> Result<Vec<(String, String)>> {
            Ok(names.iter().map(|n| (n.clone(), "concept".to_string())).collect())
        }
        async fn export_d3_json(&self) -> Result<String> {
            Ok("{\"nodes\":[],\"links\":[]}".to_string())
        }
    }

    #[tokio::test]
    async fn graph_rag_returns_augmented_context() {
        let vs = Arc::new(VectorStore::new_in_memory());
        vs.add_documents(
            vec!["Rust async runtime with tokio".to_string()],
            vec![serde_json::json!({"source": "docA"})],
        ).await.expect("add docs");
        let kg: SharedKnowledgeGraph = Arc::new(MockKg);
        let rag = GraphRAG::new(vs, kg);
        let out = rag.retrieve("rust async", 2).await.expect("retrieve");
        assert!(!out.is_empty());
        assert!(out.iter().any(|x| x.source == "knowledge_graph"));
    }
}
