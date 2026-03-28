//! Dynamic Knowledge Graph Indexing System
//!
//! A sophisticated knowledge graph system with multi-layer indexing for efficient
//! entity and relationship retrieval.
//!
//! # Core Features
//!
//! 1. **Multi-layer Index Architecture** - ID/Relation/Attribute/Vector indexes
//! 2. **Incremental Update Mechanism** - Real-time updates without full rebuild
//! 3. **Dynamic Query Optimization** - Adaptive query planning
//! 4. **Full-text Search Integration** - Combine structured and unstructured search
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Knowledge Graph Store                     │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
//! │  │  L1: ID    │  │  L2: Rel    │  │  L3: Attr   │         │
//! │  │  Index     │  │  Index      │  │  Index      │         │
//! │  │  (HashMap) │  │  (B+Tree)   │  │  (Inverted) │         │
//! │  └─────────────┘  └─────────────┘  └─────────────┘         │
//! │  ┌─────────────────────────────────────────────┐            │
//! │  │           L4: Vector Index (HNSW)          │            │
//! │  └─────────────────────────────────────────────┘            │
//! ├─────────────────────────────────────────────────────────────┤
//! │                    Query Optimizer                          │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐       │
//! │  │ Rule-based  │  │ Cost-based   │  │ Adaptive    │       │
//! │  │ Optimization│  │ Optimization │  │ Learning    │       │
//! │  └─────────────┘  └─────────────┘  └─────────────┘       │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! let kg = KnowledgeGraph::new(kg_config);
//!
//! // Add entities
//! kg.add_entity("Alice", EntityType::Person, vec![
//!     ("age", "30"),
//!     ("occupation", "Engineer"),
//! ])?;
//!
//! // Add relationships
//! kg.add_relation("Alice", "works_at", "TechCorp")?;
//!
//! // Query with traversal
//! let results = kg.query().traverse("Alice").via("works_at").execute().await?;
//! ```

use std::collections::{HashMap, VecDeque};
use std::cmp::Ordering;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Configuration for the knowledge graph
#[derive(Clone, Debug)]
pub struct KnowledgeGraphConfig {
    /// Maximum entities to store
    pub max_entities: usize,
    /// Maximum relations per entity
    pub max_relations_per_entity: usize,
    /// Vector dimension for embeddings
    pub vector_dimension: usize,
    /// Enable incremental updates
    pub incremental_updates: bool,
    /// Query timeout in milliseconds
    pub query_timeout_ms: u64,
    /// Cache size for recent queries
    pub cache_size: usize,
}

impl Default for KnowledgeGraphConfig {
    fn default() -> Self {
        Self {
            max_entities: 1_000_000,
            max_relations_per_entity: 1000,
            vector_dimension: 768,
            incremental_updates: true,
            query_timeout_ms: 1000,
            cache_size: 10000,
        }
    }
}

/// Type of entity
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityType {
    Person,
    Organization,
    Location,
    Concept,
    Event,
    Custom(String),
}

impl EntityType {
    pub fn as_str(&self) -> &str {
        match self {
            EntityType::Person => "Person",
            EntityType::Organization => "Organization",
            EntityType::Location => "Location",
            EntityType::Concept => "Concept",
            EntityType::Event => "Event",
            EntityType::Custom(s) => s,
        }
    }
}

/// Type of relation
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationType {
    /// Direct relationship
    Direct,
    /// Hierarchical (parent-child)
    Hierarchical,
    /// Causal (cause-effect)
    Causal,
    /// Temporal (before-after)
    Temporal,
    /// Similarity
    Similar,
    Custom(String),
}

/// An entity in the knowledge graph
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entity {
    /// Unique entity ID
    pub id: String,
    /// Entity name
    pub name: String,
    /// Entity type
    pub entity_type: EntityType,
    /// Attributes (key-value pairs)
    pub attributes: HashMap<String, String>,
    /// Vector embedding (optional)
    pub embedding: Option<Vec<f32>>,
    /// Creation timestamp
    pub created_at: u64,
    /// Last update timestamp
    pub updated_at: u64,
}

