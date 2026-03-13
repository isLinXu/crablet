//! Knowledge Weaver - Continuous knowledge relationship discovery
//!
//! This module continuously discovers and maintains relationships
//! between knowledge entities:
//! - Entity relationship extraction
//! - Knowledge graph enrichment
//! - Concept clustering
//! - Semantic bridge discovery
//! - Cross-domain connection finding
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    Knowledge Weaver                                 │
//! │                                                                      │
//! │   ┌─────────────┐    ┌─────────────┐    ┌──────────────────────┐   │
//! │   │  Ingest     │───→│  Extract    │───→│  Weave Relations     │   │
//! │   │  Memories   │    │  Entities   │    │                      │   │
//! │   └─────────────┘    └─────────────┘    └──────────────────────┘   │
//! │                                                │                     │
//! │                                                ▼                     │
//! │   ┌────────────────────────────────────────────────────────────┐   │
//! │   │                    Weaving Operations                      │   │
//! │   │  • Entity Linking (connect related entities)               │   │
//! │   │  • Concept Clustering (group similar concepts)             │   │
//! │   │  • Semantic Bridges (find cross-domain links)              │   │
//! │   │  • Knowledge Gaps (identify missing connections)           │   │
//! │   │  • Inference Rules (discover implicit relationships)       │   │
//! │   └────────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────┘

use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn, debug};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::events::{AgentEvent, EventBus};
use crate::knowledge::graph::KnowledgeGraph;
use crate::knowledge::vector_store::VectorStore;
use crate::cognitive::llm::LlmClient;
use crate::error::Result;

/// Configuration for Knowledge Weaver
#[derive(Debug, Clone)]
pub struct KnowledgeWeaverConfig {
    /// How often to run weaving operations (default: 30 minutes)
    pub weave_interval: Duration,
    /// Minimum confidence for new relationships
    pub min_relationship_confidence: f32,
    /// Maximum entities to process per cycle
    pub max_entities_per_cycle: usize,
    /// Enable entity linking
    pub enable_entity_linking: bool,
    /// Enable concept clustering
    pub enable_concept_clustering: bool,
    /// Enable semantic bridge discovery
    pub enable_semantic_bridges: bool,
    /// Enable inference rule discovery
    pub enable_inference_rules: bool,
    /// Similarity threshold for clustering
    pub clustering_similarity_threshold: f32,
    /// Maximum cluster size
    pub max_cluster_size: usize,
}

impl Default for KnowledgeWeaverConfig {
    fn default() -> Self {
        Self {
            weave_interval: Duration::from_secs(1800), // 30 minutes
            min_relationship_confidence: 0.75,
            max_entities_per_cycle: 50,
            enable_entity_linking: true,
            enable_concept_clustering: true,
            enable_semantic_bridges: true,
            enable_inference_rules: true,
            clustering_similarity_threshold: 0.8,
            max_cluster_size: 20,
        }
    }
}

/// Types of relationships that can be discovered
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RelationshipType {
    /// Direct semantic similarity
    Similar,
    /// Part-whole relationship
    PartOf,
    /// Type-instance relationship
    InstanceOf,
    /// Causal relationship
    Causes,
    /// Temporal relationship
    Precedes,
    /// Functional relationship
    UsedFor,
    /// Cross-domain analogy
    Analogous,
    /// Implicit connection
    Related,
}

/// A discovered relationship between entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredRelationship {
    pub id: String,
    pub source_entity: String,
    pub target_entity: String,
    pub relationship_type: RelationshipType,
    pub confidence: f32,
    pub evidence: Vec<String>,
    pub discovered_at: DateTime<Utc>,
    pub verified: bool,
    pub metadata: serde_json::Value,
}

impl DiscoveredRelationship {
    pub fn new(
        source: String,
        target: String,
        rel_type: RelationshipType,
        confidence: f32,
        evidence: Vec<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source_entity: source,
            target_entity: target,
            relationship_type: rel_type,
            confidence,
            evidence,
            discovered_at: Utc::now(),
            verified: false,
            metadata: serde_json::Value::Null,
        }
    }
}

/// A cluster of related concepts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptCluster {
    pub id: String,
    pub name: String,
    pub concepts: Vec<String>,
    pub centroid_embedding: Option<Vec<f32>>,
    pub created_at: DateTime<Utc>,
    pub coherence_score: f32,
}

