//! Active Forgetting Mechanism
//!
//! Implements intelligent memory cleanup based on:
//! - Memory value scoring (recency, frequency, relevance)
//! - Ebbinghaus forgetting curve simulation
//! - Storage capacity constraints
//! - User interaction patterns
//!
//! # Architecture
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      Active Forgetting Engine                    │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │   Memory Store ──▶ Value Scorer ──▶ Forget List ──▶ Cleanup     │
//! │        │              │                 │            │           │
//! │        ▼              ▼                 ▼            ▼           │
//! │   Memory Pool    Scoring Criteria    Priority     Delete      │
//! │   (Working/       - Recency (0.3)     Queue       Operations  │
//! │    Episodic/      - Frequency (0.3)                            │
//! │    Semantic)     - Relevance (0.2)                            │
//! │                   - User Action (0.2)                          │
//! │                                                                 │
//! │   ───────────────────────────────────────────────────────────  │
//! │   Configurable Forgetting Strategies:                          │
//! │   1. Value Threshold Cleanup                                  │
//! │   2. Age-based Cleanup (Ebbinghaus)                           │
//! │   3. LRU (Least Recently Used)                                 │
//! │   4. Random Sampling (for diversity)                           │
//! │                                                                 │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use anyhow::Result;
use chrono::{DateTime, Utc};

use crate::memory::{MemoryEntry, MemoryType};

// ============================================================================
// Memory Value Scoring
// ============================================================================

/// Memory value score (0.0 - 1.0, higher is more valuable)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryScore {
    pub memory_id: String,
    pub memory_type: MemoryType,
    pub value: f32,
    pub recency_score: f32,
    pub frequency_score: f32,
    pub relevance_score: f32,
    pub user_action_score: f32,
    pub last_accessed: DateTime<Utc>,
    pub access_count: u32,
}

impl MemoryScore {
    /// Compute composite value score
    pub fn compute_value(&self, weights: &ScoringWeights) -> f32 {
        self.recency_score * weights.recency
            + self.frequency_score * weights.frequency
            + self.relevance_score * weights.relevance
            + self.user_action_score * weights.user_action
    }
}

/// Weights for scoring criteria
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub recency: f32,
    pub frequency: f32,
    pub relevance: f32,
    pub user_action: f32,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            recency: 0.3,
            frequency: 0.3,
            relevance: 0.2,
            user_action: 0.2,
        }
    }
}

/// Memory value scorer
pub struct MemoryScorer {
    weights: ScoringWeights,
    recency_decay: Duration,  // Time for recency to decay to 0.5
    access_window: Duration,   // Window for frequency counting
}

impl MemoryScorer {
    pub fn new(weights: ScoringWeights) -> Self {
        Self {
            weights,
            recency_decay: Duration::from_secs(3600 * 24),  // 1 day
            access_window: Duration::from_secs(3600 * 24 * 7),  // 1 week
        }
    }

    /// Score a memory entry
    pub fn score(&self, entry: &MemoryEntry, access_count: u32, last_accessed: DateTime<Utc>) -> MemoryScore {
        let now = Utc::now();
        let age = now.signed_duration_since(last_accessed);

        // Recency: exponential decay
        let age_seconds = age.num_seconds() as f64;
        let recency_score = (-age_seconds / self.recency_decay.as_secs_f64()).exp() as f32;

        // Frequency: logarithmic scaling
        let frequency_score = (access_count as f32 / (1.0 + access_count as f32).ln()).min(1.0);

        // Relevance: based on memory type and metadata (simplified)
        let relevance_score = self.score_relevance(entry);

        // User action: based on interactions (simplified)
        let user_action_score = self.score_user_action(entry);

        let mut score = MemoryScore {
            memory_id: entry.id.clone(),
            memory_type: entry.memory_type.clone(),
            value: 0.0,
            recency_score,
            frequency_score,
            relevance_score,
            user_action_score,
            last_accessed,
            access_count,
        };

        score.value = score.compute_value(&self.weights);
        score
    }

