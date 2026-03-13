//! Predictive Memory - Anticipatory memory preloading and caching
//!
//! This module predicts user needs and preloads relevant memories:
//! - Context-based prediction
//! - Pattern-based preloading
//! - Intent prediction
//! - Proactive memory warming
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    Predictive Memory                                │
//! │                                                                      │
//! │   User Input ──→  Predict Intent  ──→  Preload Memories            │
//! │                        │                                           │
//! │                        ▼                                           │
//! │   ┌────────────────────────────────────────────────────────────┐   │
//! │   │                    Prediction Models                       │   │
//! │   │  • Context Similarity (what's similar to current)          │   │
//! │   │  • Sequence Patterns (what usually follows)                │   │
//! │   │  • Intent Classification (what's the goal)                 │   │
//! │   │  • Temporal Patterns (time-based predictions)              │   │
//! │   └────────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────┘

use std::sync::Arc;
use std::collections::{HashMap, VecDeque};
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn, debug};
use chrono::{DateTime, Utc, Timelike};
use serde::{Deserialize, Serialize};

use crate::events::{AgentEvent, EventBus};
use crate::memory::manager::MemoryManager;
use crate::knowledge::vector_store::VectorStore;
use crate::cognitive::llm::LlmClient;
use crate::types::Message;
use crate::error::Result;

/// Configuration for Predictive Memory
#[derive(Debug, Clone)]
pub struct PredictiveMemoryConfig {
    /// Enable predictive preloading
    pub enable_preloading: bool,
    /// Maximum memories to preload per prediction
    pub max_preload_memories: usize,
    /// Confidence threshold for predictions
    pub prediction_confidence_threshold: f32,
    /// Pattern history window size
    pub pattern_history_size: usize,
    /// Enable temporal predictions
    pub enable_temporal_predictions: bool,
    /// Enable sequence predictions
    pub enable_sequence_predictions: bool,
    /// Cache TTL for predictions
    pub prediction_cache_ttl: Duration,
}

impl Default for PredictiveMemoryConfig {
    fn default() -> Self {
        Self {
            enable_preloading: true,
            max_preload_memories: 10,
            prediction_confidence_threshold: 0.6,
            pattern_history_size: 100,
            enable_temporal_predictions: true,
            enable_sequence_predictions: true,
            prediction_cache_ttl: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// A prediction about user needs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    pub id: String,
    pub predicted_intent: String,
    pub confidence: f32,
    pub predicted_memories: Vec<PredictedMemory>,
    pub context_signals: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// A memory predicted to be needed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedMemory {
    pub memory_id: String,
    pub memory_type: MemoryType,
    pub relevance_score: f32,
    pub content: String,
    pub source: PredictionSource,
}

/// Type of memory
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MemoryType {
    Episodic,
    Semantic,
    Core,
    Working,
}

/// Source of prediction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PredictionSource {
    ContextSimilarity,
    SequencePattern,
    IntentMatch,
    TemporalPattern,
    UserPreference,
}

/// Pattern in user behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorPattern {
    pub id: String,
    pub pattern_type: PatternType,
    pub sequence: Vec<String>,
    pub frequency: u32,
    pub last_observed: DateTime<Utc>,
    pub confidence: f32,
}

/// Type of behavior pattern
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PatternType {
    QuerySequence,
    TopicTransition,
    TimeBased,
    ToolUsage,
}

/// Statistics for Predictive Memory
#[derive(Debug, Clone, Default)]
pub struct PredictiveMemoryStats {
    pub total_predictions: u64,
    pub accurate_predictions: u64,
    pub memories_preloaded: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub patterns_learned: u64,
    pub avg_prediction_confidence: f32,
    pub last_prediction: Option<DateTime<Utc>>,
}

/// Predictive Memory system
pub struct PredictiveMemory {
    config: PredictiveMemoryConfig,
    event_bus: Arc<EventBus>,
    memory_manager: Arc<MemoryManager>,
    vector_store: Option<Arc<VectorStore>>,
    llm: Arc<Box<dyn LlmClient>>,
    /// Pattern history
    pattern_history: Arc<RwLock<VecDeque<String>>>,
    /// Learned patterns
    patterns: Arc<RwLock<Vec<BehaviorPattern>>>,
    /// Prediction cache
    prediction_cache: Arc<RwLock<HashMap<String, Prediction>>>,
    /// Statistics
    stats: Arc<RwLock<PredictiveMemoryStats>>,
    /// Preloaded memories
    preloaded_memories: Arc<RwLock<Vec<PredictedMemory>>>,
}

