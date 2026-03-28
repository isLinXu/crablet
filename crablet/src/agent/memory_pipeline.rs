//! Agent Memory Pipeline - Unified Memory System
//!
//! 融合工作记忆 + 情节记忆 + 语义记忆的统一管道
//!
//! # 核心特性
//! - 工作记忆: Harness 实时上下文
//! - 情节记忆: 执行历史片段自动存储
//! - 语义记忆: 跨任务模式提取
//! - 支持向量检索 + 精确检索融合
//!
//! # 架构
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    Unified Agent Memory Pipeline                       │
//! ├─────────────────────────────────────────────────────────────────────┤
//! │                                                                       │
//! │   ┌──────────────┐    ┌──────────────┐    ┌──────────────┐        │
//! │   │   Working    │    │   Episodic   │    │   Semantic   │        │
//! │   │   Memory     │    │   Memory     │    │   Memory     │        │
//! │   │  (Harness)   │    │  (History)   │    │  (Patterns)  │        │
//! │   └──────┬───────┘    └──────┬───────┘    └──────┬───────┘        │
//! │          │                    │                    │                 │
//! │          └────────────────────┼────────────────────┘                 │
//! │                               │                                      │
//! │                    ┌──────────▼──────────┐                          │
//! │                    │  Memory Weaver      │                          │
//! │                    │  (Pattern Mining)   │                          │
//! │                    └──────────┬──────────┘                          │
//! │                               │                                      │
//! │                    ┌──────────▼──────────┐                          │
//! │                    │ Unified Retrieval   │                          │
//! │                    │ Vector + Exact     │                          │
//! │                    │ Score Fusion       │                          │
//! │                    └────────────────────┘                          │
//! │                                                                       │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug, Clone)]
pub enum MemoryPipelineError {
    #[error("Memory operation failed: {0}")]
    OperationFailed(String),

    #[error("Memory not found: {0}")]
    NotFound(String),

    #[error("Invalid memory type: {0}")]
    InvalidType(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Retrieval error: {0}")]
    RetrievalError(String),
}

// ============================================================================
// Memory Types
// ============================================================================

/// Types of memory in the pipeline
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MemoryType {
    Working,    // Current execution context
    Episodic,   // Historical execution episodes
    Semantic,   // Cross-task patterns
}

/// Memory content wrapper
#[derive(Debug, Clone)]
pub enum MemoryContent {
    Working(WorkingMemoryEntry),
    Episodic(EpisodicMemoryEntry),
    Semantic(SemanticMemoryEntry),
}

/// A single memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub memory_type: MemoryType,
    pub content: String,
    pub importance: f64,           // 0.0 - 1.0
    pub access_count: u64,
    pub last_accessed: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub tags: Vec<String>,
    pub metadata: HashMap<String, String>,
}

