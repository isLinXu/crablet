//! Background Thinker - Advanced idle-time reflection and insight generation
//!
//! This module extends the basic heartbeat mechanism with sophisticated
//! background thinking capabilities:
//! - Deep conversation analysis
//! - Pattern recognition in user behavior
//! - Proactive insight generation
//! - User preference learning
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                    Background Thinker                               │
//! │                                                                      │
//! │   ┌─────────────┐    ┌─────────────┐    ┌──────────────────────┐   │
//! │   │  Trigger    │───→│  Analyze    │───→│  Generate Insights   │   │
//! │   │  (Idle)     │    │  Context    │    │                      │   │
//! │   └─────────────┘    └─────────────┘    └──────────────────────┘   │
//! │                                                │                     │
//! │                                                ▼                     │
//! │   ┌────────────────────────────────────────────────────────────┐   │
//! │   │                    Analysis Types                          │   │
//! │   │  • Conversation Pattern Analysis                           │   │
//! │   │  • User Preference Extraction                              │   │
//! │   │  • Knowledge Gap Detection                                 │   │
//! │   │  • Relationship Mapping                                    │   │
//! │   │  • Proactive Suggestion Generation                         │   │
//! │   └────────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────┘
//! ```

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn, debug};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::events::{AgentEvent, EventBus};
use crate::memory::manager::MemoryManager;
use crate::memory::core::{CoreMemoryBlock};
use crate::cognitive::llm::LlmClient;
use crate::types::Message;
use crate::error::Result;

/// Configuration for Background Thinker
#[derive(Debug, Clone)]
pub struct BackgroundThinkerConfig {
    /// Minimum idle time before deep thinking (default: 5 minutes)
    pub deep_thinking_threshold: Duration,
    /// Minimum idle time for light thinking (default: 2 minutes)
    pub light_thinking_threshold: Duration,
    /// Maximum conversations to analyze in one session
    pub max_conversations_to_analyze: usize,
    /// Enable pattern recognition
    pub enable_pattern_recognition: bool,
    /// Enable preference learning
    pub enable_preference_learning: bool,
    /// Enable proactive suggestions
    pub enable_proactive_suggestions: bool,
    /// Minimum confidence for insights to be stored
    pub min_insight_confidence: f32,
}

impl Default for BackgroundThinkerConfig {
    fn default() -> Self {
        Self {
            deep_thinking_threshold: Duration::from_secs(300), // 5 minutes
            light_thinking_threshold: Duration::from_secs(120), // 2 minutes
            max_conversations_to_analyze: 10,
            enable_pattern_recognition: true,
            enable_preference_learning: true,
            enable_proactive_suggestions: true,
            min_insight_confidence: 0.7,
        }
    }
}

/// Types of insights that can be generated
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InsightType {
    /// User preference discovered
    UserPreference,
    /// Pattern in user behavior
    BehaviorPattern,
    /// Knowledge gap identified
    KnowledgeGap,
    /// Relationship between concepts
    ConceptRelationship,
    /// Suggestion for user
    ProactiveSuggestion,
    /// Emotional state pattern
    EmotionalPattern,
}

/// A generated insight from background thinking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Insight {
    pub id: String,
    pub insight_type: InsightType,
    pub content: String,
    pub confidence: f32,
    pub source_sessions: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub applied: bool,
    pub metadata: serde_json::Value,
}

impl Insight {
    pub fn new(
        insight_type: InsightType,
        content: String,
        confidence: f32,
        source_sessions: Vec<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            insight_type,
            content,
            confidence,
            source_sessions,
            created_at: Utc::now(),
            expires_at: None,
            applied: false,
            metadata: serde_json::Value::Null,
        }
    }

    /// Check if insight is still valid (not expired)
    pub fn is_valid(&self) -> bool {
        match self.expires_at {
            Some(expiry) => Utc::now() < expiry,
            None => true,
        }
    }
}