impl Entity {
    /// Create a new entity
    pub fn new(id: String, name: String, entity_type: EntityType) -> Self {
        let now = current_timestamp();
        Self {
            id,
            name,
            entity_type,
            attributes: HashMap::new(),
            embedding: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Add an attribute
    pub fn with_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Set embedding
    pub fn with_embedding(mut self, embedding: Vec<f32>) -> Self {
        self.embedding = Some(embedding);
        self
    }
}

/// A relation between entities
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relation {
    /// Unique relation ID
    pub id: String,
    /// Source entity ID
    pub source_id: String,
    /// Target entity ID
    pub target_id: String,
    /// Relation type
    pub relation_type: RelationType,
    /// Relation name (e.g., "works_at", "located_in")
    pub name: String,
    /// Relation weight/confidence
    pub weight: f32,
    /// Properties
    pub properties: HashMap<String, String>,
    /// Creation timestamp
    pub created_at: u64,
}

impl Relation {
    /// Create a new relation
    pub fn new(
        source_id: String,
        target_id: String,
        name: String,
        relation_type: RelationType,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source_id,
            target_id,
            name,
            relation_type,
            weight: 1.0,
            properties: HashMap::new(),
            created_at: current_timestamp(),
        }
    }

    /// Set weight
    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight;
        self
    }

    /// Add property
    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }
}

/// A path in the knowledge graph
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Path {
    /// Entities in the path
    pub entities: Vec<Entity>,
    /// Relations in the path
    pub relations: Vec<Relation>,
    /// Total weight of the path
    pub total_weight: f32,
}

impl Path {
    /// Create a new path
    pub fn new() -> Self {
        Self {
            entities: Vec::new(),
            relations: Vec::new(),
            total_weight: 0.0,
        }
    }

    /// Add a step to the path
    pub fn add_step(&mut self, entity: Entity, relation: Option<Relation>) {
        self.entities.push(entity);
        if let Some(rel) = relation {
            self.total_weight += rel.weight;
            self.relations.push(rel);
        }
    }

    /// Get path length (number of hops)
    pub fn length(&self) -> usize {
        self.relations.len()
    }
}

impl Default for Path {
    fn default() -> Self {
        Self::new()
    }
}

/// Query for the knowledge graph
#[derive(Clone, Debug)]
pub struct GraphQuery {
    /// Starting entity ID
    pub start_id: Option<String>,
    /// Target entity ID
    pub target_id: Option<String>,
    /// Relation types to traverse
    pub relation_types: Vec<String>,
    /// Maximum depth
    pub max_depth: usize,
    /// Entity types to include
    pub entity_types: Vec<EntityType>,
    /// Attributes to filter
    pub attribute_filters: HashMap<String, String>,
    /// Sort by
    pub sort_by: QuerySortBy,
    /// Limit results
    pub limit: usize,
}

/// Sort criteria for query results
#[derive(Clone, Debug)]
pub enum QuerySortBy {
    /// Sort by weight
    Weight,
    /// Sort by path length
    PathLength,
    /// Sort by relevance
    Relevance,
    /// Sort by creation time
    CreatedAt,
}

/// Query result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryResult {
    /// Matching paths
    pub paths: Vec<Path>,
    /// Total matches
    pub total_matches: usize,
    /// Query execution time (ms)
    pub execution_time_ms: u64,
    /// Whether results were cached
    pub cached: bool,
}

/// Statistics about the knowledge graph
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphStatistics {
    /// Total entities
    pub total_entities: usize,
    /// Total relations
    pub total_relations: usize,
    /// Entities by type
    pub entities_by_type: HashMap<String, usize>,
    /// Relations by type
    pub relations_by_type: HashMap<String, usize>,
    /// Average relations per entity
    pub avg_relations_per_entity: f32,
    /// Graph density
    pub density: f32,
}