impl MemoryEntry {
    pub fn new(memory_type: MemoryType, content: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            memory_type,
            content,
            importance: 0.5,
            access_count: 0,
            last_accessed: now,
            created_at: now,
            tags: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_importance(mut self, importance: f64) -> Self {
        self.importance = importance;
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn access(&mut self) {
        self.access_count += 1;
        self.last_accessed = chrono::Utc::now();
    }
}

// ============================================================================
// Working Memory Entry
// ============================================================================

/// Entry from the harness working memory
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkingMemoryEntry {
    pub session_id: String,
    pub step_count: usize,
    pub tool_call_count: usize,
    pub tool_failure_count: usize,
    pub llm_tokens_used: Option<u64>,
    pub current_thought: Option<String>,
    pub last_tool: Option<String>,
    pub last_tool_args: Option<String>,
    pub last_tool_result: Option<String>,
    pub resource_usage: ResourceSnapshot,
    pub error_history: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    pub memory_bytes: u64,
    pub cpu_time_ms: u64,
    pub network_requests: u64,
}

// ============================================================================
// Episodic Memory Entry
// ============================================================================

/// Entry from episodic memory (execution episodes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicMemoryEntry {
    pub episode_id: String,
    pub session_id: String,
    pub task_description: String,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub steps: Vec<EpisodeStep>,
    pub outcome: EpisodeOutcome,
    pub summary: String,
    pub lessons_learned: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeStep {
    pub step_number: usize,
    pub thought: String,
    pub action: String,
    pub observation: String,
    pub duration_ms: u64,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EpisodeOutcome {
    Success,
    PartialSuccess { completion_rate: f64 },
    Failure { reason: String },
    Timeout,
    Cancelled,
}

// ============================================================================
// Semantic Memory Entry
// ============================================================================

/// Entry from semantic memory (cross-task patterns)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMemoryEntry {
    pub pattern_id: String,
    pub pattern_type: PatternType,
    pub pattern_content: String,
    pub occurrence_count: u64,
    pub first_seen: chrono::DateTime<chrono::Utc>,
    pub last_seen: chrono::DateTime<chrono::Utc>,
    pub success_rate: f64,
    pub avg_duration_ms: Option<u64>,
    pub related_tasks: Vec<String>,
    pub extracted_rules: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatternType {
    ToolSequence,      // Common tool call sequences
    ErrorPattern,      // Recurring error patterns
    SolutionStrategy,  // Successful solution approaches
    ContextPattern,    // Context-dependent patterns
    UserPreference,    // User-specific preferences
}

// ============================================================================
// Unified Retrieval
// ============================================================================

/// Search mode for unified retrieval
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SearchMode {
    Exact,              // Keyword-based exact match
    Semantic,           // Vector similarity search
    Hybrid { alpha: f64 }, // α * semantic + (1-α) * exact
}

impl Default for SearchMode {
    fn default() -> Self {
        SearchMode::Hybrid { alpha: 0.6 }
    }
}

/// Unified search result
#[derive(Debug, Clone)]
pub struct UnifiedSearchResult {
    pub entries: Vec<SearchResultEntry>,
    pub total_score: f64,
    pub retrieval_time_ms: u64,
}

#[derive(Debug, Clone)]
pub struct SearchResultEntry {
    pub entry: MemoryEntry,
    pub memory_type: MemoryType,
    pub exact_score: f64,
    pub semantic_score: f64,
    pub final_score: f64,
}

/// Search query
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub query: String,
    pub mode: SearchMode,
    pub memory_types: Option<Vec<MemoryType>>,
    pub limit: usize,
    pub min_score: f64,
    pub tags: Option<Vec<String>>,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            mode: SearchMode::default(),
            memory_types: None,
            limit: 10,
            min_score: 0.3,
            tags: None,
        }
    }
}

// ============================================================================
// Memory Pipeline Core
// ============================================================================

pub struct MemoryPipeline {
    /// Working memory entries (short-term)
    working_memory: Arc<RwLock<VecDeque<MemoryEntry>>>,
    /// Episodic memory entries (medium-term)
    episodic_memory: Arc<RwLock<Vec<MemoryEntry>>>,
    /// Semantic memory entries (long-term)
    semantic_memory: Arc<RwLock<Vec<MemoryEntry>>>,
    /// Pattern extraction cache
    pattern_cache: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// Access statistics
    access_stats: Arc<RwLock<AccessStatistics>>,
    /// Configuration
    config: PipelineConfig,
    /// Event sender for pipeline events
    event_tx: tokio::sync::broadcast::Sender<PipelineEvent>,
}

#[derive(Debug, Clone, Default)]
pub struct AccessStatistics {
    pub total_searches: u64,
    pub total_hits: u64,
    pub by_memory_type: HashMap<MemoryType, TypeStats>,
}

#[derive(Debug, Clone, Default)]
pub struct TypeStats {
    pub accesses: u64,
    pub hits: u64,
    pub avg_latency_ms: f64,
}

