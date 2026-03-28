//! Biological Memory Decay System
//!
//! Implements memory management inspired by biological memory decay mechanisms:
//! - Ebbinghaus forgetting curve simulation
//! - Memory importance re-evaluation
//! - Spaced repetition for memory reinforcement
//! - Emotional tagging affects decay rate
//! - Adaptive decay based on access patterns
//!
//! # Forgetting Curve Model
//!
//! ```text
//! Retention (%)
//!     ^
//! 100 |**
//!     |   **
//!  80 |      **
//!     |         **
//!  60 |            **---- memory saved (review)
//!     |                 **
//!  40 |                    **
//!     |                       **-----------
//!  20 |                                          **
//!     |                                             ** (almost forgotten)
//!   0 +----------------------------------------------> Time
//!      0    1    7    14   30   60   90  (days)
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};

/// Memory decay model
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DecayModel {
    /// Ebbinghaus forgetting curve: R = e^(-t/S)
    Ebbinghaus,
    /// Power law decay: R = t^(-α)
    PowerLaw,
    /// Adaptive decay based on importance
    Adaptive,
}

/// Memory entry with decay tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayingMemory {
    /// Memory ID
    pub id: String,
    
    /// Memory content
    pub content: String,
    
    /// Current retention strength (0.0 - 1.0)
    pub retention: f32,
    
    /// Base importance score (0.0 - 1.0)
    pub base_importance: f32,
    
    /// Current importance (adjusted by usage)
    pub current_importance: f32,
    
    /// Decay rate modifier based on emotion
    pub emotional_modifier: f32,
    
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    
    /// Last access timestamp
    pub last_accessed: DateTime<Utc>,
    
    /// Last review timestamp
    pub last_reviewed: DateTime<Utc>,
    
    /// Number of times accessed
    pub access_count: u32,
    
    /// Number of times reviewed/reinforced
    pub review_count: u32,
    
    /// Optimal review interval (in hours)
    pub optimal_interval_hours: f32,
    
    /// Next scheduled review
    pub next_review: DateTime<Utc>,
    
    /// Memory category for grouping
    pub category: String,
    
    /// Tags for filtering
    pub tags: Vec<String>,
    
    /// Whether memory is protected from decay
    pub is_pinned: bool,
    
    /// User feedback on this memory (positive reinforcement)
    pub user_feedback_score: f32,
}

impl DecayingMemory {
    pub fn new(id: String, content: String, importance: f32, category: String) -> Self {
        let now = Utc::now();
        Self {
            id,
            content,
            retention: 1.0,
            base_importance: importance,
            current_importance: importance,
            emotional_modifier: 1.0,
            created_at: now,
            last_accessed: now,
            last_reviewed: now,
            access_count: 0,
            review_count: 0,
            optimal_interval_hours: 24.0,
            next_review: now + Duration::hours(24),
            category,
            tags: Vec::new(),
            is_pinned: false,
            user_feedback_score: 0.5,
        }
    }
    
    /// Calculate retention using Ebbinghaus model
    pub fn ebbinghaus_retention(&self, now: DateTime<Utc>) -> f32 {
        let hours_elapsed = (now - self.last_reviewed).num_minutes() as f32 / 60.0;
        let stability = self.current_importance * 10.0;
        
        (-hours_elapsed / stability).exp().max(0.0).min(1.0)
    }
    
    /// Calculate retention using Power law model
    pub fn power_law_retention(&self, now: DateTime<Utc>) -> f32 {
        let days_elapsed = (now - self.last_reviewed).num_minutes() as f32 / (60.0 * 24.0);
        let alpha = 0.1 + (1.0 - self.current_importance) * 0.2;
        
        (1.0 + days_elapsed).powf(-alpha).max(0.0).min(1.0)
    }
    
    /// Get current retention based on configured model
    pub fn get_retention(&self, now: DateTime<Utc>, model: DecayModel) -> f32 {
        if self.is_pinned {
            return 1.0;
        }
        
        let base = match model {
            DecayModel::Ebbinghaus => self.ebbinghaus_retention(now),
            DecayModel::PowerLaw => self.power_law_retention(now),
            DecayModel::Adaptive => {
                let e = self.ebbinghaus_retention(now);
                let p = self.power_law_retention(now);
                e * 0.6 + p * 0.4
            }
        };
        
        base * self.emotional_modifier
    }
    
    /// Calculate next optimal review interval using spaced repetition
    pub fn calculate_next_interval(&self) -> f32 {
        let base_interval = 24.0;
        
        if self.review_count == 0 {
            return base_interval;
        }
        
        let interval = base_interval * (2.0_f32).powf(self.review_count as f32);
        let adjusted = interval * (0.5 + self.current_importance);
        
        adjusted.min(720.0)
    }
    
