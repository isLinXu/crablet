//! Memory Gardener - Automated memory maintenance and optimization
//!
//! This module implements automated memory care:
//! - Memory pruning (removing stale/irrelevant memories)
//! - Memory consolidation (merging similar memories)
//! - Memory archiving (moving old memories to cold storage)
//! - Memory quality scoring
//! - Duplicate detection and removal
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    Memory Gardener                                  │
//! │                                                                      │
//! │   ┌─────────────┐    ┌─────────────┐    ┌──────────────────────┐   │
//! │   │   Scan      │───→│  Evaluate   │───→│    Take Action       │   │
//! │   │  Memories   │    │   Quality   │    │                      │   │
//! │   └─────────────┘    └─────────────┘    └──────────────────────┘   │
//! │                                                │                     │
//! │                                                ▼                     │
//! │   ┌────────────────────────────────────────────────────────────┐   │
//! │   │                    Actions                                 │   │
//! │   │  • Prune (delete low-quality)                              │   │
//! │   │  • Consolidate (merge similar)                             │   │
//! │   │  • Archive (move to cold storage)                          │   │
//! │   │  • Boost (increase priority for valuable)                  │   │
//! │   └────────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────┘

use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio::time::Instant;
use tracing::{info, warn, debug};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::events::{AgentEvent, EventBus};
use crate::memory::manager::MemoryManager;
use crate::knowledge::vector_store::VectorStore;
use crate::error::Result;

/// Configuration for Memory Gardener
#[derive(Debug, Clone)]
pub struct MemoryGardenerConfig {
    /// How often to run maintenance (default: 1 hour)
    pub maintenance_interval: Duration,
    /// Memory age threshold for archiving (default: 30 days)
    pub archive_threshold: Duration,
    /// Memory age threshold for pruning (default: 90 days)
    pub prune_threshold: Duration,
    /// Minimum quality score to keep (0.0-1.0)
    pub min_quality_score: f32,
    /// Maximum number of memories to process per cycle
    pub max_memories_per_cycle: usize,
    /// Enable automatic pruning
    pub enable_pruning: bool,
    /// Enable automatic archiving
    pub enable_archiving: bool,
    /// Enable duplicate detection
    pub enable_duplicate_detection: bool,
    /// Enable consolidation
    pub enable_consolidation: bool,
    /// Similarity threshold for duplicates (0.0-1.0)
    pub duplicate_similarity_threshold: f32,
    /// Similarity threshold for consolidation (0.0-1.0)
    pub consolidation_similarity_threshold: f32,
}

impl Default for MemoryGardenerConfig {
    fn default() -> Self {
        Self {
            maintenance_interval: Duration::from_secs(3600), // 1 hour
            archive_threshold: Duration::from_secs(30 * 24 * 3600), // 30 days
            prune_threshold: Duration::from_secs(90 * 24 * 3600), // 90 days
            min_quality_score: 0.3,
            max_memories_per_cycle: 100,
            enable_pruning: true,
            enable_archiving: true,
            enable_duplicate_detection: true,
            enable_consolidation: true,
            duplicate_similarity_threshold: 0.95,
            consolidation_similarity_threshold: 0.85,
        }
    }
}

/// Quality metrics for a memory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryQuality {
    pub memory_id: String,
    pub relevance_score: f32,      // 0.0-1.0
    pub access_frequency: u32,     // Number of times accessed
    pub last_accessed: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub importance_score: f32,     // 0.0-1.0
    pub quality_score: f32,        // Combined score
    pub metadata: serde_json::Value,
}

impl MemoryQuality {
    /// Calculate composite quality score
    pub fn calculate_score(&self, now: DateTime<Utc>) -> f32 {
        let age_days = (now - self.created_at).num_days() as f32;
        let days_since_access = (now - self.last_accessed).num_days() as f32;
        
        // Factors:
        // - Importance (weight: 0.4)
        // - Relevance (weight: 0.3)
        // - Recency of access (weight: 0.2)
        // - Age (weight: 0.1, inverse)
        
        let recency_factor = if days_since_access < 1.0 {
            1.0
        } else {
            (1.0 / days_since_access.sqrt()).min(1.0)
        };
        
        let age_factor = if age_days < 7.0 {
            1.0
        } else {
            (7.0 / age_days).min(1.0)
        };
        
        let access_factor = (self.access_frequency as f32 / 10.0).min(1.0);
        
        self.importance_score * 0.4 +
        self.relevance_score * 0.3 +
        recency_factor * 0.2 * access_factor +
        age_factor * 0.1
    }
}

/// Action taken on a memory
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GardenerAction {
    Keep,
    Prune,
    Archive,
    Consolidate { target_id: String },
    BoostPriority,
}