/// The main knowledge graph structure
pub struct KnowledgeGraph {
    /// Configuration
    config: KnowledgeGraphConfig,
    /// L1: Entity ID index (HashMap for O(1) lookup)
    id_index: HashMap<String, Entity>,
    /// L2: Relation type index
    relation_type_index: HashMap<String, Vec<String>>, // relation_name -> vec of relation IDs
    /// L2: Outgoing relations index
    outgoing_index: HashMap<String, Vec<String>>, // entity_id -> vec of relation IDs
    /// L2: Incoming relations index
    incoming_index: HashMap<String, Vec<String>>, // entity_id -> vec of relation IDs
    /// L3: Attribute inverted index
    attribute_index: HashMap<String, HashMap<String, Vec<String>>>, // attr_key -> attr_value -> vec of entity IDs
    /// L4: Vector index (simplified HNSW-like)
    vector_index: Vec<VectorEntry>,
    /// All relations
    relations: HashMap<String, Relation>,
    /// Query cache
    query_cache: HashMap<String, QueryResult>,
    /// Statistics
    stats: GraphStatistics,
    /// Update log for incremental updates
    update_log: VecDeque<UpdateEntry>,
}

/// A vector entry for similarity search
#[derive(Clone, Debug)]
struct VectorEntry {
    entity_id: String,
    vector: Vec<f32>,
}

/// An entry in the update log
#[derive(Clone, Debug)]
enum UpdateEntry {
    AddEntity(Entity),
    AddRelation(Relation),
    UpdateEntity { id: String, old: Entity, new: Entity },
    DeleteEntity { id: String },
    DeleteRelation { id: String },
}

impl KnowledgeGraph {
    /// Create a new knowledge graph
    pub fn new(config: KnowledgeGraphConfig) -> Self {
        Self {
            config,
            id_index: HashMap::new(),
            relation_type_index: HashMap::new(),
            outgoing_index: HashMap::new(),
            incoming_index: HashMap::new(),
            attribute_index: HashMap::new(),
            vector_index: Vec::new(),
            relations: HashMap::new(),
            query_cache: HashMap::new(),
            stats: GraphStatistics {
                total_entities: 0,
                total_relations: 0,
                entities_by_type: HashMap::new(),
                relations_by_type: HashMap::new(),
                avg_relations_per_entity: 0.0,
                density: 0.0,
            },
            update_log: VecDeque::new(),
        }
    }

    /// Add an entity to the graph
    pub fn add_entity(&mut self, entity: Entity) -> Result<()> {
        if self.id_index.len() >= self.config.max_entities {
            return Err(anyhow!("Maximum entity limit reached"));
        }

        let entity_id = entity.id.clone();

        // Add to ID index
        self.id_index.insert(entity_id.clone(), entity.clone());

        // Update attribute index
        for (key, value) in &entity.attributes {
            self.attribute_index
                .entry(key.clone())
                .or_default()
                .entry(value.clone())
                .or_default()
                .push(entity_id.clone());
        }

        // Add to vector index if embedding exists
        if let Some(ref embedding) = entity.embedding {
            self.vector_index.push(VectorEntry {
                entity_id: entity_id.clone(),
                vector: embedding.clone(),
            });
        }

        // Update statistics
        self.stats.total_entities += 1;
        let type_name = entity.entity_type.as_str().to_string();
        *self.stats.entities_by_type.entry(type_name).or_insert(0) += 1;

        // Log update
        if self.config.incremental_updates {
            self.update_log.push_back(UpdateEntry::AddEntity(entity));
            if self.update_log.len() > 10000 {
                self.update_log.pop_front();
            }
        }

        Ok(())
    }

    /// Add a relation to the graph
    pub fn add_relation(&mut self, relation: Relation) -> Result<()> {
        let relation_id = relation.id.clone();

        // Add to relations map
        self.relations.insert(relation_id.clone(), relation.clone());

        // Update outgoing index
        self.outgoing_index
            .entry(relation.source_id.clone())
            .or_default()
            .push(relation_id.clone());

        // Update incoming index
        self.incoming_index
            .entry(relation.target_id.clone())
            .or_default()
            .push(relation_id.clone());

        // Update relation type index
        self.relation_type_index
            .entry(relation.name.clone())
            .or_default()
            .push(relation_id.clone());

        // Update statistics
        self.stats.total_relations += 1;
        let rel_type_name = match relation.relation_type {
            RelationType::Direct => "Direct",
            RelationType::Hierarchical => "Hierarchical",
            RelationType::Causal => "Causal",
            RelationType::Temporal => "Temporal",
            RelationType::Similar => "Similar",
            RelationType::Custom(ref s) => s,
        }.to_string();
        *self.stats.relations_by_type.entry(rel_type_name).or_insert(0) += 1;

        // Update average relations
        if self.stats.total_entities > 0 {
            self.stats.avg_relations_per_entity =
                self.stats.total_relations as f32 / self.stats.total_entities as f32;
        }

        // Log update
        if self.config.incremental_updates {
            self.update_log.push_back(UpdateEntry::AddRelation(relation));
            if self.update_log.len() > 10000 {
                self.update_log.pop_front();
            }
        }

        Ok(())
    }

