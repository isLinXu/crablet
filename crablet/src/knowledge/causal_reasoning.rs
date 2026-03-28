// Advanced GraphRAG with Causal Reasoning
// P0-3: Enhanced knowledge graph with causal chains and multi-hop reasoning

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::knowledge::graph_rag::GraphRAG;
use crate::memory::semantic::SharedKnowledgeGraph;
pub use crate::knowledge::graph_rag::RetrievedContext;

/// Causal reasoning chain for explainable AI
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CausalChain {
    /// Chain ID
    pub id: String,
    /// Nodes in the chain
    pub nodes: Vec<CausalNode>,
    /// Edges representing causal relationships
    pub edges: Vec<CausalEdge>,
    /// Confidence score
    pub confidence: f32,
    /// Reasoning depth (hops)
    pub depth: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalNode {
    pub id: String,
    pub entity: String,
    pub entity_type: String,
    pub description: String,
    pub is_query_entity: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub causal_strength: f32,
    pub evidence: Vec<String>,
}

impl CausalChain {
    /// Create a new causal chain
    pub fn new(id: String) -> Self {
        Self {
            id,
            nodes: Vec::new(),
            edges: Vec::new(),
            confidence: 0.0,
            depth: 0,
        }
    }
    
    /// Add a node to the chain
    pub fn add_node(&mut self, node: CausalNode) {
        self.nodes.push(node);
    }
    
    /// Add an edge to the chain
    pub fn add_edge(&mut self, edge: CausalEdge) {
        self.edges.push(edge);
        self.depth = self.depth.max(self.edges.len());
    }
    
    /// Calculate chain confidence based on edge strengths
    pub fn calculate_confidence(&mut self) {
        if self.edges.is_empty() {
            self.confidence = 0.0;
            return;
        }
        let sum: f32 = self.edges.iter().map(|e| e.causal_strength).sum();
        self.confidence = sum / self.edges.len() as f32;
    }
}

/// Multi-hop reasoning types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiHopQuery {
    pub query: String,
    pub hops: usize,
    pub entities: Vec<String>,
    pub reasoning_type: ReasoningType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReasoningType {
    /// A causes B
    Causal,
    /// A is part of B
    Compositional,
    /// A is similar to B  
    Analogical,
    /// A contradicts B
    Contrastive,
}

/// Query analysis for reasoning type detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryAnalysis {
    pub reasoning_type: ReasoningType,
    pub keywords: Vec<String>,
    pub entity_count: usize,
    pub complexity_score: f32,
    pub suggested_hops: usize,
}

impl QueryAnalysis {
    pub fn analyze(query: &str) -> Self {
        let lower = query.to_lowercase();
        let keywords: Vec<String> = lower
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .map(|w| w.to_string())
            .collect();
        let entity_count = keywords.len();
        
        // Detect reasoning type
        let reasoning_type = if keywords.iter().any(|w| 
            w.contains("why") || w.contains("cause") || w.contains("effect") || w.contains("导致") || w.contains("原因")
        ) {
            ReasoningType::Causal
        } else if keywords.iter().any(|w|
            w.contains("part") || w.contains("component") || w.contains("组成") || w.contains("部分")
        ) {
            ReasoningType::Compositional
        } else if keywords.iter().any(|w|
            w.contains("similar") || w.contains("like") || w.contains("similar") || w.contains("类似")
        ) {
            ReasoningType::Analogical
        } else if keywords.iter().any(|w|
            w.contains("but") || w.contains("however") || w.contains("然而") || w.contains("但是")
        ) {
            ReasoningType::Contrastive
        } else {
            ReasoningType::Causal // Default to causal
        };
        
        // Estimate complexity
        let complexity_score = (keywords.len() as f32 / 10.0).min(1.0);
        
        // Suggest hops based on query complexity
        let suggested_hops = match keywords.len() {
            0..=3 => 1,
            4..=7 => 2,
            8..=12 => 3,
            _ => 4,
        };
        
        Self {
            reasoning_type,
            keywords,
            entity_count,
            complexity_score,
            suggested_hops,
        }
    }
}