/// Result of a maintenance operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceResult {
    pub timestamp: DateTime<Utc>,
    pub memories_scanned: usize,
    pub memories_pruned: usize,
    pub memories_archived: usize,
    pub memories_consolidated: usize,
    pub duplicates_found: usize,
    pub actions: Vec<(String, GardenerAction)>,
}

/// Statistics for Memory Gardener
#[derive(Debug, Clone, Default)]
pub struct MemoryGardenerStats {
    pub total_maintenance_runs: u64,
    pub total_memories_pruned: u64,
    pub total_memories_archived: u64,
    pub total_memories_consolidated: u64,
    pub total_duplicates_removed: u64,
    pub last_run: Option<DateTime<Utc>>,
    pub last_result: Option<MaintenanceResult>,
    pub average_processing_time_ms: u64,
}

/// Memory Gardener - Automated memory maintenance
pub struct MemoryGardener {
    config: MemoryGardenerConfig,
    event_bus: Arc<EventBus>,
    memory_manager: Arc<MemoryManager>,
    vector_store: Option<Arc<VectorStore>>,
    /// Memory quality cache
    quality_cache: Arc<RwLock<HashMap<String, MemoryQuality>>>,
    /// Statistics
    stats: Arc<RwLock<MemoryGardenerStats>>,
    /// Shutdown signal
    shutdown: Arc<RwLock<bool>>,
    /// Archive storage path
    archive_path: Option<std::path::PathBuf>,
}