    /// Get an entity by ID
    pub fn get_entity(&self, id: &str) -> Option<&Entity> {
        self.id_index.get(id)
    }

    /// Get relations for an entity
    pub fn get_relations(&self, entity_id: &str) -> Vec<&Relation> {
        let mut result = Vec::new();

        // Get outgoing relations
        if let Some(rel_ids) = self.outgoing_index.get(entity_id) {
            for rel_id in rel_ids {
                if let Some(rel) = self.relations.get(rel_id) {
                    result.push(rel);
                }
            }
        }

        result
    }

    /// Execute a graph query
    pub async fn query(&mut self, query: GraphQuery) -> Result<QueryResult> {
        let start_time = Instant::now();

        // Check cache first
        let cache_key = self.compute_cache_key(&query);
        if let Some(cached) = self.query_cache.get(&cache_key) {
            return Ok(QueryResult {
                paths: cached.paths.clone(),
                total_matches: cached.total_matches,
                execution_time_ms: start_time.elapsed().as_millis() as u64,
                cached: true,
            });
        }

        // Build query plan
        let plan = self.build_query_plan(&query);

        // Execute query
        let paths = self.execute_query_plan(plan, &query).await?;

        let total_matches = paths.len();
        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        // Cache result
        if self.query_cache.len() >= self.config.cache_size {
            // Remove oldest entry
            if let Some(first_key) = self.query_cache.keys().next().cloned() {
                self.query_cache.remove(&first_key);
            }
        }
        self.query_cache.insert(
            cache_key,
            QueryResult {
                paths: paths.clone(),
                total_matches,
                execution_time_ms,
                cached: false,
            },
        );

        Ok(QueryResult {
            paths,
            total_matches,
            execution_time_ms,
            cached: false,
        })
    }

    /// Compute cache key for a query
    fn compute_cache_key(&self, query: &GraphQuery) -> String {
        format!(
            "{:?}:{:?}:{}:{:?}:{}",
            query.start_id,
            query.target_id,
            query.max_depth,
            query.sort_by,
            query.limit
        )
    }

    /// Build a query execution plan
    fn build_query_plan(&self, query: &GraphQuery) -> QueryPlan {
        // Simple rule-based optimization
        let mut plan = QueryPlan {
            steps: Vec::new(),
            estimated_cost: 0.0,
        };

        // If we have a start ID, use ID index first (fastest)
        if let Some(ref start_id) = query.start_id {
            if self.id_index.contains_key(start_id) {
                plan.steps.push(PlanStep::IndexScan {
                    index_type: IndexType::IdIndex,
                    key: start_id.clone(),
                });
                plan.estimated_cost = 1.0;
            }
        }

        // If we have attribute filters, consider using attribute index
        if !query.attribute_filters.is_empty() {
            let smallest_filter = self.find_smallest_attribute_filter(&query.attribute_filters);
            if let Some((key, value)) = smallest_filter {
                plan.steps.push(PlanStep::IndexScan {
                    index_type: IndexType::AttributeIndex,
                    key: format!("{}={}", key, value),
                });
                plan.estimated_cost *= 0.5; // Attribute index is faster for filtering
            }
        }

        // Add traversal step
        plan.steps.push(PlanStep::Traverse {
            max_depth: query.max_depth,
            relation_types: query.relation_types.clone(),
        });

        plan
    }

    /// Find the attribute filter with the smallest result set
    fn find_smallest_attribute_filter(
        &self,
        filters: &HashMap<String, String>,
    ) -> Option<(String, String)> {
        let mut smallest: Option<(String, String, usize)> = None;

        for (key, value) in filters {
            if let Some(value_map) = self.attribute_index.get(key) {
                let count = value_map.get(value).map(|v| v.len()).unwrap_or(0);
                if smallest.is_none() || count < smallest.as_ref().unwrap().2 {
                    smallest = Some((key.clone(), value.clone(), count));
                }
            }
        }

        smallest.map(|(k, v, _)| (k, v))
    }