/// Causal reasoning engine
pub struct CausalReasoningEngine {
    /// Maximum chain depth
    max_depth: usize,
    /// Minimum causal strength threshold
    min_causal_strength: f32,
}

impl CausalReasoningEngine {
    pub fn new() -> Self {
        Self {
            max_depth: 5,
            min_causal_strength: 0.3,
        }
    }
    
    /// Build causal chain from query
    pub async fn build_causal_chain(
        &self,
        query: &str,
        knowledge_graph: &dyn KnowledgeGraphSearch,
    ) -> Result<CausalChain> {
        let mut chain = CausalChain::new(format!("causal_{}", generate_uuid()));
        
        // Extract entities from query
        let entities = self.extract_entities(query);
        
        // Start with query entities as first nodes
        for entity in &entities {
            chain.add_node(CausalNode {
                id: entity.clone(),
                entity: entity.clone(),
                entity_type: "query_entity".to_string(),
                description: format!("Query entity: {}", entity),
                is_query_entity: true,
            });
        }
        
        // BFS to find causal paths
        let mut visited: HashSet<String> = entities.iter().cloned().collect();
        let mut queue: VecDeque<(String, f32)> = entities
            .iter()
            .map(|e| (e.clone(), 1.0))
            .collect();
        
        while let Some((current, strength)) = queue.pop_front() {
            if chain.edges.len() >= self.max_depth {
                break;
            }
            
            // Find causal relations
            let relations = knowledge_graph.find_causal_relations(&current).await?;
            
            for (relation, target, causal_strength) in relations {
                if causal_strength < self.min_causal_strength {
                    continue;
                }
                
                if !visited.contains(&target) {
                    visited.insert(target.clone());
                    
                    // Add node if not exists
                    if !chain.nodes.iter().any(|n| n.id == target) {
                        chain.add_node(CausalNode {
                            id: target.clone(),
                            entity: target.clone(),
                            entity_type: "derived".to_string(),
                            description: format!("Derived from: {}", current),
                            is_query_entity: false,
                        });
                    }
                    
                    // Add causal edge
                    chain.add_edge(CausalEdge {
                        source: current.clone(),
                        target: target.clone(),
                        relation,
                        causal_strength,
                        evidence: vec![format!("Path strength: {:.2}", strength * causal_strength)],
                    });
                    
                    // Continue search with reduced strength
                    queue.push_back((target, strength * causal_strength));
                }
            }
        }
        
        chain.calculate_confidence();
        Ok(chain)
    }
    
    /// Extract entities from query text
    fn extract_entities(&self, query: &str) -> Vec<String> {
        // Simple extraction - in production would use NER
        let words: Vec<&str> = query
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|w| w.len() > 2)
            .collect();
        
        // Filter stopwords
        let stopwords = ["the", "and", "for", "with", "from", "that", "this", "are", "was", "were"];
        words
            .into_iter()
            .filter(|w| !stopwords.contains(&w.to_lowercase().as_str()))
            .map(|w| w.to_string())
            .collect()
    }
}

/// Multi-hop reasoning engine
pub struct MultiHopReasoningEngine {
    max_hops: usize,
}

impl MultiHopReasoningEngine {
    pub fn new() -> Self {
        Self { max_hops: 3 }
    }
    
    /// Perform multi-hop reasoning
    pub async fn reason(
        &self,
        query: &MultiHopQuery,
        knowledge_graph: &dyn KnowledgeGraphSearch,
    ) -> Result<Vec<CausalChain>> {
        let mut chains = Vec::new();
        
        for entity in &query.entities {
            let chain = self.hop_search(entity, query.hops, knowledge_graph).await?;
            chains.push(chain);
        }
        
        // Merge chains that share common nodes
        let merged = self.merge_chains(chains);
        Ok(merged)
    }
    