    /// Reinforce memory (called when memory is accessed or reviewed)
    pub fn reinforce(&mut self, success: bool, feedback: Option<f32>) {
        self.access_count += 1;
        self.last_accessed = Utc::now();
        
        if success {
            self.review_count += 1;
            self.last_reviewed = Utc::now();
            self.retention = 1.0;
            self.current_importance = (self.current_importance + 0.1).min(1.0);
            self.optimal_interval_hours = self.calculate_next_interval();
            self.next_review = Utc::now() + Duration::hours(self.optimal_interval_hours as i64);
        } else {
            self.current_importance = (self.current_importance - 0.1).max(0.1);
            self.optimal_interval_hours = (self.optimal_interval_hours / 2.0).max(1.0);
            self.next_review = Utc::now() + Duration::hours(self.optimal_interval_hours as i64);
        }
        
        if let Some(score) = feedback {
            self.user_feedback_score = (self.user_feedback_score + score) / 2.0;
            let score_f = score as f32;
            let half = 0.5_f32;
            self.emotional_modifier = if score_f > half {
                1.0 + (score_f - half) * 0.5
            } else if score_f < half {
                1.0 - (half - score_f) * 0.5
            } else {
                1.0
            };
        }
    }
    
    /// Check if memory should be forgotten
    pub fn should_forget(&self, now: DateTime<Utc>, threshold: f32, model: DecayModel) -> bool {
        if self.is_pinned {
            return false;
        }
        
        self.get_retention(now, model) < threshold
    }
}

/// Memory decay statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayStats {
    pub total_memories: usize,
    pub healthy_memories: usize,
    pub decaying_memories: usize,
    pub critical_memories: usize,
    pub pinned_memories: usize,
    pub average_retention: f32,
    pub memories_needing_review: usize,
    pub forgotten_memories_count: usize,
}

/// Configuration for memory decay
#[derive(Debug, Clone)]
pub struct DecayConfig {
    pub decay_model: DecayModel,
    pub forget_threshold: f32,
    pub critical_threshold: f32,
    pub review_threshold: f32,
    pub max_memories: usize,
    pub enable_auto_forget: bool,
    pub enable_spaced_repetition: bool,
    pub base_decay_modifier: f32,
    pub check_interval_seconds: u64,
}

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            decay_model: DecayModel::Adaptive,
            forget_threshold: 0.1,
            critical_threshold: 0.3,
            review_threshold: 0.7,
            max_memories: 10000,
            enable_auto_forget: true,
            enable_spaced_repetition: true,
            base_decay_modifier: 1.0,
            check_interval_seconds: 3600,
        }
    }
}