/// A semantic bridge connecting different domains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticBridge {
    pub id: String,
    pub domain_a: String,
    pub domain_b: String,
    pub connecting_concepts: Vec<(String, String)>,
    pub bridge_strength: f32,
    pub discovered_at: DateTime<Utc>,
}

/// Statistics for Knowledge Weaver
#[derive(Debug, Clone, Default)]
pub struct KnowledgeWeaverStats {
    pub total_weave_cycles: u64,
    pub relationships_discovered: u64,
    pub relationships_verified: u64,
    pub clusters_formed: u64,
    pub bridges_discovered: u64,
    pub inference_rules_found: u64,
    pub last_weave: Option<DateTime<Utc>>,
    pub avg_weave_duration_ms: u64,
}

/// Knowledge Weaver - Continuous knowledge relationship discovery
pub struct KnowledgeWeaver {
    config: KnowledgeWeaverConfig,
    event_bus: Arc<EventBus>,
    knowledge_graph: Option<Arc<RwLock<KnowledgeGraph>>>,
    vector_store: Option<Arc<VectorStore>>,
    llm: Arc<Box<dyn LlmClient>>,
    /// Discovered relationships
    relationships: Arc<RwLock<Vec<DiscoveredRelationship>>>,
    /// Concept clusters
    clusters: Arc<RwLock<Vec<ConceptCluster>>>,
    /// Semantic bridges
    bridges: Arc<RwLock<Vec<SemanticBridge>>>,
    /// Statistics
    stats: Arc<RwLock<KnowledgeWeaverStats>>,
    /// Shutdown signal
    shutdown: Arc<RwLock<bool>>,
}