    fn score_relevance(&self, entry: &MemoryEntry) -> f32 {
        match entry.memory_type {
            MemoryType::Working => 0.4,  // High relevance, short-lived
            MemoryType::Episodic => 0.6,  // Medium relevance
            MemoryType::Semantic => 0.8,  // High relevance, long-lived
            MemoryType::Procedural => 0.5,  // Medium relevance for procedural
        }
    }

    fn score_user_action(&self, entry: &MemoryEntry) -> f32 {
        // Simplified: check if entry has metadata indicating user interaction
        if entry.metadata.contains_key("user_starred") {
            1.0
        } else if entry.metadata.contains_key("user_retrieved") {
            0.7
        } else {
            0.5
        }
    }
}

impl Default for MemoryScorer {
    fn default() -> Self {
        Self::new(ScoringWeights::default())
    }
}

// ============================================================================
// Forgetting Strategies
// ============================================================================

/// Forgetting strategy
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ForgettingStrategy {
    /// Forget memories below value threshold
    ValueThreshold(f32),
    /// Forget based on Ebbinghaus curve
    Ebbinghaus {
        initial_retention: f32,
        decay_constant: f32,
    },
    /// Least Recently Used
    LRU { max_age: Duration },
    /// Random sampling for diversity
    Random { keep_ratio: f32 },
}

impl Default for ForgettingStrategy {
    fn default() -> Self {
        ForgettingStrategy::ValueThreshold(0.3)
    }
}

/// Ebbinghaus forgetting curve: R = e^(-t/S)
pub fn ebbinghaus_retention(elapsed: Duration, decay_constant: Duration) -> f32 {
    let t = elapsed.as_secs_f64();
    let s = decay_constant.as_secs_f64();
    (-t / s).exp() as f32
}

// ============================================================================
// Forget List
// ============================================================================

/// Memory to forget with priority
#[derive(Debug, Clone)]
pub struct ForgetEntry {
    pub memory_id: String,
    pub memory_type: MemoryType,
    pub value: f32,
    pub forget_at: DateTime<Utc>,
    pub reason: ForgetReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ForgetReason {
    LowValue,
    Expired,
    CapacityLimit,
    UserRequest,
}

/// Priority queue of memories to forget
pub struct ForgetList {
    entries: Vec<ForgetEntry>,
    capacity: usize,
}

impl ForgetList {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Add a memory to forget list
    pub fn add(&mut self, entry: ForgetEntry) {
        self.entries.push(entry);
        // Maintain min-heap based on forget_at (or could sort by value)
        self.entries.sort_by_key(|e| e.forget_at);
    }

    /// Get memories ready to forget
    pub fn ready_to_forget(&self) -> Vec<&ForgetEntry> {
        let now = Utc::now();
        self.entries
            .iter()
            .filter(|e| e.forget_at <= now)
            .collect()
    }

    /// Remove an entry from the list
    pub fn remove(&mut self, memory_id: &str) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| e.memory_id == memory_id) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get count
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

// ============================================================================
// Active Forgetting Engine
// ============================================================================

/// Active forgetting engine configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgettingConfig {
    /// Value threshold for default strategy
    pub value_threshold: f32,
    /// Maximum memories of each type to keep
    pub max_working_memories: usize,
    pub max_episodic_memories: usize,
    pub max_semantic_memories: usize,
    /// Check interval
    pub check_interval: Duration,
    /// Forgetting strategy
    pub strategy: ForgettingStrategy,
}

impl Default for ForgettingConfig {
    fn default() -> Self {
        Self {
            value_threshold: 0.3,
            max_working_memories: 100,
            max_episodic_memories: 1000,
            max_semantic_memories: 10000,
            check_interval: Duration::from_secs(300),  // 5 minutes
            strategy: ForgettingStrategy::default(),
        }
    }
}

/// Active forgetting engine
pub struct ActiveForgettingEngine {
    config: ForgettingConfig,
    scorer: MemoryScorer,
    forget_list: ForgetList,
    memory_pools: Arc<RwLock<HashMap<MemoryType, Vec<MemoryEntry>>>>,
    access_counts: Arc<RwLock<HashMap<String, u32>>>,
    last_accessed: Arc<RwLock<HashMap<String, DateTime<Utc>>>>,
}