/// Memory Decay Manager
pub struct MemoryDecayManager {
    config: DecayConfig,
    memories: Arc<RwLock<HashMap<String, DecayingMemory>>>,
    decay_log: Arc<RwLock<Vec<DecayEvent>>>,
    stats: Arc<RwLock<DecayStats>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayEvent {
    pub timestamp: DateTime<Utc>,
    pub memory_id: String,
    pub event_type: DecayEventType,
    pub retention_before: f32,
    pub retention_after: Option<f32>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecayEventType {
    Created,
    Accessed,
    Reinforced,
    Reviewed,
    Forgot,
    Pinned,
    Unpinned,
    ReEvaluated,
}

impl MemoryDecayManager {
    pub fn new(config: DecayConfig) -> Self {
        Self {
            config,
            memories: Arc::new(RwLock::new(HashMap::new())),
            decay_log: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(DecayStats {
                total_memories: 0,
                healthy_memories: 0,
                decaying_memories: 0,
                critical_memories: 0,
                pinned_memories: 0,
                average_retention: 1.0,
                memories_needing_review: 0,
                forgotten_memories_count: 0,
            })),
        }
    }
    
    pub fn with_default() -> Self {
        Self::new(DecayConfig::default())
    }
    
    /// Add a new memory
    pub async fn add_memory(
        &self,
        id: String,
        content: String,
        importance: f32,
        category: String,
        tags: Vec<String>,
    ) -> Result<(), MemoryDecayError> {
        let mut memories = self.memories.write().await;
        
        if memories.len() >= self.config.max_memories {
            let to_remove: Vec<String> = memories
                .iter()
                .filter(|(_, m)| !m.is_pinned && m.retention < 0.2)
                .map(|(id, _)| id.clone())
                .take(memories.len() - self.config.max_memories + 100)
                .collect();
            
            for id in to_remove {
                memories.remove(&id);
            }
        }
        
        let mut memory = DecayingMemory::new(id.clone(), content, importance, category);
        memory.tags = tags;
        
        memories.insert(id.clone(), memory);
        
        self.log_event(DecayEvent {
            timestamp: Utc::now(),
            memory_id: id,
            event_type: DecayEventType::Created,
            retention_before: 1.0,
            retention_after: None,
            reason: "Memory created".to_string(),
        }).await;
        
        self.update_stats().await;
        Ok(())
    }
    
    /// Access memory (boosts retention)
    pub async fn access_memory(&self, id: &str, success: bool) -> Result<(), MemoryDecayError> {
        let mut memories = self.memories.write().await;
        
        if let Some(memory) = memories.get_mut(id) {
            let before = memory.retention;
            memory.reinforce(success, None);
            
            self.log_event(DecayEvent {
                timestamp: Utc::now(),
                memory_id: id.to_string(),
                event_type: DecayEventType::Accessed,
                retention_before: before,
                retention_after: Some(memory.retention),
                reason: format!("Access count: {}", memory.access_count),
            }).await;
        }
        
        self.update_stats().await;
        Ok(())
    }
    
    /// Provide feedback on memory
    pub async fn feedback_memory(&self, id: &str, score: f32) -> Result<(), MemoryDecayError> {
        let mut memories = self.memories.write().await;
        
        if let Some(memory) = memories.get_mut(id) {
            memory.reinforce(true, Some(score));
            
            self.log_event(DecayEvent {
                timestamp: Utc::now(),
                memory_id: id.to_string(),
                event_type: DecayEventType::Reinforced,
                retention_before: 1.0,
                retention_after: Some(memory.retention),
                reason: format!("Feedback score: {:.2}", score),
            }).await;
        }
        
        self.update_stats().await;
        Ok(())
    }
    
    /// Pin memory (prevent decay)
    pub async fn pin_memory(&self, id: &str) -> Result<(), MemoryDecayError> {
        let mut memories = self.memories.write().await;
        
        if let Some(memory) = memories.get_mut(id) {
            memory.is_pinned = true;
            memory.retention = 1.0;
            
            self.log_event(DecayEvent {
                timestamp: Utc::now(),
                memory_id: id.to_string(),
                event_type: DecayEventType::Pinned,
                retention_before: 1.0,
                retention_after: Some(1.0),
                reason: "Memory pinned by user".to_string(),
            }).await;
        }
        
        self.update_stats().await;
        Ok(())
    }
    
    /// Unpin memory
    pub async fn unpin_memory(&self, id: &str) -> Result<(), MemoryDecayError> {
        let mut memories = self.memories.write().await;
        
        if let Some(memory) = memories.get_mut(id) {
            memory.is_pinned = false;
            
            self.log_event(DecayEvent {
                timestamp: Utc::now(),
                memory_id: id.to_string(),
                event_type: DecayEventType::Unpinned,
                retention_before: 1.0,
                retention_after: Some(memory.retention),
                reason: "Memory unpinned".to_string(),
            }).await;
        }
        
        self.update_stats().await;
        Ok(())
    }
    
    /// Run decay cycle (should be called periodically)
    pub async fn run_decay_cycle(&self) -> DecayCycleResult {
        let now = Utc::now();
        let mut result = DecayCycleResult::default();
        
        let mut memories = self.memories.write().await;
        
        // First pass: update retention for all memories
        let mut to_remove: Vec<String> = Vec::new();
        
        for (id, memory) in memories.iter_mut() {
            if memory.is_pinned {
                continue;
            }
            
            let old_retention = memory.retention;
            memory.retention = memory.get_retention(now, self.config.decay_model);
            
            if (old_retention - memory.retention).abs() > 0.01 {
                result.memories_updated += 1;
            }
            
            if self.config.enable_auto_forget && memory.should_forget(now, self.config.forget_threshold, self.config.decay_model) {
                to_remove.push(id.clone());
            }
        }
        
        // Second pass: remove forgotten memories
        for id in &to_remove {
            if let Some(memory) = memories.get(id) {
                self.log_event(DecayEvent {
                    timestamp: now,
                    memory_id: id.clone(),
                    event_type: DecayEventType::Forgot,
                    retention_before: memory.retention,
                    retention_after: Some(0.0),
                    reason: format!("Retention dropped below {:.2}", self.config.forget_threshold),
                }).await;
            }
            memories.remove(id);
            result.memories_forgotten += 1;
        }
        
        result.memories_needing_review = memories
            .values()
            .filter(|m| !m.is_pinned && m.next_review <= now)
            .count();
        
        self.update_stats().await;
        result
    }
    
    /// Get memories due for review
    pub async fn get_memories_for_review(&self, limit: usize) -> Vec<DecayingMemory> {
        let memories = self.memories.read().await;
        let now = Utc::now();
        
        let mut due: Vec<_> = memories
            .values()
            .filter(|m| !m.is_pinned && m.next_review <= now)
            .cloned()
            .collect();
        
        due.sort_by(|a, b| {
            let a_score = a.retention * 0.7 + a.current_importance * 0.3;
            let b_score = b.retention * 0.7 + b.current_importance * 0.3;
            b_score.partial_cmp(&a_score).unwrap()
        });
        
        due.truncate(limit);
        due
    }
    
    /// Get current decay statistics
    pub async fn get_stats(&self) -> DecayStats {
        self.stats.read().await.clone()
    }
    
    /// Get memory by ID
    pub async fn get_memory(&self, id: &str) -> Option<DecayingMemory> {
        let memories = self.memories.read().await;
        let now = Utc::now();
        
        memories.get(id).map(|m| {
            let mut memory = m.clone();
            if !memory.is_pinned {
                memory.retention = memory.get_retention(now, self.config.decay_model);
            }
            memory
        })
    }
    
    /// Log a decay event
    async fn log_event(&self, event: DecayEvent) {
        let mut log = self.decay_log.write().await;
        log.push(event);
        
        if log.len() > 10000 {
            log.drain(0..1000);
        }
    }
    
    /// Update statistics
    async fn update_stats(&self) {
        let memories = self.memories.read().await;
        let now = Utc::now();
        
        let mut stats = self.stats.write().await;
        
        stats.total_memories = memories.len();
        stats.pinned_memories = memories.values().filter(|m| m.is_pinned).count();
        stats.memories_needing_review = memories
            .values()
            .filter(|m| !m.is_pinned && m.next_review <= now)
            .count();
        
        let (healthy, decaying, critical, avg, forgotten) = {
            let mut h = 0;
            let mut d = 0;
            let mut c = 0;
            let mut sum = 0.0_f32;
            let mut f = 0;
            
            for memory in memories.values() {
                let retention = memory.get_retention(now, self.config.decay_model);
                sum += retention;
                
                if retention > self.config.review_threshold {
                    h += 1;
                } else if retention >= self.config.critical_threshold {
                    d += 1;
                } else if !memory.is_pinned {
                    c += 1;
                }
                
                if retention < self.config.forget_threshold && !memory.is_pinned {
                    f += 1;
                }
            }
            
            (h, d, c, if memories.is_empty() { 0.0 } else { sum / memories.len() as f32 }, f)
        };
        
        stats.healthy_memories = healthy;
        stats.decaying_memories = decaying;
        stats.critical_memories = critical;
        stats.average_retention = avg;
        stats.forgotten_memories_count = forgotten;
    }
    
    /// Set emotional modifier for memories matching criteria
    pub async fn set_emotional_modifier(&self, category: Option<&str>, modifier: f32) -> usize {
        let mut memories = self.memories.write().await;
        let mut count = 0;
        
        for memory in memories.values_mut() {
            let matches = category.map_or(true, |cat| memory.category == cat);
            if matches {
                memory.emotional_modifier = modifier;
                count += 1;
            }
        }
        
        count
    }
}

/// Result of a decay cycle
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecayCycleResult {
    pub memories_updated: usize,
    pub memories_forgotten: usize,
    pub memories_needing_review: usize,
    pub cycle_duration_ms: u64,
}

/// Errors for memory decay operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryDecayError {
    MemoryNotFound(String),
    InvalidImportance(f32),
    PersistenceError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ebbinghaus_retention() {
        let memory = DecayingMemory::new(
            "test".to_string(),
            "Test content".to_string(),
            0.7,
            "test".to_string(),
        );
        
        let now = Utc::now();
        let retention = memory.ebbinghaus_retention(now);
        assert_eq!(retention, 1.0);
    }

    #[tokio::test]
    async fn test_add_memory() {
        let manager = MemoryDecayManager::with_default();
        
        manager.add_memory(
            "mem1".to_string(),
            "Test memory".to_string(),
            0.8,
            "test".to_string(),
            vec!["test".to_string()],
        ).await.unwrap();
        
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_memories, 1);
    }

    #[tokio::test]
    async fn test_pin_memory() {
        let manager = MemoryDecayManager::with_default();
        
        manager.add_memory(
            "mem1".to_string(),
            "Test memory".to_string(),
            0.8,
            "test".to_string(),
            vec![],
        ).await.unwrap();
        
        manager.pin_memory("mem1").await.unwrap();
        
        let memory = manager.get_memory("mem1").await.unwrap();
        assert!(memory.is_pinned);
        assert_eq!(memory.retention, 1.0);
    }

    #[tokio::test]
    async fn test_decay_cycle() {
        let manager = MemoryDecayManager::with_default();
        
        manager.add_memory(
            "mem1".to_string(),
            "Test memory".to_string(),
            0.5,
            "test".to_string(),
            vec![],
        ).await.unwrap();
        
        let result = manager.run_decay_cycle().await;
        assert!(result.memories_needing_review <= 1);
    }
}