#[derive(Debug, Clone)]
pub enum PipelineEvent {
    MemoryStored { memory_type: MemoryType, entry_id: String },
    MemoryAccessed { memory_type: MemoryType, entry_id: String },
    PatternExtracted { pattern_type: PatternType, pattern_id: String },
    MemoryConsolidated { from_type: MemoryType, to_type: MemoryType, count: usize },
    SearchCompleted { mode: SearchMode, results_count: usize, duration_ms: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineConfig {
    pub working_memory_capacity: usize,
    pub episodic_memory_max_size: usize,
    pub semantic_memory_max_size: usize,
    pub auto_consolidate: bool,
    pub consolidation_threshold: usize,
    pub pattern_extraction_enabled: bool,
    pub importance_decay_rate: f64,
    pub tag_extraction_enabled: bool,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            working_memory_capacity: 100,
            episodic_memory_max_size: 1000,
            semantic_memory_max_size: 5000,
            auto_consolidate: true,
            consolidation_threshold: 50,
            pattern_extraction_enabled: true,
            importance_decay_rate: 0.95,
            tag_extraction_enabled: true,
        }
    }
}

impl MemoryPipeline {
    /// Create a new memory pipeline
    pub fn new(config: PipelineConfig) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(1000);

        Self {
            working_memory: Arc::new(RwLock::new(VecDeque::with_capacity(config.working_memory_capacity))),
            episodic_memory: Arc::new(RwLock::new(Vec::with_capacity(config.episodic_memory_max_size))),
            semantic_memory: Arc::new(RwLock::new(Vec::with_capacity(config.semantic_memory_max_size))),
            pattern_cache: Arc::new(RwLock::new(HashMap::new())),
            access_stats: Arc::new(RwLock::new(AccessStatistics::default())),
            config,
            event_tx,
        }
    }

    /// Create with default configuration
    pub fn with_default() -> Self {
        Self::new(PipelineConfig::default())
    }

    /// Subscribe to pipeline events
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<PipelineEvent> {
        self.event_tx.subscribe()
    }

    // --- Storage Operations ---

    /// Store a working memory entry
    pub async fn store_working(&self, entry: WorkingMemoryEntry) -> String {
        let memory_entry = MemoryEntry::new(
            MemoryType::Working,
            serde_json::to_string(&entry).unwrap_or_default(),
        );

        let id = memory_entry.id.clone();

        let mut working = self.working_memory.write().await;
        working.push_back(memory_entry.clone());

        // Enforce capacity
        while working.len() > self.config.working_memory_capacity {
            working.pop_front();
        }

        let _ = self.event_tx.send(PipelineEvent::MemoryStored {
            memory_type: MemoryType::Working,
            entry_id: id.clone(),
        });

        id
    }

    /// Store an episodic memory entry
    pub async fn store_episodic(&self, entry: EpisodicMemoryEntry) -> String {
        let memory_entry = MemoryEntry::new(
            MemoryType::Episodic,
            serde_json::to_string(&entry).unwrap_or_default(),
        );

        let id = memory_entry.id.clone();

        let mut episodic = self.episodic_memory.write().await;
        episodic.push(memory_entry.clone());

        // Enforce max size (remove oldest if needed)
        while episodic.len() > self.config.episodic_memory_max_size {
            episodic.remove(0);
        }

        let _ = self.event_tx.send(PipelineEvent::MemoryStored {
            memory_type: MemoryType::Episodic,
            entry_id: id.clone(),
        });

        // Try to extract patterns
        if self.config.pattern_extraction_enabled {
            self.extract_patterns_from_episode(&entry).await;
        }

        id
    }

    /// Store a semantic memory entry
    pub async fn store_semantic(&self, entry: SemanticMemoryEntry) -> String {
        let memory_entry = MemoryEntry::new(
            MemoryType::Semantic,
            serde_json::to_string(&entry).unwrap_or_default(),
        );

        let id = memory_entry.id.clone();

        let mut semantic = self.semantic_memory.write().await;
        semantic.push(memory_entry.clone());

        // Enforce max size
        while semantic.len() > self.config.semantic_memory_max_size {
            semantic.remove(0);
        }

        let _ = self.event_tx.send(PipelineEvent::MemoryStored {
            memory_type: MemoryType::Semantic,
            entry_id: id.clone(),
        });

        id
    }

    // --- Retrieval Operations ---

    /// Unified search across all memory types
    pub async fn search(&self, query: SearchQuery) -> UnifiedSearchResult {
        let start = Instant::now();

        let mut all_entries = Vec::new();

        // Collect entries from each memory type
        if query.memory_types.is_none() || query.memory_types.as_ref().unwrap().contains(&MemoryType::Working) {
            let working = self.working_memory.read().await;
            for entry in working.iter().cloned() {
                all_entries.push((entry, MemoryType::Working));
            }
        }

        if query.memory_types.is_none() || query.memory_types.as_ref().unwrap().contains(&MemoryType::Episodic) {
            let episodic = self.episodic_memory.read().await;
            for entry in episodic.iter().cloned() {
                all_entries.push((entry, MemoryType::Episodic));
            }
        }

        if query.memory_types.is_none() || query.memory_types.as_ref().unwrap().contains(&MemoryType::Semantic) {
            let semantic = self.semantic_memory.read().await;
            for entry in semantic.iter().cloned() {
                all_entries.push((entry, MemoryType::Semantic));
            }
        }

        // Calculate scores
        let mut scored_entries = Vec::new();

        for (mut entry, memory_type) in all_entries {
            // Filter by tags if specified
            if let Some(ref query_tags) = query.tags {
                if !query_tags.iter().any(|t| entry.tags.contains(t)) {
                    continue;
                }
            }

            // Calculate exact match score (simple keyword overlap)
            let exact_score = self.calculate_exact_score(&entry.content, &query.query);

            // Calculate semantic score (placeholder - would use embeddings)
            let semantic_score = self.calculate_semantic_score(&entry.content, &query.query);

            // Calculate final score based on mode
            let final_score = match query.mode {
                SearchMode::Exact => exact_score,
                SearchMode::Semantic => semantic_score,
                SearchMode::Hybrid { alpha } => alpha * semantic_score + (1.0 - alpha) * exact_score,
            };

            if final_score >= query.min_score {
                entry.access();
                let _ = self.event_tx.send(PipelineEvent::MemoryAccessed {
                    memory_type,
                    entry_id: entry.id.clone(),
                });

                scored_entries.push(SearchResultEntry {
                    entry,
                    memory_type,
                    exact_score,
                    semantic_score,
                    final_score,
                });
            }
        }

        // Sort by final score
        scored_entries.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap());

        // Limit results
        let results: Vec<_> = scored_entries.into_iter().take(query.limit).collect();
        let count = results.len();

        let total_score: f64 = results.iter().map(|r| r.final_score).sum();

        let duration_ms = start.elapsed().as_millis() as u64;

        // Update stats
        {
            let mut stats = self.access_stats.write().await;
            stats.total_searches += 1;
            stats.total_hits += count as u64;
        }

        let _ = self.event_tx.send(PipelineEvent::SearchCompleted {
            mode: query.mode,
            results_count: count,
            duration_ms,
        });

        UnifiedSearchResult {
            entries: results,
            total_score,
            retrieval_time_ms: duration_ms,
        }
    }

    /// Calculate exact match score (keyword overlap)
    fn calculate_exact_score(&self, content: &str, query: &str) -> f64 {
        let content_lower = content.to_lowercase();
        let query_lower = query.to_lowercase();

        let query_words: HashSet<&str> = query_lower.split_whitespace().collect();
        let content_words: HashSet<&str> = content_lower.split_whitespace().collect();

        if query_words.is_empty() {
            return 0.0;
        }

        let intersection = query_words.intersection(&content_words).count();
        intersection as f64 / query_words.len() as f64
    }

    /// Calculate semantic score (placeholder - would use actual embeddings)
    fn calculate_semantic_score(&self, _content: &str, _query: &str) -> f64 {
        // TODO: Integrate with actual embedding model
        // For now, return a random score between 0.3 and 0.9
        use std::time::SystemTime;
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        (seed % 70 + 30) as f64 / 100.0
    }

    // --- Access Operations ---

    /// Get a specific memory entry
    pub async fn get(&self, memory_type: MemoryType, id: &str) -> Option<MemoryEntry> {
        let _ = self.event_tx.send(PipelineEvent::MemoryAccessed {
            memory_type: memory_type.clone(),
            entry_id: id.to_string(),
        });

        match memory_type {
            MemoryType::Working => {
                let working = self.working_memory.read().await;
                working.iter().find(|e| e.id == id).cloned()
            }
            MemoryType::Episodic => {
                let episodic = self.episodic_memory.read().await;
                episodic.iter().find(|e| e.id == id).cloned()
            }
            MemoryType::Semantic => {
                let semantic = self.semantic_memory.read().await;
                semantic.iter().find(|e| e.id == id).cloned()
            }
        }
    }

    /// Get recent working memory entries
    pub async fn get_recent_working(&self, count: usize) -> Vec<MemoryEntry> {
        let working = self.working_memory.read().await;
        working.iter().rev().take(count).cloned().collect()
    }

    /// Get entries by tags
    pub async fn get_by_tags(&self, tags: Vec<String>) -> HashMap<MemoryType, Vec<MemoryEntry>> {
        let mut result = HashMap::new();

        result.insert(MemoryType::Working, Vec::new());
        result.insert(MemoryType::Episodic, Vec::new());
        result.insert(MemoryType::Semantic, Vec::new());

        let working = self.working_memory.read().await;
        for entry in working.iter() {
            if tags.iter().any(|t| entry.tags.contains(t)) {
                result.get_mut(&MemoryType::Working).unwrap().push(entry.clone());
            }
        }

        let episodic = self.episodic_memory.read().await;
        for entry in episodic.iter() {
            if tags.iter().any(|t| entry.tags.contains(t)) {
                result.get_mut(&MemoryType::Episodic).unwrap().push(entry.clone());
            }
        }

        let semantic = self.semantic_memory.read().await;
        for entry in semantic.iter() {
            if tags.iter().any(|t| entry.tags.contains(t)) {
                result.get_mut(&MemoryType::Semantic).unwrap().push(entry.clone());
            }
        }

        result
    }

    // --- Pattern Extraction ---

    /// Extract patterns from an episodic memory entry
    async fn extract_patterns_from_episode(&self, episode: &EpisodicMemoryEntry) {
        // Extract tool sequences
        let tool_sequence: Vec<String> = episode.steps
            .iter()
            .filter_map(|s| {
                if s.action.starts_with("tool:") {
                    Some(s.action.trim_start_matches("tool:").to_string())
                } else {
                    None
                }
            })
            .collect();

        if tool_sequence.len() >= 2 {
            let pattern_key = tool_sequence.join(" -> ");
            let mut cache = self.pattern_cache.write().await;
            cache.entry(pattern_key.clone()).or_insert_with(Vec::new).push(episode.episode_id.clone());

            let _ = self.event_tx.send(PipelineEvent::PatternExtracted {
                pattern_type: PatternType::ToolSequence,
                pattern_id: pattern_key,
            });
        }

        // Extract error patterns
        for step in &episode.steps {
            if !step.success {
                let error_key = format!("error:{}", step.observation.chars().take(50).collect::<String>());
                let mut cache = self.pattern_cache.write().await;
                cache.entry(error_key.clone()).or_insert_with(Vec::new).push(episode.episode_id.clone());

                let _ = self.event_tx.send(PipelineEvent::PatternExtracted {
                    pattern_type: PatternType::ErrorPattern,
                    pattern_id: error_key,
                });
            }
        }
    }

    /// Get frequent tool sequences
    pub async fn get_frequent_tool_sequences(&self, min_occurrences: u64) -> Vec<(String, u64)> {
        let cache = self.pattern_cache.read().await;
        let mut sequences: Vec<_> = cache.iter()
            .filter(|(k, v)| k.contains("->") && v.len() as u64 >= min_occurrences)
            .map(|(k, v)| (k.clone(), v.len() as u64))
            .collect();
        sequences.sort_by(|a, b| b.1.cmp(&a.1));
        sequences
    }

    // --- Consolidation ---

    /// Consolidate memories (move important episodic to semantic)
    pub async fn consolidate(&self) -> usize {
        if !self.config.auto_consolidate {
            return 0;
        }

        let episodic = self.episodic_memory.read().await;
        let _semantic = self.semantic_memory.read().await;

        let to_consolidate: Vec<_> = episodic.iter()
            .filter(|e| e.importance > 0.7 && e.access_count > 3)
            .cloned()
            .collect();

        let count = to_consolidate.len();

        if count > 0 {
            let mut semantic = self.semantic_memory.write().await;
            for entry in to_consolidate {
                let mut new_entry = entry.clone();
                new_entry.id = uuid::Uuid::new_v4().to_string();
                semantic.push(new_entry);

                let _ = self.event_tx.send(PipelineEvent::MemoryConsolidated {
                    from_type: MemoryType::Episodic,
                    to_type: MemoryType::Semantic,
                    count: 1,
                });
            }
        }

        count
    }

    // --- Statistics ---

    /// Get access statistics
    pub async fn get_stats(&self) -> AccessStatistics {
        self.access_stats.read().await.clone()
    }

    /// Get memory counts by type
    pub async fn get_counts(&self) -> HashMap<MemoryType, usize> {
        let mut counts = HashMap::new();

        let working = self.working_memory.read().await;
        counts.insert(MemoryType::Working, working.len());

        let episodic = self.episodic_memory.read().await;
        counts.insert(MemoryType::Episodic, episodic.len());

        let semantic = self.semantic_memory.read().await;
        counts.insert(MemoryType::Semantic, semantic.len());

        counts
    }

    /// Clear all memories
    pub async fn clear(&self) {
        self.working_memory.write().await.clear();
        self.episodic_memory.write().await.clear();
        self.semantic_memory.write().await.clear();
        self.pattern_cache.write().await.clear();
    }
}