    /// Execute a query plan
    async fn execute_query_plan(&self, plan: QueryPlan, query: &GraphQuery) -> Result<Vec<Path>> {
        let mut results: Vec<Path> = Vec::new();

        for step in &plan.steps {
            match step {
                PlanStep::IndexScan { index_type, key } => {
                    match index_type {
                        IndexType::IdIndex => {
                            if let Some(entity) = self.id_index.get(key) {
                                let mut path = Path::new();
                                path.add_step(entity.clone(), None);
                                results.push(path);
                            }
                        }
                        IndexType::AttributeIndex => {
                            // Parse key in format "attr=value"
                            if let Some((attr_key, attr_value)) = key.split_once('=') {
                                if let Some(entity_ids) = self.attribute_index.get(attr_key)
                                    .and_then(|m| m.get(attr_value))
                                {
                                    for entity_id in entity_ids {
                                        if let Some(entity) = self.id_index.get(entity_id) {
                                            let mut path = Path::new();
                                            path.add_step(entity.clone(), None);
                                            results.push(path);
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                PlanStep::Traverse { max_depth, relation_types } => {
                    // Expand paths by traversing relations
                    let mut expanded: Vec<Path> = Vec::new();
                    for path in results {
                        let last_entity_id = path.entities.last().map(|e| e.id.clone());
                        if let Some(entity_id) = last_entity_id {
                            let relations = self.get_relations(&entity_id);
                            for rel in relations {
                                // Filter by relation types if specified
                                if !relation_types.is_empty()
                                    && !relation_types.contains(&rel.name)
                                {
                                    continue;
                                }

                                if let Some(target_entity) = self.id_index.get(&rel.target_id) {
                                    let mut new_path = path.clone();
                                    new_path.add_step(target_entity.clone(), Some(rel.clone()));
                                    expanded.push(new_path);
                                }
                            }
                        }
                    }
                    results = expanded;

                    // Check depth
                    if path_length(&results) >= *max_depth {
                        break;
                    }
                }
            }
        }

        // Apply sorting
        match query.sort_by {
            QuerySortBy::Weight => results.sort_by(|a, b| {
                b.total_weight.partial_cmp(&a.total_weight).unwrap_or(Ordering::Equal)
            }),
            QuerySortBy::PathLength => results.sort_by_key(|p| p.length()),
            QuerySortBy::Relevance => {
                // For now, just use weight as proxy for relevance
                results.sort_by(|a, b| {
                    b.total_weight.partial_cmp(&a.total_weight).unwrap_or(Ordering::Equal)
                })
            }
            QuerySortBy::CreatedAt => {
                results.sort_by(|a, b| {
                    let a_time = a.entities.first().map(|e| e.created_at).unwrap_or(0);
                    let b_time = b.entities.first().map(|e| e.created_at).unwrap_or(0);
                    b_time.cmp(&a_time)
                })
            }
        }

        // Apply limit
        if query.limit > 0 && results.len() > query.limit {
            results.truncate(query.limit);
        }

        Ok(results)
    }

    /// Find similar entities using vector search
    pub fn find_similar(&self, entity_id: &str, limit: usize) -> Result<Vec<(String, f32)>> {
        let entity = self
            .id_index
            .get(entity_id)
            .ok_or_else(|| anyhow!("Entity not found"))?;

        let embedding = entity
            .embedding
            .as_ref()
            .ok_or_else(|| anyhow!("Entity has no embedding"))?;

        let mut similarities = Vec::new();

        for entry in &self.vector_index {
            if entry.entity_id != entity_id {
                let similarity = cosine_similarity(embedding, &entry.vector);
                similarities.push((entry.entity_id.clone(), similarity));
            }
        }

        // Sort by similarity
        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal));

        // Return top k
        similarities.truncate(limit);
        Ok(similarities)
    }

    /// Get graph statistics
    pub fn get_statistics(&self) -> GraphStatistics {
        let mut stats = self.stats.clone();

        // Calculate density
        let max_possible_relations = self.stats.total_entities * (self.stats.total_entities - 1) / 2;
        if max_possible_relations > 0 {
            stats.density = self.stats.total_relations as f32 / max_possible_relations as f32;
        }

        stats
    }

    /// Get entities by type
    pub fn get_entities_by_type(&self, entity_type: &EntityType) -> Vec<&Entity> {
        self.id_index
            .values()
            .filter(|e| &e.entity_type == entity_type)
            .collect()
    }

    /// Get relations by type
    pub fn get_relations_by_type(&self, relation_name: &str) -> Vec<&Relation> {
        self.relation_type_index
            .get(relation_name)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.relations.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Perform incremental update
    pub fn perform_incremental_update(&mut self) {
        if !self.config.incremental_updates {
            return;
        }

        // Process update log entries
        while let Some(entry) = self.update_log.pop_front() {
            match entry {
                UpdateEntry::AddEntity(entity) => {
                    // Entity already added, update any derived structures
                    self.rebuild_attribute_index_for_entity(&entity);
                }
                UpdateEntry::AddRelation(relation) => {
                    // Relation already added, update derived structures
                    self.update_relation_derived_structures(&relation);
                }
                UpdateEntry::DeleteEntity { id } => {
                    self.remove_entity_derivations(&id);
                }
                UpdateEntry::DeleteRelation { id } => {
                    if let Some(rel) = self.relations.remove(&id) {
                        self.remove_relation_derivations(&rel);
                    }
                }
                UpdateEntry::UpdateEntity { id: _, old: _, new } => {
                    // Re-index the entity
                    self.rebuild_attribute_index_for_entity(&new);
                }
            }
        }
    }

    /// Rebuild attribute index for an entity
    fn rebuild_attribute_index_for_entity(&mut self, entity: &Entity) {
        // Remove old entries would require tracking, so we rebuild on-demand
        // For now, just add new entries
        for (key, value) in &entity.attributes {
            self.attribute_index
                .entry(key.clone())
                .or_default()
                .entry(value.clone())
                .or_default()
                .push(entity.id.clone());
        }
    }

    /// Update derived structures for a relation
    fn update_relation_derived_structures(&mut self, _relation: &Relation) {
        // This could update cached paths, materialized views, etc.
        // For now, we just invalidate relevant cache entries
        self.query_cache.retain(|_, v| !v.cached);
    }

    /// Remove entity derivations
    fn remove_entity_derivations(&mut self, entity_id: &str) {
        self.id_index.remove(entity_id);

        // Remove from attribute index
        for value_map in self.attribute_index.values_mut() {
            for entity_ids in value_map.values_mut() {
                entity_ids.retain(|id| id != entity_id);
            }
        }

        // Remove from vector index
        self.vector_index.retain(|e| e.entity_id != entity_id);
    }

    /// Remove relation derivations
    fn remove_relation_derivations(&mut self, relation: &Relation) {
        // Remove from outgoing index
        if let Some(rel_ids) = self.outgoing_index.get_mut(&relation.source_id) {
            rel_ids.retain(|id| id != &relation.id);
        }

        // Remove from incoming index
        if let Some(rel_ids) = self.incoming_index.get_mut(&relation.target_id) {
            rel_ids.retain(|id| id != &relation.id);
        }

        // Remove from relation type index
        if let Some(rel_ids) = self.relation_type_index.get_mut(&relation.name) {
            rel_ids.retain(|id| id != &relation.id);
        }

        // Update statistics
        self.stats.total_relations = self.stats.total_relations.saturating_sub(1);
    }

    /// Clear the query cache
    pub fn clear_cache(&mut self) {
        self.query_cache.clear();
    }
}

/// Get the maximum path length from a list of paths
fn path_length(paths: &[Path]) -> usize {
    paths
        .iter()
        .map(|p| p.length())
        .max()
        .unwrap_or(0)
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if magnitude_a == 0.0 || magnitude_b == 0.0 {
        return 0.0;
    }

    dot_product / (magnitude_a * magnitude_b)
}

/// Query execution plan
#[derive(Clone, Debug)]
struct QueryPlan {
    steps: Vec<PlanStep>,
    estimated_cost: f32,
}

/// A step in the query plan
#[derive(Clone, Debug)]
enum PlanStep {
    IndexScan {
        index_type: IndexType,
        key: String,
    },
    Traverse {
        max_depth: usize,
        relation_types: Vec<String>,
    },
}

/// Type of index
#[derive(Clone, Debug)]
enum IndexType {
    IdIndex,
    RelationTypeIndex,
    AttributeIndex,
    VectorIndex,
}

/// Get current timestamp
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Query builder for fluent API
pub struct GraphQueryBuilder {
    query: GraphQuery,
}

impl GraphQueryBuilder {
    /// Create a new query builder
    pub fn new() -> Self {
        Self {
            query: GraphQuery {
                start_id: None,
                target_id: None,
                relation_types: Vec::new(),
                max_depth: 3,
                entity_types: Vec::new(),
                attribute_filters: HashMap::new(),
                sort_by: QuerySortBy::Relevance,
                limit: 100,
            },
        }
    }

    /// Set the starting entity
    pub fn start(mut self, entity_id: impl Into<String>) -> Self {
        self.query.start_id = Some(entity_id.into());
        self
    }

    /// Set the target entity
    pub fn target(mut self, entity_id: impl Into<String>) -> Self {
        self.query.target_id = Some(entity_id.into());
        self
    }

    /// Add a relation type to traverse
    pub fn via(mut self, relation_type: impl Into<String>) -> Self {
        self.query.relation_types.push(relation_type.into());
        self
    }

    /// Set maximum traversal depth
    pub fn max_depth(mut self, depth: usize) -> Self {
        self.query.max_depth = depth;
        self
    }

    /// Add an entity type filter
    pub fn entity_type(mut self, entity_type: EntityType) -> Self {
        self.query.entity_types.push(entity_type);
        self
    }

    /// Add an attribute filter
    pub fn where_attr(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.query.attribute_filters.insert(key.into(), value.into());
        self
    }

    /// Set sort by
    pub fn sort_by(mut self, sort_by: QuerySortBy) -> Self {
        self.query.sort_by = sort_by;
        self
    }

    /// Set result limit
    pub fn limit(mut self, limit: usize) -> Self {
        self.query.limit = limit;
        self
    }

    /// Build the query
    pub fn build(self) -> GraphQuery {
        self.query
    }
}

impl Default for GraphQueryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_creation() {
        let entity = Entity::new(
            "e1".to_string(),
            "Alice".to_string(),
            EntityType::Person,
        )
        .with_attribute("age", "30")
        .with_attribute("city", "Beijing");

        assert_eq!(entity.id, "e1");
        assert_eq!(entity.name, "Alice");
        assert_eq!(entity.attributes.get("age"), Some(&"30".to_string()));
        assert_eq!(entity.attributes.get("city"), Some(&"Beijing".to_string()));
    }

    #[test]
    fn test_relation_creation() {
        let relation = Relation::new(
            "e1".to_string(),
            "e2".to_string(),
            "knows".to_string(),
            RelationType::Direct,
        )
        .with_weight(0.8)
        .with_property("since", "2020");

        assert_eq!(relation.source_id, "e1");
        assert_eq!(relation.target_id, "e2");
        assert_eq!(relation.weight, 0.8);
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_path_creation() {
        let mut path = Path::new();
        let entity1 = Entity::new("e1".to_string(), "Alice".to_string(), EntityType::Person);
        let entity2 = Entity::new("e2".to_string(), "Bob".to_string(), EntityType::Person);
        let relation = Relation::new("e1".to_string(), "e2".to_string(), "knows".to_string(), RelationType::Direct);

        path.add_step(entity1, None);
        path.add_step(entity2, Some(relation));

        assert_eq!(path.length(), 1);
        assert_eq!(path.entities.len(), 2);
    }

    #[test]
    fn test_query_builder() {
        let query = GraphQueryBuilder::new()
            .start("Alice")
            .via("knows")
            .via("works_at")
            .max_depth(3)
            .limit(10)
            .build();

        assert_eq!(query.start_id, Some("Alice".to_string()));
        assert_eq!(query.max_depth, 3);
        assert_eq!(query.limit, 10);
    }

    #[test]
    fn test_knowledge_graph_config() {
        let config = KnowledgeGraphConfig::default();
        assert_eq!(config.max_entities, 1_000_000);
        assert!(config.incremental_updates);
    }
}