    /// BFS hop search
    async fn hop_search(
        &self,
        start_entity: &str,
        max_hops: usize,
        knowledge_graph: &dyn KnowledgeGraphSearch,
    ) -> Result<CausalChain> {
        let mut chain = CausalChain::new(generate_uuid());
        
        chain.add_node(CausalNode {
            id: start_entity.to_string(),
            entity: start_entity.to_string(),
            entity_type: "start".to_string(),
            description: "Starting entity".to_string(),
            is_query_entity: true,
        });
        
        let mut visited: HashSet<String> = [start_entity.to_string()].into_iter().collect();
        let mut current_level: Vec<String> = vec![start_entity.to_string()];
        
        for _hop in 0..max_hops {
            let mut next_level: Vec<String> = Vec::new();
            
            for entity in &current_level {
                let relations = knowledge_graph.find_related(entity, 3).await?;
                
                for (direction, relation, target) in relations {
                    if !visited.contains(&target) {
                        visited.insert(target.clone());
                        next_level.push(target.clone());
                        
                        chain.add_node(CausalNode {
                            id: target.clone(),
                            entity: target.clone(),
                            entity_type: "hop".to_string(),
                            description: format!("Hop {} relation: {}", _hop + 1, relation),
                            is_query_entity: false,
                        });
                        
                        chain.add_edge(CausalEdge {
                            source: entity.clone(),
                            target: target.clone(),
                            relation: format!("{} {}", direction, relation),
                            causal_strength: 0.8_f32.powi((_hop + 1) as i32),
                            evidence: vec![format!("Hop {}", _hop + 1)],
                        });
                    }
                }
            }
            
            current_level = next_level;
            
            if current_level.is_empty() {
                break;
            }
        }
        
        chain.calculate_confidence();
        Ok(chain)
    }
    
    /// Merge overlapping chains
    fn merge_chains(&self, chains: Vec<CausalChain>) -> Vec<CausalChain> {
        if chains.len() <= 1 {
            return chains;
        }
        
        // Find common nodes between chains
        let mut merged: Vec<CausalChain> = Vec::new();
        let mut used: Vec<bool> = vec![false; chains.len()];
        
        for i in 0..chains.len() {
            if used[i] { continue; }
            
            let mut combined = chains[i].clone();
            used[i] = true;
            
            for j in (i + 1)..chains.len() {
                if used[j] { continue; }
                
                // Check if chains share any nodes
                let common: HashSet<_> = combined.nodes.iter()
                    .map(|n| n.id.clone())
                    .collect();
                
                let has_common = chains[j].nodes.iter()
                    .any(|n| common.contains(&n.id));
                
                if has_common {
                    // Merge chains
                    for node in &chains[j].nodes {
                        if !combined.nodes.iter().any(|n| n.id == node.id) {
                            combined.nodes.push(node.clone());
                        }
                    }
                    for edge in &chains[j].edges {
                        if !combined.edges.iter().any(|e| e.source == edge.source && e.target == edge.target) {
                            combined.edges.push(edge.clone());
                        }
                    }
                    used[j] = true;
                }
            }
            
            combined.calculate_confidence();
            merged.push(combined);
        }
        
        merged
    }
}

/// Knowledge graph search trait (simplified)
#[async_trait::async_trait]
pub trait KnowledgeGraphSearch: Send + Sync {
    /// Find related entities
    async fn find_related(&self, entity: &str, limit: usize) -> Result<Vec<(String, String, String)>>;
    
    /// Find causal relations
    async fn find_causal_relations(&self, entity: &str) -> Result<Vec<(String, String, f32)>>;
}

#[async_trait::async_trait]
impl KnowledgeGraphSearch for SharedKnowledgeGraph {
    async fn find_related(&self, entity: &str, limit: usize) -> Result<Vec<(String, String, String)>> {
        let mut relations = self.as_ref().find_related(entity).await?;
        relations.truncate(limit);
        Ok(relations)
    }

    async fn find_causal_relations(&self, entity: &str) -> Result<Vec<(String, String, f32)>> {
        let relations = self.as_ref().find_related(entity).await?;
        Ok(relations
            .into_iter()
            .filter_map(|(direction, relation, target)| {
                let strength = estimate_causal_strength(&relation);
                if strength > 0.0 {
                    Some((format!("{} {}", direction, relation).trim().to_string(), target, strength))
                } else {
                    None
                }
            })
            .collect())
    }
}