// ============================================================================
// Integration with Harness
// ============================================================================

// NOTE: HarnessMemoryIntegration trait disabled due to async_trait lifetime issues
// Will be re-enabled after async_trait is updated or the trait is redesigned

/*
/// Trait for integrating memory pipeline with harness
#[async_trait::async_trait]
pub trait HarnessMemoryIntegration: Send + Sync {
    /// Create memory pipeline from harness context
    async fn create_pipeline_from_harness(&self) -> MemoryPipeline;

    /// Store current harness state to memory
    async fn snapshot_to_memory(&self, pipeline: &MemoryPipeline);
}

impl HarnessMemoryIntegration for super::harness::AgentHarnessContext {
    async fn create_pipeline_from_harness(&self) -> MemoryPipeline {
        MemoryPipeline::with_default()
    }

    async fn snapshot_to_memory(&self, pipeline: &MemoryPipeline) {
        let working_entry = WorkingMemoryEntry {
            session_id: self.config().metadata.get("session_id")
                .cloned().unwrap_or_default(),
            step_count: self.metadata().step_count,
            tool_call_count: self.metadata().tool_call_count,
            tool_failure_count: self.metadata().tool_failure_count,
            llm_tokens_used: self.metadata().llm_tokens_used,
            current_thought: None,
            last_tool: None,
            last_tool_args: None,
            last_tool_result: None,
            resource_usage: ResourceSnapshot {
                memory_bytes: self.resource_tracker().get_usage().memory_bytes,
                cpu_time_ms: self.resource_tracker().get_usage().cpu_time_ms,
                network_requests: 0,
            },
            error_history: self.error_history().iter().map(|e| e.to_string()).collect(),
        };

        pipeline.store_working(working_entry).await;
    }
}
*/

