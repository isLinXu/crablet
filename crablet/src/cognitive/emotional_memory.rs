//! Emotional Memory Fusion System
//!
//! Analyzes user emotional states and fuses emotional context into memories for personalized interactions.
//!
//! # Features
//!
//! - Real-time emotion detection from user messages
//! - Emotional memory tagging and indexing
//! - Mood-aware response generation
//! - Emotional pattern tracking across sessions
//! - Sentiment-based memory prioritization
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                  Emotional Memory Fusion                              │
//! │                                                                      │
//! │   ┌─────────────┐    ┌─────────────┐    ┌──────────────────────┐   │
//! │   │   Emotion   │───→│  Emotional  │───→│   Memory Fusion       │   │
//! │   │  Detector  │    │  Tagger     │    │                      │   │
//! │   └─────────────┘    └─────────────┘    └──────────────────────┘   │
//! │          │                  │                     │                │
//! │          ▼                  ▼                     ▼                │
//! │   ┌────────────────────────────────────────────────────────────┐   │
//! │   │                  Emotion Categories                         │   │
//! │   │  Joy │ Sadness │ Anger │ Fear │ Surprise │ Trust │ ...   │   │
//! │   └────────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tracing::info;

/// Emotion categories for analysis
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Emotion {
    /// Positive emotions
    Joy,
    Satisfaction,
    Surprise,
    Excitement,
    Relief,
    
    /// Negative emotions
    Sadness,
    Frustration,
    Anger,
    Fear,
    Anxiety,
    Disappointment,
    
    /// Neutral/Complex emotions
    Confusion,
    Curiosity,
    Neutral,
    Trust,
    Skepticism,
}

impl Emotion {
    /// Get emotion valence (-1.0 to 1.0)
    pub fn valence(&self) -> f32 {
        match self {
            Emotion::Joy => 0.9,
            Emotion::Satisfaction => 0.7,
            Emotion::Surprise => 0.3,
            Emotion::Excitement => 0.8,
            Emotion::Relief => 0.6,
            Emotion::Sadness => -0.8,
            Emotion::Frustration => -0.6,
            Emotion::Anger => -0.7,
            Emotion::Fear => -0.6,
            Emotion::Anxiety => -0.5,
            Emotion::Disappointment => -0.5,
            Emotion::Confusion => -0.2,
            Emotion::Curiosity => 0.2,
            Emotion::Neutral => 0.0,
            Emotion::Trust => 0.6,
            Emotion::Skepticism => -0.3,
        }
    }
    
    /// Get emotion arousal (0.0 to 1.0)
    pub fn arousal(&self) -> f32 {
        match self {
            Emotion::Joy => 0.7,
            Emotion::Satisfaction => 0.4,
            Emotion::Surprise => 0.9,
            Emotion::Excitement => 0.9,
            Emotion::Relief => 0.5,
            Emotion::Sadness => 0.4,
            Emotion::Frustration => 0.7,
            Emotion::Anger => 0.8,
            Emotion::Fear => 0.8,
            Emotion::Anxiety => 0.7,
            Emotion::Disappointment => 0.5,
            Emotion::Confusion => 0.6,
            Emotion::Curiosity => 0.6,
            Emotion::Neutral => 0.1,
            Emotion::Trust => 0.4,
            Emotion::Skepticism => 0.3,
        }
    }
    
    /// Get emotion color for visualization
    pub fn color(&self) -> &'static str {
        match self {
            Emotion::Joy => "#FFD700",
            Emotion::Satisfaction => "#90EE90",
            Emotion::Surprise => "#FF69B4",
            Emotion::Excitement => "#FF4500",
            Emotion::Relief => "#87CEEB",
            Emotion::Sadness => "#4682B4",
            Emotion::Frustration => "#FF6347",
            Emotion::Anger => "#DC143C",
            Emotion::Fear => "#8B0000",
            Emotion::Anxiety => "#DDA0DD",
            Emotion::Disappointment => "#708090",
            Emotion::Confusion => "#D2691E",
            Emotion::Curiosity => "#00CED1",
            Emotion::Neutral => "#C0C0C0",
            Emotion::Trust => "#4169E1",
            Emotion::Skepticism => "#808080",
        }
    }
}

/// Detected emotion with confidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedEmotion {
    pub emotion: Emotion,
    pub confidence: f32,
    pub indicators: Vec<String>,
}