/// Statistics for background thinking
#[derive(Debug, Clone, Default)]
pub struct BackgroundThinkerStats {
    /// Total thinking sessions
    pub total_sessions: u64,
    /// Deep thinking sessions
    pub deep_sessions: u64,
    /// Light thinking sessions
    pub light_sessions: u64,
    /// Total insights generated
    pub insights_generated: u64,
    /// Insights by type
    pub insights_by_type: std::collections::HashMap<String, u64>,
    /// Insights applied to core memory
    pub insights_applied: u64,
    /// Last thinking session timestamp
    pub last_session: Option<DateTime<Utc>>,
    /// Average session duration
    pub avg_session_duration_ms: u64,
}

/// Background Thinker - Advanced reflection system
pub struct BackgroundThinker {
    config: BackgroundThinkerConfig,
    event_bus: Arc<EventBus>,
    memory_manager: Arc<MemoryManager>,
    llm: Arc<Box<dyn LlmClient>>,
    /// Generated insights storage
    insights: Arc<RwLock<Vec<Insight>>>,
    /// Statistics
    stats: Arc<RwLock<BackgroundThinkerStats>>,
    /// Shutdown signal
    shutdown: Arc<RwLock<bool>>,
}

impl BackgroundThinker {
    pub fn new(
        config: BackgroundThinkerConfig,
        event_bus: Arc<EventBus>,
        memory_manager: Arc<MemoryManager>,
        llm: Arc<Box<dyn LlmClient>>,
    ) -> Self {
        Self {
            config,
            event_bus,
            memory_manager,
            llm,
            insights: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(BackgroundThinkerStats::default())),
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the background thinker loop
    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            info!(
                "Background Thinker started (deep_threshold: {:?}, light_threshold: {:?})",
                self.config.deep_thinking_threshold,
                self.config.light_thinking_threshold
            );

            let mut interval = tokio::time::interval(Duration::from_secs(60));

            loop {
                interval.tick().await;

                if *self.shutdown.read().await {
                    info!("Background Thinker shutting down");
                    break;
                }

                if let Err(e) = self.check_and_think().await {
                    warn!("Background thinking failed: {}", e);
                }
            }
        });
    }

    /// Stop the background thinker
    pub async fn stop(&self) {
        *self.shutdown.write().await = true;
    }

    /// Check idle state and trigger appropriate thinking level
    async fn check_and_think(&self) -> Result<()> {
        let idle_duration = self.memory_manager.idle_duration().await;

        // Determine thinking level based on idle time
        let thinking_level = if idle_duration >= self.config.deep_thinking_threshold {
            ThinkingLevel::Deep
        } else if idle_duration >= self.config.light_thinking_threshold {
            ThinkingLevel::Light
        } else {
            return Ok(()); // Not idle enough
        };

        let session_start = Utc::now();
        
        // Publish event
        self.event_bus.publish(AgentEvent::BackgroundThinkingTriggered {
            reason: format!("User idle for {:?}", idle_duration),
            context_summary: format!("{:?} thinking session", thinking_level),
        });

        // Execute thinking
        let insights = match thinking_level {
            ThinkingLevel::Deep => self.deep_think().await?,
            ThinkingLevel::Light => self.light_think().await?,
        };

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.total_sessions += 1;
            match thinking_level {
                ThinkingLevel::Deep => stats.deep_sessions += 1,
                ThinkingLevel::Light => stats.light_sessions += 1,
            }
            stats.insights_generated += insights.len() as u64;
            stats.last_session = Some(session_start);
            
            let duration = Utc::now().signed_duration_since(session_start);
            let duration_ms = duration.num_milliseconds().max(0) as u64;
            
            // Update average
            if stats.total_sessions == 1 {
                stats.avg_session_duration_ms = duration_ms;
            } else {
                stats.avg_session_duration_ms = 
                    (stats.avg_session_duration_ms * (stats.total_sessions - 1) + duration_ms) 
                    / stats.total_sessions;
            }
        }

        // Store insights
        {
            let mut stored_insights = self.insights.write().await;
            for insight in insights {
                // Update type count
                let type_name = format!("{:?}", insight.insight_type);
                let mut stats = self.stats.write().await;
                *stats.insights_by_type.entry(type_name).or_insert(0) += 1;
                drop(stats);
                
                stored_insights.push(insight);
            }
        }

        // Publish completion event
        self.event_bus.publish(AgentEvent::BackgroundThinkingResult {
            insights: format!("Generated {} insights", insights.len()),
            suggested_actions: vec![],
            memories_updated: vec![],
        });

        Ok(())
    }

    /// Light thinking - Quick pattern detection and preference updates
    async fn light_think(&self) -> Result<Vec<Insight>> {
        debug!("Starting light thinking session");
        let mut insights = Vec::new();

        // 1. Quick preference extraction from recent context
        if self.config.enable_preference_learning {
            let preference_insights = self.extract_recent_preferences().await?;
            insights.extend(preference_insights);
        }

        // 2. Simple pattern detection
        if self.config.enable_pattern_recognition {
            let pattern_insights = self.detect_simple_patterns().await?;
            insights.extend(pattern_insights);
        }

        Ok(insights)
    }

    /// Deep thinking - Comprehensive analysis and insight generation
    async fn deep_think(&self) -> Result<Vec<Insight>> {
        info!("Starting deep thinking session");
        let mut insights = Vec::new();

        // 1. Comprehensive conversation analysis
        let conversation_insights = self.analyze_conversations().await?;
        insights.extend(conversation_insights);

        // 2. Knowledge gap detection
        let gap_insights = self.detect_knowledge_gaps().await?;
        insights.extend(gap_insights);

        // 3. Generate proactive suggestions
        if self.config.enable_proactive_suggestions {
            let suggestion_insights = self.generate_proactive_suggestions().await?;
            insights.extend(suggestion_insights);
        }

        // 4. Apply high-confidence insights to core memory
        self.apply_insights_to_core_memory(&insights).await?;

        Ok(insights)
    }

    /// Extract user preferences from recent conversations
    async fn extract_recent_preferences(&self) -> Result<Vec<Insight>> {
        let prompt = r#"Analyze the recent conversation context and extract any user preferences.

Look for:
1. Communication style preferences (formal/casual, detailed/concise)
2. Technical preferences (programming languages, tools, frameworks)
3. Content preferences (topics of interest, depth of explanation)
4. Interaction preferences (frequency of updates, level of detail)

Respond with a JSON object:
{
  "preferences": [
    {
      "type": "communication|technical|content|interaction",
      "description": "Clear description of the preference",
      "confidence": 0.0-1.0,
      "evidence": "Brief evidence from conversation"
    }
  ]
}

If no clear preferences are found, return {"preferences": []}"#;

        let messages = vec![Message::system(prompt)];
        
        match self.llm.chat_complete(&messages).await {
            Ok(response) => {
                let parsed: serde_json::Value = serde_json::from_str(&response)
                    .unwrap_or_else(|_| serde_json::json!({"preferences": []}));
                
                let mut insights = Vec::new();
                if let Some(prefs) = parsed.get("preferences").and_then(|p| p.as_array()) {
                    for pref in prefs {
                        if let (Some(confidence), Some(description)) = (
                            pref.get("confidence").and_then(|c| c.as_f64()),
                            pref.get("description").and_then(|d| d.as_str())
                        ) {
                            if confidence >= self.config.min_insight_confidence as f64 {
                                insights.push(Insight::new(
                                    InsightType::UserPreference,
                                    description.to_string(),
                                    confidence as f32,
                                    vec!["recent_context".to_string()],
                                ));
                            }
                        }
                    }
                }
                Ok(insights)
            }
            Err(e) => {
                warn!("Failed to extract preferences: {}", e);
                Ok(vec![])
            }
        }
    }

    /// Detect simple patterns in user behavior
    async fn detect_simple_patterns(&self) -> Result<Vec<Insight>> {
        // This would analyze message timing, response patterns, etc.
        // For now, return empty - can be enhanced with actual pattern detection
        Ok(vec![])
    }

    /// Analyze conversations comprehensively
    async fn analyze_conversations(&self) -> Result<Vec<Insight>> {
        let prompt = r#"Perform a comprehensive analysis of recent conversations.

Analyze:
1. Recurring themes and topics
2. User's problem-solving approach
3. Communication patterns
4. Emotional trends
5. Knowledge evolution

Respond with a JSON object:
{
  "analysis": {
    "themes": ["theme1", "theme2"],
    "approach": "description of problem-solving style",
    "communication_style": "description",
    "emotional_trend": "trend description",
    "knowledge_growth": ["area1", "area2"]
  },
  "insights": [
    {
      "type": "BehaviorPattern|ConceptRelationship|EmotionalPattern",
      "content": "insight description",
      "confidence": 0.0-1.0
    }
  ]
}"#;

        let messages = vec![Message::system(prompt)];
        
        match self.llm.chat_complete(&messages).await {
            Ok(response) => {
                let parsed: serde_json::Value = serde_json::from_str(&response)
                    .unwrap_or_else(|_| serde_json::json!({"insights": []}));
                
                let mut insights = Vec::new();
                if let Some(insight_list) = parsed.get("insights").and_then(|i| i.as_array()) {
                    for insight_data in insight_list {
                        if let (Some(confidence), Some(content), Some(type_str)) = (
                            insight_data.get("confidence").and_then(|c| c.as_f64()),
                            insight_data.get("content").and_then(|d| d.as_str()),
                            insight_data.get("type").and_then(|t| t.as_str())
                        ) {
                            let insight_type = match type_str {
                                "BehaviorPattern" => InsightType::BehaviorPattern,
                                "ConceptRelationship" => InsightType::ConceptRelationship,
                                "EmotionalPattern" => InsightType::EmotionalPattern,
                                _ => InsightType::BehaviorPattern,
                            };
                            
                            if confidence >= self.config.min_insight_confidence as f64 {
                                insights.push(Insight::new(
                                    insight_type,
                                    content.to_string(),
                                    confidence as f32,
                                    vec!["conversation_analysis".to_string()],
                                ));
                            }
                        }
                    }
                }
                Ok(insights)
            }
            Err(e) => {
                warn!("Failed to analyze conversations: {}", e);
                Ok(vec![])
            }
        }
    }

    /// Detect knowledge gaps that could be filled
    async fn detect_knowledge_gaps(&self) -> Result<Vec<Insight>> {
        let prompt = r#"Analyze the conversations to identify potential knowledge gaps.

Look for:
1. Topics the user seems interested in but lacks depth
2. Questions that suggest missing foundational knowledge
3. Areas where explanations were particularly helpful
4. Concepts that were revisited multiple times

Respond with a JSON object:
{
  "knowledge_gaps": [
    {
      "topic": "topic name",
      "description": "description of the gap",
      "priority": "high|medium|low",
      "confidence": 0.0-1.0
    }
  ]
}"#;

        let messages = vec![Message::system(prompt)];
        
        match self.llm.chat_complete(&messages).await {
            Ok(response) => {
                let parsed: serde_json::Value = serde_json::from_str(&response)
                    .unwrap_or_else(|_| serde_json::json!({"knowledge_gaps": []}));
                
                let mut insights = Vec::new();
                if let Some(gaps) = parsed.get("knowledge_gaps").and_then(|g| g.as_array()) {
                    for gap in gaps {
                        if let (Some(confidence), Some(description)) = (
                            gap.get("confidence").and_then(|c| c.as_f64()),
                            gap.get("description").and_then(|d| d.as_str())
                        ) {
                            if confidence >= self.config.min_insight_confidence as f64 {
                                insights.push(Insight::new(
                                    InsightType::KnowledgeGap,
                                    description.to_string(),
                                    confidence as f32,
                                    vec!["knowledge_analysis".to_string()],
                                ));
                            }
                        }
                    }
                }
                Ok(insights)
            }
            Err(e) => {
                warn!("Failed to detect knowledge gaps: {}", e);
                Ok(vec![])
            }
        }
    }

    /// Generate proactive suggestions for the user
    async fn generate_proactive_suggestions(&self) -> Result<Vec<Insight>> {
        let prompt = r#"Based on the conversation history, generate proactive suggestions that might help the user.

Consider:
1. Related topics they might find interesting
2. Resources that could help with their current work
3. Better ways to approach their problems
4. Tools or techniques they might benefit from

Respond with a JSON object:
{
  "suggestions": [
    {
      "content": "suggestion description",
      "context": "why this suggestion is relevant",
      "confidence": 0.0-1.0,
      "priority": "high|medium|low"
    }
  ]
}"#;

        let messages = vec![Message::system(prompt)];
        
        match self.llm.chat_complete(&messages).await {
            Ok(response) => {
                let parsed: serde_json::Value = serde_json::from_str(&response)
                    .unwrap_or_else(|_| serde_json::json!({"suggestions": []}));
                
                let mut insights = Vec::new();
                if let Some(suggestions) = parsed.get("suggestions").and_then(|s| s.as_array()) {
                    for suggestion in suggestions {
                        if let (Some(confidence), Some(content)) = (
                            suggestion.get("confidence").and_then(|c| c.as_f64()),
                            suggestion.get("content").and_then(|d| d.as_str())
                        ) {
                            if confidence >= self.config.min_insight_confidence as f64 {
                                insights.push(Insight::new(
                                    InsightType::ProactiveSuggestion,
                                    content.to_string(),
                                    confidence as f32,
                                    vec!["proactive_analysis".to_string()],
                                ));
                            }
                        }
                    }
                }
                Ok(insights)
            }
            Err(e) => {
                warn!("Failed to generate suggestions: {}", e);
                Ok(vec![])
            }
        }
    }

    /// Apply high-confidence insights to core memory
    async fn apply_insights_to_core_memory(&self, insights: &[Insight]) -> Result<()> {
        let mut applied_count = 0;

        for insight in insights {
            // Only apply high-confidence user preferences
            if insight.insight_type == InsightType::UserPreference && insight.confidence >= 0.8 {
                match self.memory_manager.core_memory_append(
                    CoreMemoryBlock::Human,
                    &format!("[Learned Preference] {}", insight.content)
                ).await {
                    Ok(_) => {
                        applied_count += 1;
                        
                        // Publish event
                        self.event_bus.publish(AgentEvent::CoreMemoryUpdated {
                            block: "human".to_string(),
                            operation: "append".to_string(),
                            timestamp: Utc::now(),
                        });
                    }
                    Err(e) => {
                        warn!("Failed to apply insight to core memory: {}", e);
                    }
                }
            }
        }

        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.insights_applied += applied_count;
        }

        Ok(())
    }

    /// Get all stored insights
    pub async fn get_insights(&self) -> Vec<Insight> {
        self.insights.read().await.clone()
    }

    /// Get insights by type
    pub async fn get_insights_by_type(&self, insight_type: InsightType) -> Vec<Insight> {
        self.insights.read().await
            .iter()
            .filter(|i| i.insight_type == insight_type && i.is_valid())
            .cloned()
            .collect()
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> BackgroundThinkerStats {
        self.stats.read().await.clone()
    }

    /// Clear all insights
    pub async fn clear_insights(&self) {
        self.insights.write().await.clear();
    }
}