impl ActiveForgettingEngine {
    pub fn new(config: ForgettingConfig) -> Self {
        Self {
            scorer: MemoryScorer::default(),
            forget_list: ForgetList::new(1000),
            memory_pools: Arc::new(RwLock::new(HashMap::new())),
            access_counts: Arc::new(RwLock::new(HashMap::new())),
            last_accessed: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Register a memory access
    pub async fn access_memory(&self, memory_id: &str) {
        let mut counts = self.access_counts.write().await;
        *counts.entry(memory_id.to_string()).or_insert(0) += 1;

        let mut last = self.last_accessed.write().await;
        last.insert(memory_id.to_string(), Utc::now());
    }

    /// Evaluate memories for forgetting
    pub async fn evaluate(&mut self, memories: Vec<MemoryEntry>) -> Result<usize> {
        let mut added = 0;
        let now = Utc::now();

        // Group by memory type
        let mut by_type: HashMap<MemoryType, Vec<MemoryEntry>> = HashMap::new();
        for mem in memories {
            by_type.entry(mem.memory_type.clone())
                .or_insert_with(Vec::new)
                .push(mem);
        }

        // For each type, apply strategy
        for (mem_type, entries) in by_type {
            let max_allowed = match mem_type {
                MemoryType::Working => self.config.max_working_memories,
                MemoryType::Episodic => self.config.max_episodic_memories,
                MemoryType::Semantic => self.config.max_semantic_memories,
                MemoryType::Procedural => usize::MAX, // Procedural memory has no limit
            };

            if entries.len() > max_allowed {
                // Need to forget some
                // First, acquire read locks for access counts and last accessed
                let access_counts = self.access_counts.read().await;
                let last_accessed = self.last_accessed.read().await;

                let scores: Vec<_> = entries
                    .iter()
                    .map(|entry| {
                        let count = access_counts
                            .get(&entry.id)
                            .copied()
                            .unwrap_or(0);
                        let last = last_accessed
                            .get(&entry.id)
                            .copied()
                            .unwrap_or(now);

                        self.scorer.score(entry, count, last)
                    })
                    .collect();

                // Sort by value (lowest first)
                let mut sorted: Vec<_> = scores.into_iter().enumerate().collect();
                sorted.sort_by(|a, b| a.1.value.partial_cmp(&b.1.value).unwrap());

                // Mark excess low-value memories for forgetting
                let to_forget = entries.len() - max_allowed;
                for (idx, score) in sorted.iter().take(to_forget) {
                    let entry = &entries[*idx];

                    if self.should_forget(score) {
                        let forget_entry = ForgetEntry {
                            memory_id: entry.id.clone(),
                            memory_type: entry.memory_type.clone(),
                            value: score.value,
                            forget_at: self.calculate_forget_time(entry, score),
                            reason: ForgetReason::LowValue,
                        };

                        self.forget_list.add(forget_entry);
                        added += 1;
                    }
                }
            }
        }

        Ok(added)
    }

    fn should_forget(&self, score: &MemoryScore) -> bool {
        match self.config.strategy {
            ForgettingStrategy::ValueThreshold(threshold) => score.value < threshold,
            ForgettingStrategy::Ebbinghaus { .. } => {
                let now = Utc::now();
                let elapsed = now.signed_duration_since(score.last_accessed);
                let elapsed_secs = elapsed.num_seconds() as f64;
                let retention = ebbinghaus_retention(Duration::from_secs_f64(elapsed_secs), Duration::from_secs(3600 * 24));
                retention < 0.3
            }
            ForgettingStrategy::LRU { max_age } => {
                let now = Utc::now();
                let elapsed = now.signed_duration_since(score.last_accessed);
                elapsed.to_std().map(|d| d > max_age).unwrap_or(false)
            }
            ForgettingStrategy::Random { keep_ratio } => {
                fastrand::f32() > keep_ratio
            }
        }
    }

    fn calculate_forget_time(&self, _entry: &MemoryEntry, _score: &MemoryScore) -> DateTime<Utc> {
        // Default: forget immediately (can add delay based on policy)
        Utc::now()
    }

    /// Execute forgetting
    pub async fn forget(&mut self) -> Result<Vec<String>> {
        // Collect ready entries first to avoid borrow conflicts
        let ready: Vec<_> = self.forget_list.ready_to_forget();
        let memory_ids: Vec<(String, MemoryType)> = ready
            .into_iter()
            .map(|e| (e.memory_id.clone(), e.memory_type.clone()))
            .collect();

        let mut forgotten = Vec::new();

        for (memory_id, memory_type) in memory_ids {
            // Remove from pools
            let mut pools = self.memory_pools.write().await;
            if let Some(pool) = pools.get_mut(&memory_type) {
                pool.retain(|m| m.id != memory_id);
            }

            // Remove from tracking
            self.access_counts.write().await.remove(&memory_id);
            self.last_accessed.write().await.remove(&memory_id);

            // Remove from forget list
            self.forget_list.remove(&memory_id);

            forgotten.push(memory_id);
        }

        Ok(forgotten)
    }

    /// Get statistics
    pub async fn stats(&self) -> ForgettingStats {
        let pools = self.memory_pools.read().await;

        let working_count = pools.get(&MemoryType::Working)
            .map(|v| v.len())
            .unwrap_or(0);
        let episodic_count = pools.get(&MemoryType::Episodic)
            .map(|v| v.len())
            .unwrap_or(0);
        let semantic_count = pools.get(&MemoryType::Semantic)
            .map(|v| v.len())
            .unwrap_or(0);

        ForgettingStats {
            total_memories: working_count + episodic_count + semantic_count,
            working_count,
            episodic_count,
            semantic_count,
            pending_forgets: self.forget_list.len(),
        }
    }
}

impl Default for ActiveForgettingEngine {
    fn default() -> Self {
        Self::new(ForgettingConfig::default())
    }
}

/// Forgetting statistics
#[derive(Debug, Clone)]
pub struct ForgettingStats {
    pub total_memories: usize,
    pub working_count: usize,
    pub episodic_count: usize,
    pub semantic_count: usize,
    pub pending_forgets: usize,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ebbinghaus_curve() {
        // After 1 second, should be near 1.0
        let r1 = ebbinghaus_retention(Duration::from_secs(1), Duration::from_secs(3600 * 24));
        assert!(r1 > 0.95);

        // After 1 day (S), should be ~0.37 (e^-1)
        let r2 = ebbinghaus_retention(Duration::from_secs(3600 * 24), Duration::from_secs(3600 * 24));
        assert!((r2 - 0.37).abs() < 0.1);
    }

    #[test]
    fn test_memory_scorer() {
        let scorer = MemoryScorer::default();
        let entry = MemoryEntry {
            id: "test".to_string(),
            memory_type: MemoryType::Working,
            content: "".to_string(),
            created_at: chrono::Utc::now(),
            metadata: HashMap::new(),
            ..Default::default()
        };

        let score = scorer.score(&entry, 5, chrono::Utc::now());
        assert!(score.value >= 0.0 && score.value <= 1.0);
    }

    #[tokio::test]
    async fn test_active_forgetting() {
        let mut engine = ActiveForgettingEngine::new(ForgettingConfig {
            max_working_memories: 1,
            ..Default::default()
        });

        let memories = vec![
            MemoryEntry {
                id: "mem1".to_string(),
                memory_type: MemoryType::Working,
                content: "".to_string(),
                created_at: chrono::Utc::now(),
                metadata: HashMap::new(),
                ..Default::default()
            },
            MemoryEntry {
                id: "mem2".to_string(),
                memory_type: MemoryType::Working,
                content: "".to_string(),
                created_at: chrono::Utc::now(),
                metadata: HashMap::new(),
                ..Default::default()
            },
        ];

        let added = engine.evaluate(memories).await.unwrap();
        assert!(added > 0);
    }

    #[test]
    fn test_forget_list() {
        let mut list = ForgetList::new(10);
        let entry = ForgetEntry {
            memory_id: "test".to_string(),
            memory_type: MemoryType::Working,
            value: 0.1,
            forget_at: chrono::Utc::now(),
            reason: ForgetReason::LowValue,
        };

        list.add(entry);
        assert_eq!(list.len(), 1);

        assert!(list.ready_to_forget().len() >= 1);
    }
}