impl DetectedEmotion {
    pub fn new(emotion: Emotion, confidence: f32) -> Self {
        Self {
            emotion,
            confidence,
            indicators: Vec::new(),
        }
    }
    
    pub fn with_indicator(mut self, indicator: String) -> Self {
        self.indicators.push(indicator);
        self
    }
}

/// Emotional state of a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalState {
    pub session_id: String,
    pub primary_emotion: Emotion,
    pub secondary_emotion: Option<Emotion>,
    pub intensity: f32,  // 0.0 - 1.0
    pub valence: f32,     // -1.0 to 1.0
    pub arousal: f32,     // 0.0 to 1.0
    pub emotional_history: Vec<EmotionalSnapshot>,
    pub updated_at: DateTime<Utc>,
}

/// Snapshot of emotion at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalSnapshot {
    pub timestamp: DateTime<Utc>,
    pub emotion: Emotion,
    pub intensity: f32,
    pub message_preview: String,
}

/// Emotional memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalMemory {
    pub id: String,
    pub content: String,
    pub emotions: Vec<Emotion>,
    pub average_valence: f32,
    pub average_arousal: f32,
    pub importance: f32,
    pub emotional_impact: f32,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub access_count: u32,
    pub source_session: String,
    pub tags: Vec<String>,
}

impl EmotionalMemory {
    pub fn new(
        id: String,
        content: String,
        emotions: Vec<Emotion>,
        source_session: String,
    ) -> Self {
        let avg_valence = emotions.iter().map(|e| e.valence()).sum::<f32>() / emotions.len() as f32;
        let avg_arousal = emotions.iter().map(|e| e.arousal()).sum::<f32>() / emotions.len() as f32;
        
        Self {
            id,
            content,
            emotions,
            average_valence: avg_valence,
            average_arousal: avg_arousal,
            importance: 0.5,
            emotional_impact: avg_valence.abs(),
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            access_count: 0,
            source_session,
            tags: Vec::new(),
        }
    }
}

/// Emotional pattern detected across sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalPattern {
    pub id: String,
    pub pattern_type: PatternType,
    pub trigger_keywords: Vec<String>,
    pub typical_emotion: Emotion,
    pub frequency: u32,
    pub last_observed: DateTime<Utc>,
    pub sessions_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternType {
    UserHappyWhen,
    UserFrustratedWhen,
    UserConfusedWhen,
    UserCuriousAbout,
    UserTrustsTopic,
    UserSkepticalAbout,
}

/// Emotional context for response generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalContext {
    pub current_mood: Emotion,
    pub mood_intensity: f32,
    pub emotional_goal: Option<EmotionalGoal>,
    pub avoid_emotions: Vec<Emotion>,
    pub recent_positive_count: u32,
    pub recent_negative_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmotionalGoal {
    /// Try to improve user's mood
    Brighten,
    /// Provide comfort
    Comfort,
    /// Be more professional
    Professional,
    /// Match user's energy
    MatchEnergy,
    /// Calm user down
    Calm,
}

/// Configuration for emotional analysis
#[derive(Debug, Clone)]
pub struct EmotionalConfig {
    pub detection_sensitivity: f32,
    pub memory_importance_threshold: f32,
    pub pattern_min_frequency: u32,
    pub mood_decay_rate: f32,
    pub enable_emotional_memory: bool,
    pub enable_pattern_detection: bool,
}

impl Default for EmotionalConfig {
    fn default() -> Self {
        Self {
            detection_sensitivity: 0.6,
            memory_importance_threshold: 0.3,
            pattern_min_frequency: 3,
            mood_decay_rate: 0.1,
            enable_emotional_memory: true,
            enable_pattern_detection: true,
        }
    }
}