impl PredictiveMemory {
    pub fn new(
        config: PredictiveMemoryConfig,
        event_bus: Arc<EventBus>,
        memory_manager: Arc<MemoryManager>,
        vector_store: Option<Arc<VectorStore>>,
        llm: Arc<Box<dyn LlmClient>>,
    ) -> Self {
        Self {
            config,
            event_bus,
            memory_manager,
            vector_store,
            llm,
            pattern_history: Arc::new(RwLock::new(VecDeque::with_capacity(100))),
            patterns: Arc::new(RwLock::new(Vec::new())),
            prediction_cache: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(PredictiveMemoryStats::default())),
            preloaded_memories: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Record user input for pattern learning
    pub async fn record_input(&self, input: &str, context: &str) {
        // Add to pattern history
        let mut history = self.pattern_history.write().await;
        history.push_back(input.to_string());
        
        // Trim to max size
        while history.len() > self.config.pattern_history_size {
            history.pop_front();
        }
        drop(history);

        // Learn patterns from updated history
        if self.config.enable_sequence_predictions {
            self.learn_patterns().await;
        }

        // Generate prediction
        if self.config.enable_preloading {
            if let Err(e) = self.predict_and_preload(input, context).await {
                warn!("Failed to generate prediction: {}", e);
            }
        }
    }

    /// Learn patterns from history
    async fn learn_patterns(&self) {
        let history = self.pattern_history.read().await;
        
        if history.len() < 3 {
            return;
        }

        let history_vec: Vec<_> = history.iter().cloned().collect();
        drop(history);

        // Look for repeating sequences
        let mut patterns = self.patterns.write().await;
        
        for window_size in 2..=5 {
            for i in 0..=history_vec.len().saturating_sub(window_size * 2) {
                let sequence: Vec<_> = history_vec[i..i + window_size].to_vec();
                
                // Check if this sequence repeats
                let mut count = 1;
                for j in (i + window_size)..=history_vec.len().saturating_sub(window_size) {
                    if history_vec[j..j + window_size] == sequence[..] {
                        count += 1;
                    }
                }
                
                // If sequence repeats frequently, add as pattern
                if count >= 3 {
                    let pattern_exists = patterns.iter().any(|p| {
                        p.pattern_type == PatternType::QuerySequence &&
                        p.sequence == sequence
                    });
                    
                    if !pattern_exists {
                        let pattern = BehaviorPattern {
                            id: uuid::Uuid::new_v4().to_string(),
                            pattern_type: PatternType::QuerySequence,
                            sequence: sequence.clone(),
                            frequency: count,
                            last_observed: Utc::now(),
                            confidence: (count as f32 / 10.0).min(1.0),
                        };
                        
                        patterns.push(pattern);
                        
                        // Update stats
                        self.stats.write().await.patterns_learned += 1;
                        
                        debug!("Learned new pattern: {:?}", sequence);
                    }
                }
            }
        }
    }

    /// Generate prediction and preload memories
    async fn predict_and_preload(&self, current_input: &str, context: &str) -> Result<()> {
        let start = std::time::Instant::now();
        
        // Check cache first
        let cache_key = format!("{}:{}", current_input, context);
        {
            let cache = self.prediction_cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                if Utc::now() < cached.expires_at {
                    // Use cached prediction
                    self.preload_memories(&cached.predicted_memories).await?;
                    self.stats.write().await.cache_hits += 1;
                    return Ok(());
                }
            }
        }
        
        self.stats.write().await.cache_misses += 1;

        // Generate new prediction
        let prediction = self.generate_prediction(current_input, context).await?;
        
        // Cache prediction
        {
            let mut cache = self.prediction_cache.write().await;
            cache.insert(cache_key, prediction.clone());
            
            // Clean expired entries
            let now = Utc::now();
            cache.retain(|_, v| v.expires_at > now);
        }

        // Preload predicted memories
        if prediction.confidence >= self.config.prediction_confidence_threshold {
            self.preload_memories(&prediction.predicted_memories).await?;
            
            // Update stats
            let mut stats = self.stats.write().await;
            stats.total_predictions += 1;
            stats.memories_preloaded += prediction.predicted_memories.len() as u64;
            stats.last_prediction = Some(Utc::now());
            
            // Update average confidence
            if stats.total_predictions == 1 {
                stats.avg_prediction_confidence = prediction.confidence;
            } else {
                stats.avg_prediction_confidence = 
                    (stats.avg_prediction_confidence * (stats.total_predictions - 1) as f32 + prediction.confidence)
                    / stats.total_predictions as f32;
            }
        }

        debug!(
            "Prediction generated in {:?}: intent='{}', confidence={}",
            start.elapsed(),
            prediction.predicted_intent,
            prediction.confidence
        );

        Ok(())
    }