/// Trait for integrating memory pipeline with harness
pub trait HarnessMemoryIntegration: Send + Sync {
    /// Create memory pipeline from harness context
    fn create_pipeline_from_harness(&self) -> MemoryPipeline;

    /// Store current harness state to memory
    fn snapshot_to_memory(&self, pipeline: &MemoryPipeline);
}

impl HarnessMemoryIntegration for super::harness::AgentHarnessContext {
    fn create_pipeline_from_harness(&self) -> MemoryPipeline {
        MemoryPipeline::with_default()
    }

    fn snapshot_to_memory(&self, _pipeline: &MemoryPipeline) {
        // Placeholder implementation - stores current harness state to memory
        // Note: This is a simplified version; full implementation would capture
        // step_count, tool_call_count, error_history, etc.
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pipeline_creation() {
        let pipeline = MemoryPipeline::with_default();
        let counts = pipeline.get_counts().await;

        assert_eq!(*counts.get(&MemoryType::Working).unwrap(), 0);
        assert_eq!(*counts.get(&MemoryType::Episodic).unwrap(), 0);
        assert_eq!(*counts.get(&MemoryType::Semantic).unwrap(), 0);
    }

    #[tokio::test]
    async fn test_store_and_retrieve_working() {
        let pipeline = MemoryPipeline::with_default();

        let entry = WorkingMemoryEntry {
            session_id: "test-session".to_string(),
            step_count: 5,
            tool_call_count: 3,
            tool_failure_count: 0,
            llm_tokens_used: Some(1000),
            current_thought: Some("Test thought".to_string()),
            last_tool: Some("search".to_string()),
            last_tool_args: None,
            last_tool_result: None,
            resource_usage: Default::default(),
            error_history: Vec::new(),
        };

        let id = pipeline.store_working(entry).await;
        assert!(!id.is_empty());

        let retrieved = pipeline.get(MemoryType::Working, &id).await;
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_search_exact() {
        let pipeline = MemoryPipeline::with_default();

        let entry = WorkingMemoryEntry {
            session_id: "test-session".to_string(),
            step_count: 5,
            tool_call_count: 3,
            tool_failure_count: 0,
            llm_tokens_used: Some(1000),
            current_thought: Some("Test thought about Rust programming".to_string()),
            last_tool: Some("search".to_string()),
            last_tool_args: None,
            last_tool_result: None,
            resource_usage: Default::default(),
            error_history: Vec::new(),
        };

        pipeline.store_working(entry).await;

        let query = SearchQuery {
            query: "Rust programming".to_string(),
            mode: SearchMode::Exact,
            limit: 10,
            ..Default::default()
        };

        let results = pipeline.search(query).await;
        assert!(!results.entries.is_empty());
    }

    #[tokio::test]
    async fn test_get_recent_working() {
        let pipeline = MemoryPipeline::with_default();

        for i in 0..5 {
            let entry = WorkingMemoryEntry {
                session_id: format!("session-{}", i),
                step_count: i,
                ..Default::default()
            };
            pipeline.store_working(entry).await;
        }

        let recent = pipeline.get_recent_working(3).await;
        assert_eq!(recent.len(), 3);
    }

    #[tokio::test]
    async fn test_memory_counts() {
        let pipeline = MemoryPipeline::with_default();

        // Add working memory
        pipeline.store_working(WorkingMemoryEntry {
            session_id: "test".to_string(),
            ..Default::default()
        }).await;

        // Add episodic memory
        pipeline.store_episodic(EpisodicMemoryEntry {
            episode_id: "ep1".to_string(),
            session_id: "test".to_string(),
            task_description: "Test task".to_string(),
            start_time: chrono::Utc::now(),
            end_time: None,
            steps: Vec::new(),
            outcome: EpisodeOutcome::Success,
            summary: "Test summary".to_string(),
            lessons_learned: Vec::new(),
        }).await;

        let counts = pipeline.get_counts().await;
        assert_eq!(*counts.get(&MemoryType::Working).unwrap(), 1);
        assert_eq!(*counts.get(&MemoryType::Episodic).unwrap(), 1);
    }
}