/// Emotion detection patterns
#[derive(Debug, Clone)]
pub struct EmotionPatterns {
    positive_patterns: Vec<(Emotion, Vec<&'static str>)>,
    negative_patterns: Vec<(Emotion, Vec<&'static str>)>,
    neutral_patterns: Vec<(Emotion, Vec<&'static str>)>,
}

impl EmotionPatterns {
    pub fn new() -> Self {
        Self {
            positive_patterns: vec![
                (Emotion::Joy, vec![
                    "太好了", "完美", "太棒了", "非常好", "棒", "优秀", "厉害",
                    "great", "perfect", "excellent", "awesome", "amazing", "wonderful",
                ]),
                (Emotion::Satisfaction, vec![
                    "满意", "还不错", "可以", "还行", "ok", "okay",
                    "satisfied", "good", "fine", "acceptable",
                ]),
                (Emotion::Excitement, vec![
                    "太激动了", "期待", "好兴奋", "迫不及待",
                    "excited", "can't wait", "looking forward", "thrilled",
                ]),
                (Emotion::Relief, vec![
                    "终于", "松了口气", "好险", "解决了",
                    "finally", "relief", "thank god", "sorted out",
                ]),
                (Emotion::Surprise, vec![
                    "真的吗", "没想到", "惊了", "哇",
                    "really?", "wow", "surprised", "unexpected", "omg",
                ]),
                (Emotion::Trust, vec![
                    "相信", "信任", "靠谱", "放心",
                    "trust", "believe", "reliable", "confident",
                ]),
            ],
            negative_patterns: vec![
                (Emotion::Frustration, vec![
                    "烦", "头疼", "难", "搞不懂", "搞不定",
                    "frustrated", "annoying", "difficult", "troublesome", "can't figure out",
                ]),
                (Emotion::Anger, vec![
                    "生气", "愤怒", "讨厌", "烦人", "垃圾",
                    "angry", "mad", "hate", "terrible", "awful", "worst",
                ]),
                (Emotion::Sadness, vec![
                    "难过", "伤心", "失落", "沮丧",
                    "sad", "upset", "disappointed", "depressed", "unhappy",
                ]),
                (Emotion::Fear, vec![
                    "担心", "害怕", "怕", "紧张",
                    "worried", "afraid", "scared", "nervous", "anxious",
                ]),
                (Emotion::Anxiety, vec![
                    "焦虑", "不安", "着急", "急",
                    "anxious", "worried", "uneasy", "pressured", "urgent",
                ]),
                (Emotion::Disappointment, vec![
                    "失望", "不行", "没用", "浪费",
                    "disappointed", "failed", "useless", "waste", "doesn't work",
                ]),
            ],
            neutral_patterns: vec![
                (Emotion::Confusion, vec![
                    "不懂", "什么意思", "不明白", "哪个",
                    "don't understand", "what do you mean", "confused", "which one",
                ]),
                (Emotion::Curiosity, vec![
                    "为什么", "怎么", "是什么", "好奇",
                    "why", "how", "what is", "wondering", "curious",
                ]),
                (Emotion::Neutral, vec![
                    "好", "嗯", "哦", "行",
                    "ok", "okay", "sure", "alright", "fine",
                ]),
                (Emotion::Skepticism, vec![
                    "真的吗", "不太信", "不确定", "怀疑",
                    "really?", "not sure", "doubtful", "skeptical", "suspicious",
                ]),
            ],
        }
    }
    
    /// Detect emotions from text
    pub fn detect(&self, text: &str) -> Vec<DetectedEmotion> {
        let text_lower = text.to_lowercase();
        let mut results: Vec<DetectedEmotion> = Vec::new();
        
        // Check positive patterns
        for (emotion, patterns) in &self.positive_patterns {
            for pattern in patterns {
                if text_lower.contains(&pattern.to_lowercase()) {
                    let confidence = self.calculate_confidence(&text_lower, pattern);
                    results.push(DetectedEmotion::new(*emotion, confidence)
                        .with_indicator(pattern.to_string()));
                }
            }
        }
        
        // Check negative patterns
        for (emotion, patterns) in &self.negative_patterns {
            for pattern in patterns {
                if text_lower.contains(&pattern.to_lowercase()) {
                    let confidence = self.calculate_confidence(&text_lower, pattern);
                    results.push(DetectedEmotion::new(*emotion, confidence)
                        .with_indicator(pattern.to_string()));
                }
            }
        }
        
        // Check neutral patterns
        for (emotion, patterns) in &self.neutral_patterns {
            for pattern in patterns {
                if text_lower.contains(&pattern.to_lowercase()) {
                    let confidence = self.calculate_confidence(&text_lower, pattern);
                    results.push(DetectedEmotion::new(*emotion, confidence)
                        .with_indicator(pattern.to_string()));
                }
            }
        }
        
        // Sort by confidence
        results.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        results
    }
    
    fn calculate_confidence(&self, text: &str, pattern: &str) -> f32 {
        let pattern_lower = pattern.to_lowercase();
        let count = text.matches(&pattern_lower).count();
        let base = 0.5 + (count as f32 * 0.1).min(0.4);
        base.min(1.0)
    }
}

/// Main emotional memory fusion system
pub struct EmotionalMemoryFusion {
    config: EmotionalConfig,
    patterns: EmotionPatterns,
    emotional_states: Arc<RwLock<HashMap<String, EmotionalState>>>,
    emotional_memories: Arc<RwLock<Vec<EmotionalMemory>>>,
    emotional_patterns: Arc<RwLock<Vec<EmotionalPattern>>>,
    memory_index: Arc<RwLock<HashMap<Emotion, Vec<usize>>>>,
}

impl EmotionalMemoryFusion {
    pub fn new(config: EmotionalConfig) -> Self {
        Self {
            config,
            patterns: EmotionPatterns::new(),
            emotional_states: Arc::new(RwLock::new(HashMap::new())),
            emotional_memories: Arc::new(RwLock::new(Vec::new())),
            emotional_patterns: Arc::new(RwLock::new(Vec::new())),
            memory_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    pub fn with_default() -> Self {
        Self::new(EmotionalConfig::default())
    }
    
    /// Analyze emotions in user message
    pub async fn analyze_message(&self, session_id: &str, message: &str) -> Vec<DetectedEmotion> {
        let detected = self.patterns.detect(message);
        
        // Update emotional state
        if !detected.is_empty() {
            self.update_emotional_state(session_id, &detected, message).await;
        }
        
        detected
    }
    
    /// Update emotional state for a session
    async fn update_emotional_state(
        &self,
        session_id: &str,
        emotions: &[DetectedEmotion],
        message_preview: &str,
    ) {
        let mut states = self.emotional_states.write().await;
        
        let state = states.entry(session_id.to_string())
            .or_insert_with(|| EmotionalState {
                session_id: session_id.to_string(),
                primary_emotion: Emotion::Neutral,
                secondary_emotion: None,
                intensity: 0.5,
                valence: 0.0,
                arousal: 0.1,
                emotional_history: Vec::new(),
                updated_at: Utc::now(),
            });
        
        // Find primary emotion (highest confidence)
        if let Some(primary) = emotions.first() {
            state.primary_emotion = primary.emotion;
            state.intensity = primary.confidence;
            state.valence = primary.emotion.valence();
            state.arousal = primary.emotion.arousal();
            
            // Find secondary emotion
            if emotions.len() > 1 {
                state.secondary_emotion = Some(emotions[1].emotion);
            }
        }
        
        state.updated_at = Utc::now();
        
        // Add to history
        state.emotional_history.push(EmotionalSnapshot {
            timestamp: Utc::now(),
            emotion: state.primary_emotion,
            intensity: state.intensity,
            message_preview: message_preview.chars().take(50).collect(),
        });
        
        // Keep only recent history
        if state.emotional_history.len() > 100 {
            state.emotional_history.remove(0);
        }
    }
    
    /// Get emotional state for a session
    pub async fn get_emotional_state(&self, session_id: &str) -> Option<EmotionalState> {
        let states = self.emotional_states.read().await;
        states.get(session_id).cloned()
    }
    
    /// Create emotional memory from content
    pub async fn create_emotional_memory(
        &self,
        content: String,
        emotions: Vec<Emotion>,
        source_session: String,
    ) -> String {
        if !self.config.enable_emotional_memory {
            return String::new();
        }
        
        let id = format!("emot_{}", uuid::Uuid::new_v4().to_string().replace("-", "")[..12].to_string());
        let memory = EmotionalMemory::new(
            id.clone(),
            content.clone(),
            emotions.clone(),
            source_session,
        );
        
        // Store memory
        let mut memories = self.emotional_memories.write().await;
        let idx = memories.len();
        memories.push(memory);
        
        // Update index
        let mut index = self.memory_index.write().await;
        for emotion in &emotions {
            index.entry(*emotion)
                .or_default()
                .push(idx);
        }
        
        // Detect patterns
        if self.config.enable_pattern_detection {
            self.detect_and_record_patterns(&content, &emotions).await;
        }
        
        info!("Created emotional memory: {} with emotions {:?}", id, emotions);
        id
    }
    
    /// Detect emotional patterns
    async fn detect_and_record_patterns(&self, content: &str, emotions: &[Emotion]) {
        if let Some(primary) = emotions.first() {
            let text_lower = content.to_lowercase();
            
            // Extract potential trigger words
            let words: Vec<&str> = text_lower.split_whitespace()
                .filter(|w| w.len() > 2)
                .take(5)
                .collect();
            
            let mut patterns = self.emotional_patterns.write().await;
            
            // Look for existing pattern to update
            for pattern in patterns.iter_mut() {
                if pattern.typical_emotion == *primary {
                    // Check if any trigger words match
                    for word in &words {
                        if pattern.trigger_keywords.iter().any(|k| k.contains(word) || word.contains(k)) {
                            pattern.frequency += 1;
                            pattern.last_observed = Utc::now();
                        }
                    }
                    return;
                }
            }
            
            // Create new pattern
            let new_pattern = EmotionalPattern {
                id: format!("epat_{}", uuid::Uuid::new_v4().to_string().replace("-", "")[..8].to_string()),
                pattern_type: match primary {
                    Emotion::Joy => PatternType::UserHappyWhen,
                    Emotion::Frustration => PatternType::UserFrustratedWhen,
                    Emotion::Confusion => PatternType::UserConfusedWhen,
                    Emotion::Curiosity => PatternType::UserCuriousAbout,
                    Emotion::Trust => PatternType::UserTrustsTopic,
                    Emotion::Skepticism => PatternType::UserSkepticalAbout,
                    _ => PatternType::UserHappyWhen,
                },
                trigger_keywords: words.iter().map(|s| s.to_string()).collect(),
                typical_emotion: *primary,
                frequency: 1,
                last_observed: Utc::now(),
                sessions_count: 1,
            };
            
            patterns.push(new_pattern);
        }
    }
    
    /// Search emotional memories by emotion
    pub async fn search_by_emotion(&self, emotion: Emotion, limit: usize) -> Vec<EmotionalMemory> {
        let index = self.memory_index.read().await;
        let memories = self.emotional_memories.read().await;
        
        if let Some(indices) = index.get(&emotion) {
            indices
                .iter()
                .take(limit)
                .filter_map(|&idx| memories.get(idx).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }
    
    /// Get emotional memories with positive valence
    pub async fn get_positive_memories(&self, limit: usize) -> Vec<EmotionalMemory> {
        let memories = self.emotional_memories.read().await;
        let mut positive: Vec<_> = memories
            .iter()
            .filter(|m| m.average_valence > 0.3)
            .cloned()
            .collect();
        
        positive.sort_by(|a, b| b.emotional_impact.partial_cmp(&a.emotional_impact).unwrap());
        positive.truncate(limit);
        positive
    }
    
    /// Get emotional context for response generation
    pub async fn get_emotional_context(&self, session_id: &str) -> EmotionalContext {
        let states = self.emotional_states.read().await;
        
        if let Some(state) = states.get(session_id) {
            let recent_positive = state.emotional_history
                .iter()
                .filter(|s| s.emotion.valence() > 0.3)
                .count() as u32;
            
            let recent_negative = state.emotional_history
                .iter()
                .filter(|s| s.emotion.valence() < -0.3)
                .count() as u32;
            
            let emotional_goal = self.determine_emotional_goal(state);
            
            EmotionalContext {
                current_mood: state.primary_emotion,
                mood_intensity: state.intensity,
                emotional_goal,
                avoid_emotions: self.get_emotions_to_avoid(state),
                recent_positive_count: recent_positive,
                recent_negative_count: recent_negative,
            }
        } else {
            EmotionalContext {
                current_mood: Emotion::Neutral,
                mood_intensity: 0.5,
                emotional_goal: None,
                avoid_emotions: vec![],
                recent_positive_count: 0,
                recent_negative_count: 0,
            }
        }
    }
    
    fn determine_emotional_goal(&self, state: &EmotionalState) -> Option<EmotionalGoal> {
        match state.primary_emotion {
            Emotion::Frustration | Emotion::Anger | Emotion::Anxiety => {
                Some(EmotionalGoal::Calm)
            }
            Emotion::Sadness => Some(EmotionalGoal::Comfort),
            Emotion::Confusion => Some(EmotionalGoal::Professional),
            _ if state.valence < -0.3 => Some(EmotionalGoal::Brighten),
            _ => None,
        }
    }
    
    fn get_emotions_to_avoid(&self, state: &EmotionalState) -> Vec<Emotion> {
        match state.primary_emotion {
            Emotion::Anger | Emotion::Frustration => {
                vec![Emotion::Anger, Emotion::Skepticism]
            }
            Emotion::Sadness => {
                vec![Emotion::Disappointment, Emotion::Fear]
            }
            Emotion::Anxiety | Emotion::Fear => {
                vec![Emotion::Anxiety, Emotion::Disappointment]
            }
            _ => vec![],
        }
    }
    
    /// Adjust response tone based on emotional context
    pub fn adjust_response_tone(&self, response: &str, context: &EmotionalContext) -> String {
        let mut adjusted = response.to_string();
        
        // Apply emotional goal modifications
        match context.emotional_goal {
            Some(EmotionalGoal::Brighten) => {
                // Add positive framing if response is neutral
                if context.current_mood.valence() < 0.0 {
                    adjusted = format!("{} 😊", adjusted);
                }
            }
            Some(EmotionalGoal::Comfort) => {
                // Add supportive elements
                if !adjusted.contains("理解") && !adjusted.contains("明白") {
                    adjusted = format!("我理解你的感受。{}", adjusted);
                }
            }
            Some(EmotionalGoal::Calm) => {
                // Use reassuring language
                if context.current_mood == Emotion::Anxiety || context.current_mood == Emotion::Fear {
                    adjusted = format!("别担心，让我来帮你分析一下。{}", adjusted);
                }
            }
            _ => {}
        }
        
        adjusted
    }
    
    /// Get emotional statistics
    pub async fn get_stats(&self) -> EmotionalStats {
        let states = self.emotional_states.read().await;
        let memories = self.emotional_memories.read().await;
        let patterns = self.emotional_patterns.read().await;
        
        let emotion_counts: HashMap<Emotion, u32> = {
            let mut counts: HashMap<Emotion, u32> = HashMap::new();
            for memory in memories.iter() {
                if let Some(emotion) = memory.emotions.first() {
                    *counts.entry(*emotion).or_insert(0) += 1;
                }
            }
            counts
        };
        
        EmotionalStats {
            active_sessions: states.len(),
            total_emotional_memories: memories.len(),
            detected_patterns: patterns.len(),
            emotion_distribution: emotion_counts,
        }
    }
    
    /// Record memory access for importance tracking
    pub async fn record_memory_access(&self, memory_id: &str) {
        let mut memories = self.emotional_memories.write().await;
        if let Some(memory) = memories.iter_mut().find(|m| m.id == memory_id) {
            memory.access_count += 1;
            memory.last_accessed = Utc::now();
        }
    }
}

/// Emotional statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmotionalStats {
    pub active_sessions: usize,
    pub total_emotional_memories: usize,
    pub detected_patterns: usize,
    pub emotion_distribution: HashMap<Emotion, u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_emotion_detection() {
        let fusion = EmotionalMemoryFusion::with_default();
        
        let emotions = fusion.analyze_message("session_1", "太棒了！这个问题终于解决了！").await;
        assert!(!emotions.is_empty());
        assert!(emotions[0].emotion == Emotion::Joy || emotions[0].emotion == Emotion::Relief);
    }

    #[test]
    fn test_emotion_valence() {
        assert!(Emotion::Joy.valence() > 0.0);
        assert!(Emotion::Sadness.valence() < 0.0);
        assert!(Emotion::Neutral.valence() == 0.0);
    }

    #[tokio::test]
    async fn test_emotional_state_tracking() {
        let fusion = EmotionalMemoryFusion::with_default();
        
        fusion.analyze_message("session_test", "这个问题太难了，我真的很困惑").await;
        
        let state = fusion.get_emotional_state("session_test").await;
        assert!(state.is_some());
        let state = state.unwrap();
        assert!(state.primary_emotion == Emotion::Confusion || state.primary_emotion == Emotion::Frustration);
    }

    #[tokio::test]
    async fn test_emotional_memory_creation() {
        let fusion = EmotionalMemoryFusion::with_default();
        
        let memory_id = fusion.create_emotional_memory(
            "用户表达了对我服务的满意".to_string(),
            vec![Emotion::Joy, Emotion::Satisfaction],
            "session_1".to_string(),
        ).await;
        
        assert!(!memory_id.is_empty());
        
        let memories = fusion.search_by_emotion(Emotion::Joy, 10).await;
        assert!(!memories.is_empty());
    }

    #[tokio::test]
    async fn test_response_tone_adjustment() {
        let fusion = EmotionalMemoryFusion::with_default();
        
        let context = EmotionalContext {
            current_mood: Emotion::Sadness,
            mood_intensity: 0.7,
            emotional_goal: Some(EmotionalGoal::Comfort),
            avoid_emotions: vec![],
            recent_positive_count: 1,
            recent_negative_count: 3,
        };
        
        let adjusted = fusion.adjust_response_tone("这是你的结果。", &context);
        assert!(adjusted.contains("理解"));
    }
}