    /// Generate a prediction based on current context
    async fn generate_prediction(&self, current_input: &str, context: &str) -> Result<Prediction> {
        let mut predicted_memories = Vec::new();
        let mut context_signals = Vec::new();

        // 1. Context-based prediction using vector similarity
        if let Some(vs) = &self.vector_store {
            // Search for similar past contexts
            // let similar = vs.search_similar(current_input, 5).await?;
            // For each similar context, get associated memories
        }

        // 2. Pattern-based prediction
        let patterns = self.patterns.read().await;
        for pattern in patterns.iter() {
            if pattern.pattern_type == PatternType::QuerySequence {
                // Check if current input matches start of pattern
                if pattern.sequence.len() > 1 && pattern.sequence[0] == current_input {
                    // Predict next items in sequence
                    for next_item in &pattern.sequence[1..] {
                        predicted_memories.push(PredictedMemory {
                            memory_id: uuid::Uuid::new_v4().to_string(),
                            memory_type: MemoryType::Episodic,
                            relevance_score: pattern.confidence,
                            content: next_item.clone(),
                            source: PredictionSource::SequencePattern,
                        });
                    }
                    context_signals.push(format!("pattern_match:{}", pattern.id));
                }
            }
        }
        drop(patterns);

        // 3. Intent-based prediction using LLM
        let intent = self.predict_intent(current_input, context).await?;
        context_signals.push(format!("intent:{}", intent));

        // 4. Temporal prediction
        if self.config.enable_temporal_predictions {
            let temporal_memories = self.predict_temporal().await?;
            predicted_memories.extend(temporal_memories);
        }

        // Calculate overall confidence
        let confidence = if predicted_memories.is_empty() {
            0.0
        } else {
            predicted_memories.iter().map(|m| m.relevance_score).sum::<f32>() 
                / predicted_memories.len() as f32
        };

        let prediction = Prediction {
            id: uuid::Uuid::new_v4().to_string(),
            predicted_intent: intent,
            confidence,
            predicted_memories,
            context_signals,
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::from_std(self.config.prediction_cache_ttl).unwrap_or(chrono::Duration::minutes(5)),
        };

        Ok(prediction)
    }

    /// Predict user intent using LLM
    async fn predict_intent(&self, input: &str, context: &str) -> Result<String> {
        let prompt = format!(
            "Based on the following context and input, predict the user's intent:\n\n\
            Context: {}\n\
            Input: {}\n\n\
            Intent (brief description):",
            context, input
        );

        match self.llm.chat_complete(&[Message::system(&prompt)]).await {
            Ok(intent) => Ok(intent.trim().to_string()),
            Err(e) => {
                warn!("Failed to predict intent: {}", e);
                Ok("unknown".to_string())
            }
        }
    }