/// GraphRAG enhanced with causal reasoning
pub struct EnhancedGraphRAG {
    inner: Arc<GraphRAG>,
    causal_engine: CausalReasoningEngine,
    hop_engine: MultiHopReasoningEngine,
}

impl EnhancedGraphRAG {
    pub fn new(inner: Arc<GraphRAG>) -> Self {
        Self {
            inner,
            causal_engine: CausalReasoningEngine::new(),
            hop_engine: MultiHopReasoningEngine::new(),
        }
    }
    
    /// Retrieve with causal reasoning
    pub async fn retrieve_with_causality(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<(Vec<RetrievedContext>, Vec<CausalChain>)> {
        // First get standard retrieval
        let contexts = self.inner.retrieve(query, top_k).await?;
        
        // Build causal chains
        let chains = self.causal_engine
            .build_causal_chain(query, self.inner.knowledge_graph())
            .await
            .unwrap_or_default();
        
        Ok((contexts, vec![chains]))
    }
    
    /// Retrieve with multi-hop reasoning
    pub async fn retrieve_with_multihop(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<(Vec<RetrievedContext>, Vec<CausalChain>)> {
        let contexts = self.inner.retrieve(query, top_k).await?;
        
        let analysis = QueryAnalysis::analyze(query);
        let hop_query = MultiHopQuery {
            query: query.to_string(),
            hops: analysis.suggested_hops,
            entities: analysis.keywords,
            reasoning_type: analysis.reasoning_type,
        };
        
        let chains = self.hop_engine
            .reason(&hop_query, self.inner.knowledge_graph())
            .await
            .unwrap_or_default();
        
        Ok((contexts, chains))
    }
    
    /// Export chains as visualization data
    pub fn export_chains_for_visualization(&self, chains: &[CausalChain]) -> serde_json::Value {
        let mut nodes: Vec<serde_json::Value> = Vec::new();
        let mut links: Vec<serde_json::Value> = Vec::new();
        let mut node_ids: HashSet<String> = HashSet::new();
        
        for chain in chains {
            for node in &chain.nodes {
                if node_ids.insert(node.id.clone()) {
                    nodes.push(serde_json::json!({
                        "id": node.id,
                        "label": node.entity,
                        "type": node.entity_type,
                        "description": node.description,
                        "isQueryEntity": node.is_query_entity,
                    }));
                }
            }
            
            for edge in &chain.edges {
                links.push(serde_json::json!({
                    "source": edge.source,
                    "target": edge.target,
                    "relation": edge.relation,
                    "strength": edge.causal_strength,
                    "evidence": edge.evidence,
                }));
            }
        }
        
        serde_json::json!({
            "nodes": nodes,
            "links": links,
            "stats": {
                "totalChains": chains.len(),
                "totalNodes": nodes.len(),
                "totalEdges": links.len(),
                "avgConfidence": if chains.is_empty() { 0.0 } else {
                    chains.iter().map(|c| c.confidence).sum::<f32>() / chains.len() as f32
                },
                "avgDepth": if chains.is_empty() { 0.0 } else {
                    chains.iter().map(|c| c.depth as f32).sum::<f32>() / chains.len() as f32
                },
            }
        })
    }
}

// UUID generation (simplified)
fn estimate_causal_strength(relation: &str) -> f32 {
    let lower = relation.to_lowercase();
    if ["cause", "causes", "caused", "lead", "leads", "result", "results", "trigger", "triggers", "depend", "depends", "impact", "impacts", "影响", "导致", "造成", "引发"]
        .iter()
        .any(|keyword| lower.contains(keyword))
    {
        0.85
    } else if ["use", "uses", "关联", "related", "linked", "connected"]
        .iter()
        .any(|keyword| lower.contains(keyword))
    {
        0.45
    } else {
        0.0
    }
}

fn generate_uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:x}", timestamp)
}