impl MemoryGardener {
    pub fn new(
        config: MemoryGardenerConfig,
        event_bus: Arc<EventBus>,
        memory_manager: Arc<MemoryManager>,
        vector_store: Option<Arc<VectorStore>>,
    ) -> Self {
        Self {
            config,
            event_bus,
            memory_manager,
            vector_store,
            quality_cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(MemoryGardenerStats::default())),
            shutdown: Arc::new(RwLock::new(false)),
            archive_path: None,
        }
    }

    /// Set archive path for cold storage
    pub fn with_archive_path(mut self, path: std::path::PathBuf) -> Self {
        self.archive_path = Some(path);
        self
    }

    /// Start the gardener maintenance loop
    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            info!(
                "Memory Gardener started (interval: {:?})",
                self.config.maintenance_interval
            );

            let mut interval = tokio::time::interval(self.config.maintenance_interval);

            loop {
                interval.tick().await;

                if *self.shutdown.read().await {
                    info!("Memory Gardener shutting down");
                    break;
                }

                if let Err(e) = self.run_maintenance().await {
                    warn!("Memory maintenance failed: {}", e);
                }
            }
        });
    }

    /// Stop the gardener
    pub async fn stop(&self) {
        *self.shutdown.write().await = true;
    }

    /// Run a full maintenance cycle
    pub async fn run_maintenance(&self) -> Result<MaintenanceResult> {
        let start_time = Instant::now();
        let now = Utc::now();
        
        info!("Starting memory maintenance cycle");

        let mut result = MaintenanceResult {
            timestamp: now,
            memories_scanned: 0,
            memories_pruned: 0,
            memories_archived: 0,
            memories_consolidated: 0,
            duplicates_found: 0,
            actions: Vec::new(),
        };

        // 1. Scan and evaluate memories
        let memories_to_evaluate = self.scan_memories().await?;
        result.memories_scanned = memories_to_evaluate.len();

        // 2. Detect duplicates
        if self.config.enable_duplicate_detection {
            let duplicates = self.detect_duplicates(&memories_to_evaluate).await?;
            result.duplicates_found = duplicates.len();
            
            for (keep_id, remove_ids) in duplicates {
                for remove_id in remove_ids {
                    if let Err(e) = self.remove_memory(&remove_id).await {
                        warn!("Failed to remove duplicate {}: {}", remove_id, e);
                    } else {
                        result.actions.push((remove_id, GardenerAction::Prune));
                        result.memories_pruned += 1;
                    }
                }
            }
        }

        // 3. Evaluate quality and take actions
        for memory_quality in memories_to_evaluate {
            let action = self.evaluate_action(&memory_quality, now).await?;
            
            match action {
                GardenerAction::Prune => {
                    if self.config.enable_pruning {
                        if let Err(e) = self.remove_memory(&memory_quality.memory_id).await {
                            warn!("Failed to prune memory {}: {}", memory_quality.memory_id, e);
                        } else {
                            result.memories_pruned += 1;
                            result.actions.push((memory_quality.memory_id.clone(), action));
                        }
                    }
                }
                GardenerAction::Archive => {
                    if self.config.enable_archiving {
                        if let Err(e) = self.archive_memory(&memory_quality.memory_id).await {
                            warn!("Failed to archive memory {}: {}", memory_quality.memory_id, e);
                        } else {
                            result.memories_archived += 1;
                            result.actions.push((memory_quality.memory_id.clone(), action));
                        }
                    }
                }
                GardenerAction::Consolidate { target_id } => {
                    if self.config.enable_consolidation {
                        if let Err(e) = self.consolidate_memories(&memory_quality.memory_id, &target_id).await {
                            warn!("Failed to consolidate memories: {}", e);
                        } else {
                            result.memories_consolidated += 1;
                            result.actions.push((memory_quality.memory_id.clone(), action));
                        }
                    }
                }
                GardenerAction::BoostPriority => {
                    if let Err(e) = self.boost_memory_priority(&memory_quality.memory_id).await {
                        warn!("Failed to boost memory priority: {}", e);
                    }
                    result.actions.push((memory_quality.memory_id.clone(), action));
                }
                GardenerAction::Keep => {
                    // No action needed
                }
            }
        }

        // 4. Update statistics
        let duration_ms = start_time.elapsed().as_millis() as u64;
        {
            let mut stats = self.stats.write().await;
            stats.total_maintenance_runs += 1;
            stats.total_memories_pruned += result.memories_pruned as u64;
            stats.total_memories_archived += result.memories_archived as u64;
            stats.total_memories_consolidated += result.memories_consolidated as u64;
            stats.total_duplicates_removed += result.duplicates_found as u64;
            stats.last_run = Some(now);
            stats.last_result = Some(result.clone());
            
            // Update average processing time
            if stats.total_maintenance_runs == 1 {
                stats.average_processing_time_ms = duration_ms;
            } else {
                stats.average_processing_time_ms = 
                    (stats.average_processing_time_ms * (stats.total_maintenance_runs - 1) + duration_ms)
                    / stats.total_maintenance_runs;
            }
        }

        // 5. Publish event
        self.event_bus.publish(AgentEvent::SystemLog(format!(
            "Memory maintenance completed: {} scanned, {} pruned, {} archived, {} consolidated",
            result.memories_scanned,
            result.memories_pruned,
            result.memories_archived,
            result.memories_consolidated
        )));

        info!(
            "Memory maintenance completed in {}ms: {} scanned, {} pruned, {} archived, {} consolidated",
            duration_ms,
            result.memories_scanned,
            result.memories_pruned,
            result.memories_archived,
            result.memories_consolidated
        );

        Ok(result)
    }

    /// Scan memories and collect quality metrics
    async fn scan_memories(&self) -> Result<Vec<MemoryQuality>> {
        let mut qualities = Vec::new();

        // Scan vector store memories if available
        if let Some(vs) = &self.vector_store {
            // This would query the vector store for all memories
            // For now, we'll work with the quality cache
            let cache = self.quality_cache.read().await;
            qualities.extend(cache.values().cloned());
        }

        // Limit to max per cycle
        qualities.truncate(self.config.max_memories_per_cycle);

        Ok(qualities)
    }

    /// Detect duplicate memories
    async fn detect_duplicates(&self, memories: &[MemoryQuality]) -> Result<Vec<(String, Vec<String>)>> {
        let mut duplicates = Vec::new();
        let threshold = self.config.duplicate_similarity_threshold;

        // Simple O(n²) similarity check - could be optimized with LSH or similar
        for (i, mem1) in memories.iter().enumerate() {
            let mut dups_for_mem1 = Vec::new();
            
            for (j, mem2) in memories.iter().enumerate() {
                if i >= j {
                    continue; // Avoid checking same pair twice
                }

                // Calculate similarity (simplified - would use actual embedding similarity)
                let similarity = self.calculate_similarity(mem1, mem2).await?;
                
                if similarity >= threshold {
                    dups_for_mem1.push(mem2.memory_id.clone());
                }
            }

            if !dups_for_mem1.is_empty() {
                duplicates.push((mem1.memory_id.clone(), dups_for_mem1));
            }
        }

        Ok(duplicates)
    }

    /// Calculate similarity between two memories
    async fn calculate_similarity(&self, mem1: &MemoryQuality, mem2: &MemoryQuality) -> Result<f32> {
        // This would use vector embeddings in a real implementation
        // For now, return a placeholder based on metadata similarity
        
        if let (Some(meta1), Some(meta2)) = (
            mem1.metadata.as_object(),
            mem2.metadata.as_object()
        ) {
            let common_keys: Vec<_> = meta1.keys()
                .filter(|k| meta2.contains_key(*k))
                .collect();
            
            if !common_keys.is_empty() {
                return Ok(0.5 + (common_keys.len() as f32 * 0.1).min(0.5));
            }
        }
        
        Ok(0.0)
    }

    /// Evaluate what action to take on a memory
    async fn evaluate_action(&self, quality: &MemoryQuality, now: DateTime<Utc>) -> Result<GardenerAction> {
        let age = now - quality.created_at;
        let days_since_access = (now - quality.last_accessed).num_days();

        // Calculate current quality score
        let score = quality.calculate_score(now);

        // Decision tree
        if score < self.config.min_quality_score {
            // Low quality - check if very old
            if age.num_days() > 30 {
                return Ok(GardenerAction::Prune);
            }
        }

        // Check if should archive (old and rarely accessed)
        if age > chrono::Duration::from_std(self.config.archive_threshold).unwrap_or(chrono::Duration::days(30))
            && days_since_access > 30 {
            return Ok(GardenerAction::Archive);
        }

        // Check if should boost (high quality but not accessed recently)
        if score > 0.8 && days_since_access > 7 {
            return Ok(GardenerAction::BoostPriority);
        }

        // Check for consolidation candidates (would need more context)
        // This is a simplified version
        if score > 0.6 && score < 0.8 {
            // Could be consolidated with similar memories
            // Return Consolidate action with a target (would be determined by similarity search)
        }

        Ok(GardenerAction::Keep)
    }

    /// Remove a memory
    async fn remove_memory(&self, memory_id: &str) -> Result<()> {
        // Remove from vector store if available
        if let Some(vs) = &self.vector_store {
            // vs.delete_document(memory_id).await?;
        }

        // Remove from quality cache
        self.quality_cache.write().await.remove(memory_id);

        debug!("Removed memory: {}", memory_id);
        Ok(())
    }

    /// Archive a memory to cold storage
    async fn archive_memory(&self, memory_id: &str) -> Result<()> {
        if let Some(archive_path) = &self.archive_path {
            // Move memory to archive storage
            // This would involve:
            // 1. Retrieving the memory from vector store
            // 2. Writing to archive storage
            // 3. Removing from active storage
            
            debug!("Archived memory {} to {:?}", memory_id, archive_path);
        }

        Ok(())
    }

    /// Consolidate two memories into one
    async fn consolidate_memories(&self, source_id: &str, target_id: &str) -> Result<()> {
        // This would:
        // 1. Retrieve both memories
        // 2. Merge their content
        // 3. Update the target memory
        // 4. Remove the source memory
        
        debug!("Consolidated memory {} into {}", source_id, target_id);
        Ok(())
    }

    /// Boost memory priority
    async fn boost_memory_priority(&self, memory_id: &str) -> Result<()> {
        // Update memory metadata to increase priority
        if let Some(vs) = &self.vector_store {
            // vs.update_metadata(memory_id, json!({"priority_boost": true})).await?;
        }

        debug!("Boosted priority for memory: {}", memory_id);
        Ok(())
    }

    /// Update memory quality metrics (called when memory is accessed)
    pub async fn record_access(&self, memory_id: &str) {
        let mut cache = self.quality_cache.write().await;
        
        if let Some(quality) = cache.get_mut(memory_id) {
            quality.access_frequency += 1;
            quality.last_accessed = Utc::now();
        }
    }

    /// Register a new memory with quality tracking
    pub async fn register_memory(&self, memory_id: String, importance: f32, metadata: serde_json::Value) {
        let now = Utc::now();
        let quality = MemoryQuality {
            memory_id: memory_id.clone(),
            relevance_score: 0.5, // Initial score
            access_frequency: 0,
            last_accessed: now,
            created_at: now,
            importance_score: importance,
            quality_score: 0.5,
            metadata,
        };

        self.quality_cache.write().await.insert(memory_id, quality);
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> MemoryGardenerStats {
        self.stats.read().await.clone()
    }

    /// Get quality cache size
    pub async fn get_cache_size(&self) -> usize {
        self.quality_cache.read().await.len()
    }

    /// Force run maintenance (for testing or manual triggers)
    pub async fn force_maintenance(&self) -> Result<MaintenanceResult> {
        self.run_maintenance().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_quality_calculation() {
        let quality = MemoryQuality {
            memory_id: "test".to_string(),
            relevance_score: 0.8,
            access_frequency: 5,
            last_accessed: Utc::now(),
            created_at: Utc::now(),
            importance_score: 0.9,
            quality_score: 0.0,
            metadata: serde_json::Value::Null,
        };

        let score = quality.calculate_score(Utc::now());
        assert!(score > 0.0 && score <= 1.0);
    }

    #[test]
    fn test_memory_gardener_config_default() {
        let config = MemoryGardenerConfig::default();
        assert_eq!(config.maintenance_interval, Duration::from_secs(3600));
        assert!(config.enable_pruning);
        assert!(config.enable_archiving);
        assert!(config.enable_duplicate_detection);
    }

    #[test]
    fn test_gardener_action_serialization() {
        let action = GardenerAction::Consolidate { target_id: "target".to_string() };
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("Consolidate"));
    }
}