impl KnowledgeWeaver {
    pub fn new(
        config: KnowledgeWeaverConfig,
        event_bus: Arc<EventBus>,
        knowledge_graph: Option<Arc<RwLock<KnowledgeGraph>>>,
        vector_store: Option<Arc<VectorStore>>,
        llm: Arc<Box<dyn LlmClient>>,
    ) -> Self {
        Self {
            config,
            event_bus,
            knowledge_graph,
            vector_store,
            llm,
            relationships: Arc::new(RwLock::new(Vec::new())),
            clusters: Arc::new(RwLock::new(Vec::new())),
            bridges: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(KnowledgeWeaverStats::default())),
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the knowledge weaver loop
    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            info!(
                "Knowledge Weaver started (interval: {:?})",
                self.config.weave_interval
            );

            let mut interval = tokio::time::interval(self.config.weave_interval);

            loop {
                interval.tick().await;

                if *self.shutdown.read().await {
                    info!("Knowledge Weaver shutting down");
                    break;
                }

                if let Err(e) = self.weave().await {
                    warn!("Knowledge weaving failed: {}", e);
                }
            }
        });
    }

    /// Stop the knowledge weaver
    pub async fn stop(&self) {
        *self.shutdown.write().await = true;
    }

    /// Perform a weaving cycle
    pub async fn weave(&self) -> Result<()> {
        let start_time = std::time::Instant::now();
        info!("Starting knowledge weaving cycle");

        let mut new_relationships = 0;
        let mut new_clusters = 0;
        let mut new_bridges = 0;

        // 1. Entity Linking
        if self.config.enable_entity_linking {
            let linked = self.discover_entity_relationships().await?;
            new_relationships += linked;
        }

        // 2. Concept Clustering
        if self.config.enable_concept_clustering {
            let clustered = self.perform_concept_clustering().await?;
            new_clusters += clustered;
        }

        // 3. Semantic Bridge Discovery
        if self.config.enable_semantic_bridges {
            let bridges = self.discover_semantic_bridges().await?;
            new_bridges += bridges;
        }

        // 4. Inference Rule Discovery
        if self.config.enable_inference_rules {
            self.discover_inference_rules().await?;
        }

        // Update statistics
        let duration_ms = start_time.elapsed().as_millis() as u64;
        {
            let mut stats = self.stats.write().await;
            stats.total_weave_cycles += 1;
            stats.relationships_discovered += new_relationships as u64;
            stats.clusters_formed += new_clusters as u64;
            stats.bridges_discovered += new_bridges as u64;
            stats.last_weave = Some(Utc::now());

            if stats.total_weave_cycles == 1 {
                stats.avg_weave_duration_ms = duration_ms;
            } else {
                stats.avg_weave_duration_ms =
                    (stats.avg_weave_duration_ms * (stats.total_weave_cycles - 1) + duration_ms)
                    / stats.total_weave_cycles;
            }
        }

        // Publish event
        self.event_bus.publish(AgentEvent::SystemLog(format!(
            "Knowledge weaving completed in {}ms: {} relationships, {} clusters, {} bridges",
            duration_ms, new_relationships, new_clusters, new_bridges
        )));

        info!(
            "Knowledge weaving completed in {}ms: {} relationships, {} clusters, {} bridges",
            duration_ms, new_relationships, new_clusters, new_bridges
        );

        Ok(())
    }

    /// Discover relationships between entities
    async fn discover_entity_relationships(&self) -> Result<usize> {
        let mut discovered = 0;

        // Get entities from knowledge graph
        let entities = if let Some(kg) = &self.knowledge_graph {
            // kg.read().await.get_all_entities().await?
            vec![] // Placeholder
        } else {
            vec![]
        };

        if entities.len() < 2 {
            return Ok(0);
        }

        // Use vector similarity to find potential relationships
        if let Some(vs) = &self.vector_store {
            for (i, entity_a) in entities.iter().enumerate().take(self.config.max_entities_per_cycle) {
                // Find similar entities
                // let similar = vs.search_similar(&entity_a, 5).await?;
                
                // For each potential relationship, verify with LLM
                // This is a simplified version
                
                if i >= self.config.max_entities_per_cycle {
                    break;
                }
            }
        }

        Ok(discovered)
    }

    /// Perform concept clustering
    async fn perform_concept_clustering(&self) -> Result<usize> {
        let mut new_clusters = 0;

        // Get all concepts from vector store
        let concepts = if let Some(vs) = &self.vector_store {
            // vs.get_all_concepts().await?
            vec![] // Placeholder
        } else {
            vec![]
        };

        if concepts.len() < 3 {
            return Ok(0);
        }

        // Simple clustering based on similarity
        // In a real implementation, this would use proper clustering algorithms
        // like HDBSCAN or K-means on embeddings

        let mut clustered = HashSet::new();
        let mut clusters = self.clusters.write().await;

        for concept in &concepts {
            if clustered.contains(concept) {
                continue;
            }

            // Find similar concepts
            let mut cluster_concepts = vec![concept.clone()];
            
            for other in &concepts {
                if concept == other || clustered.contains(other) {
                    continue;
                }

                // Check similarity
                let similarity = self.calculate_concept_similarity(concept, other).await?;
                
                if similarity >= self.config.clustering_similarity_threshold {
                    cluster_concepts.push(other.clone());
                    clustered.insert(other.clone());
                    
                    if cluster_concepts.len() >= self.config.max_cluster_size {
                        break;
                    }
                }
            }

            if cluster_concepts.len() >= 3 {
                let cluster = ConceptCluster {
                    id: uuid::Uuid::new_v4().to_string(),
                    name: format!("Cluster {}", clusters.len() + 1),
                    concepts: cluster_concepts,
                    centroid_embedding: None,
                    created_at: Utc::now(),
                    coherence_score: 0.8, // Placeholder
                };

                clusters.push(cluster);
                new_clusters += 1;
            }

            clustered.insert(concept.clone());
        }

        Ok(new_clusters)
    }

    /// Discover semantic bridges between domains
    async fn discover_semantic_bridges(&self) -> Result<usize> {
        let mut bridges_found = 0;

        // Get existing clusters (representing domains)
        let clusters = self.clusters.read().await;
        
        if clusters.len() < 2 {
            return Ok(0);
        }

        // Look for bridges between different clusters
        for (i, cluster_a) in clusters.iter().enumerate() {
            for cluster_b in clusters.iter().skip(i + 1) {
                // Find connecting concepts between clusters
                let bridges = self.find_cluster_bridges(cluster_a, cluster_b).await?;
                
                if !bridges.is_empty() {
                    let bridge = SemanticBridge {
                        id: uuid::Uuid::new_v4().to_string(),
                        domain_a: cluster_a.name.clone(),
                        domain_b: cluster_b.name.clone(),
                        connecting_concepts: bridges,
                        bridge_strength: 0.7, // Placeholder
                        discovered_at: Utc::now(),
                    };

                    self.bridges.write().await.push(bridge);
                    bridges_found += 1;
                }
            }
        }

        Ok(bridges_found)
    }

    /// Discover inference rules from existing relationships
    async fn discover_inference_rules(&self) -> Result<()> {
        // Analyze existing relationships to find patterns
        // that can be turned into inference rules
        
        let relationships = self.relationships.read().await;
        
        // Look for transitive patterns: A->B, B->C implies A->C
        // Look for symmetric patterns: A->B implies B->A
        // etc.
        
        debug!("Analyzing {} relationships for inference rules", relationships.len());
        
        Ok(())
    }

    /// Calculate similarity between two concepts
    async fn calculate_concept_similarity(&self, concept_a: &str, concept_b: &str) -> Result<f32> {
        // This would use vector embeddings in a real implementation
        // For now, return a simple heuristic
        
        if let Some(vs) = &self.vector_store {
            // vs.calculate_similarity(concept_a, concept_b).await
            Ok(0.5) // Placeholder
        } else {
            Ok(0.0)
        }
    }

    /// Find bridges between two clusters
    async fn find_cluster_bridges(&self, cluster_a: &ConceptCluster, cluster_b: &ConceptCluster) -> Result<Vec<(String, String)>> {
        let mut bridges = Vec::new();

        for concept_a in &cluster_a.concepts {
            for concept_b in &cluster_b.concepts {
                let similarity = self.calculate_concept_similarity(concept_a, concept_b).await?;
                
                if similarity >= self.config.clustering_similarity_threshold {
                    bridges.push((concept_a.clone(), concept_b.clone()));
                }
            }
        }

        Ok(bridges)
    }

    /// Verify a discovered relationship using LLM
    async fn verify_relationship(&self, relationship: &DiscoveredRelationship) -> Result<bool> {
        let prompt = format!(
            "Verify if the following relationship is valid:\n\n\
            Source: {}\n\
            Target: {}\n\
            Relationship Type: {:?}\n\n\
            Respond with only 'true' if valid, 'false' if not.",
            relationship.source_entity,
            relationship.target_entity,
            relationship.relationship_type
        );

        match self.llm.chat_complete(&[crate::types::Message::system(&prompt)]).await {
            Ok(response) => {
                let verified = response.trim().to_lowercase().contains("true");
                Ok(verified)
            }
            Err(e) => {
                warn!("Failed to verify relationship: {}", e);
                Ok(false)
            }
        }
    }

    /// Get all discovered relationships
    pub async fn get_relationships(&self) -> Vec<DiscoveredRelationship> {
        self.relationships.read().await.clone()
    }

    /// Get relationships for a specific entity
    pub async fn get_entity_relationships(&self, entity: &str) -> Vec<DiscoveredRelationship> {
        self.relationships.read().await
            .iter()
            .filter(|r| r.source_entity == entity || r.target_entity == entity)
            .cloned()
            .collect()
    }

    /// Get all concept clusters
    pub async fn get_clusters(&self) -> Vec<ConceptCluster> {
        self.clusters.read().await.clone()
    }

    /// Get all semantic bridges
    pub async fn get_bridges(&self) -> Vec<SemanticBridge> {
        self.bridges.read().await.clone()
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> KnowledgeWeaverStats {
        self.stats.read().await.clone()
    }

    /// Force a weave cycle (for testing or manual triggers)
    pub async fn force_weave(&self) -> Result<()> {
        self.weave().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovered_relationship_creation() {
        let rel = DiscoveredRelationship::new(
            "Rust".to_string(),
            "Programming".to_string(),
            RelationshipType::InstanceOf,
            0.9,
            vec!["evidence1".to_string()],
        );

        assert_eq!(rel.source_entity, "Rust");
        assert_eq!(rel.target_entity, "Programming");
        assert_eq!(rel.relationship_type, RelationshipType::InstanceOf);
        assert!(!rel.verified);
    }

    #[test]
    fn test_concept_cluster_creation() {
        let cluster = ConceptCluster {
            id: "test".to_string(),
            name: "Programming Languages".to_string(),
            concepts: vec!["Rust".to_string(), "Python".to_string()],
            centroid_embedding: None,
            created_at: Utc::now(),
            coherence_score: 0.85,
        };

        assert_eq!(cluster.concepts.len(), 2);
        assert_eq!(cluster.coherence_score, 0.85);
    }

    #[test]
    fn test_knowledge_weaver_config_default() {
        let config = KnowledgeWeaverConfig::default();
        assert_eq!(config.weave_interval, Duration::from_secs(1800));
        assert!(config.enable_entity_linking);
        assert!(config.enable_concept_clustering);
        assert!(config.enable_semantic_bridges);
    }
}