/// Thinking level based on idle duration
#[derive(Debug, Clone, Copy, PartialEq)]
enum ThinkingLevel {
    Light,
    Deep,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insight_creation() {
        let insight = Insight::new(
            InsightType::UserPreference,
            "User prefers Python".to_string(),
            0.9,
            vec!["session1".to_string()],
        );

        assert_eq!(insight.insight_type, InsightType::UserPreference);
        assert_eq!(insight.content, "User prefers Python");
        assert_eq!(insight.confidence, 0.9);
        assert!(insight.is_valid());
    }

    #[test]
    fn test_insight_expiration() {
        let mut insight = Insight::new(
            InsightType::UserPreference,
            "Test".to_string(),
            0.9,
            vec![],
        );
        
        // Set expiration in the past
        insight.expires_at = Some(Utc::now() - chrono::Duration::hours(1));
        
        assert!(!insight.is_valid());
    }

    #[test]
    fn test_background_thinker_config_default() {
        let config = BackgroundThinkerConfig::default();
        assert_eq!(config.deep_thinking_threshold, Duration::from_secs(300));
        assert_eq!(config.light_thinking_threshold, Duration::from_secs(120));
        assert!(config.enable_pattern_recognition);
        assert!(config.enable_preference_learning);
        assert!(config.enable_proactive_suggestions);
    }
}