    /// Predict based on temporal patterns
    async fn predict_temporal(&self) -> Result<Vec<PredictedMemory>> {
        let mut memories = Vec::new();
        let now = Utc::now();
        let hour = now.hour();
        let day_of_week = now.weekday().num_days_from_monday();

        // Time-based predictions
        if hour >= 9 && hour <= 17 {
            // Work hours - predict work-related memories
            memories.push(PredictedMemory {
                memory_id: uuid::Uuid::new_v4().to_string(),
                memory_type: MemoryType::Core,
                relevance_score: 0.6,
                content: "work_context".to_string(),
                source: PredictionSource::TemporalPattern,
            });
        } else {
            // Non-work hours
            memories.push(PredictedMemory {
                memory_id: uuid::Uuid::new_v4().to_string(),
                memory_type: MemoryType::Core,
                relevance_score: 0.5,
                content: "personal_context".to_string(),
                source: PredictionSource::TemporalPattern,
            });
        }

        // Day-based predictions
        if day_of_week < 5 {
            // Weekday
            memories.push(PredictedMemory {
                memory_id: uuid::Uuid::new_v4().to_string(),
                memory_type: MemoryType::Semantic,
                relevance_score: 0.55,
                content: "weekday_routine".to_string(),
                source: PredictionSource::TemporalPattern,
            });
        }

        Ok(memories)
    }

    /// Preload memories into working memory
    async fn preload_memories(&self, memories: &[PredictedMemory]) -> Result<()> {
        let mut preloaded = self.preloaded_memories.write().await;
        
        for memory in memories.iter().take(self.config.max_preload_memories) {
            // In a real implementation, this would:
            // 1. Retrieve the actual memory content
            // 2. Load it into appropriate memory tier
            // 3. Mark as preloaded
            
            preloaded.push(memory.clone());
            debug!("Preloaded memory: {} (score: {})", memory.memory_id, memory.relevance_score);
        }

        // Trim preloaded list
        while preloaded.len() > self.config.max_preload_memories * 2 {
            preloaded.remove(0);
        }

        Ok(())
    }

    /// Get preloaded memories relevant to current context
    pub async fn get_relevant_preloaded(&self, context: &str) -> Vec<PredictedMemory> {
        let preloaded = self.preloaded_memories.read().await;
        
        // Filter by relevance score
        preloaded
            .iter()
            .filter(|m| m.relevance_score >= self.config.prediction_confidence_threshold)
            .cloned()
            .collect()
    }

    /// Record prediction accuracy feedback
    pub async fn record_feedback(&self, prediction_id: &str, was_accurate: bool) {
        if was_accurate {
            self.stats.write().await.accurate_predictions += 1;
            debug!("Prediction {} was accurate", prediction_id);
        } else {
            debug!("Prediction {} was inaccurate", prediction_id);
        }

        // Could use this feedback to improve future predictions
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> PredictiveMemoryStats {
        self.stats.read().await.clone()
    }

    /// Get learned patterns
    pub async fn get_patterns(&self) -> Vec<BehaviorPattern> {
        self.patterns.read().await.clone()
    }

    /// Clear prediction cache
    pub async fn clear_cache(&self) {
        self.prediction_cache.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prediction_creation() {
        let prediction = Prediction {
            id: "test".to_string(),
            predicted_intent: "coding_assistance".to_string(),
            confidence: 0.85,
            predicted_memories: vec![],
            context_signals: vec!["pattern_match".to_string()],
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::minutes(5),
        };

        assert_eq!(prediction.predicted_intent, "coding_assistance");
        assert_eq!(prediction.confidence, 0.85);
    }

    #[test]
    fn test_predicted_memory_creation() {
        let memory = PredictedMemory {
            memory_id: "mem1".to_string(),
            memory_type: MemoryType::Episodic,
            relevance_score: 0.9,
            content: "Previous Rust project".to_string(),
            source: PredictionSource::ContextSimilarity,
        };

        assert_eq!(memory.memory_type, MemoryType::Episodic);
        assert_eq!(memory.source, PredictionSource::ContextSimilarity);
    }

    #[test]
    fn test_behavior_pattern_creation() {
        let pattern = BehaviorPattern {
            id: "pat1".to_string(),
            pattern_type: PatternType::QuerySequence,
            sequence: vec!["hello".to_string(), "how are you".to_string()],
            frequency: 5,
            last_observed: Utc::now(),
            confidence: 0.8,
        };

        assert_eq!(pattern.frequency, 5);
        assert_eq!(pattern.sequence.len(), 2);
    }

    #[test]
    fn test_predictive_memory_config_default() {
        let config = PredictiveMemoryConfig::default();
        assert!(config.enable_preloading);
        assert_eq!(config.max_preload_memories, 10);
        assert!(config.enable_temporal_predictions);
        assert!(config.enable_sequence_predictions);
    }
}
