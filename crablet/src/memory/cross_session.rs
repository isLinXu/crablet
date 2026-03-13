//! Cross-Session Fusion - Unified user profile across sessions
//!
//! This module integrates memories across multiple sessions to build
//! a comprehensive and unified user profile:
//! - Session correlation analysis
//! - Unified preference extraction
//! - Long-term behavior pattern recognition
//! - Cross-session knowledge transfer
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                 Cross-Session Fusion                                │
//! │                                                                      │
//! │   Sessions ──→  Correlate  ──→  Extract Patterns  ──→  Profile     │
//! │                        │                                           │
//! │                        ▼                                           │
//! │   ┌────────────────────────────────────────────────────────────┐   │
//! │   │                    Fusion Operations                       │   │
//! │   │  • Identity Resolution (link sessions to user)             │   │
//! │   │  • Preference Aggregation (merge preferences)              │   │
//! │   │  • Knowledge Synthesis (unify learnings)                   │   │
//! │   │  • Behavior Profiling (long-term patterns)                 │   │
//! │   │  • Context Transfer (carry context across)                 │   │
//! │   └────────────────────────────────────────────────────────────┘   │
//! └─────────────────────────────────────────────────────────────────────┘

use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn, debug};
use chrono::{DateTime, Utc, Duration as ChronoDuration};
use serde::{Deserialize, Serialize};

use crate::events::{AgentEvent, EventBus};
use crate::memory::manager::MemoryManager;
use crate::memory::core::{CoreMemory, CoreMemoryBlock};
use crate::knowledge::vector_store::VectorStore;
use crate::cognitive::llm::LlmClient;
use crate::types::Message;
use crate::error::Result;

/// Configuration for Cross-Session Fusion
#[derive(Debug, Clone)]
pub struct CrossSessionConfig {
    /// How often to run fusion (default: 6 hours)
    pub fusion_interval: Duration,
    /// Minimum sessions before fusion
    pub min_sessions_for_fusion: usize,
    /// Maximum sessions to analyze per cycle
    pub max_sessions_per_cycle: usize,
    /// Enable identity resolution
    pub enable_identity_resolution: bool,
    /// Enable preference aggregation
    pub enable_preference_aggregation: bool,
    /// Enable knowledge synthesis
    pub enable_knowledge_synthesis: bool,
    /// Enable behavior profiling
    pub enable_behavior_profiling: bool,
    /// Similarity threshold for session correlation
    pub session_similarity_threshold: f32,
}

impl Default for CrossSessionConfig {
    fn default() -> Self {
        Self {
            fusion_interval: Duration::from_secs(21600), // 6 hours
            min_sessions_for_fusion: 3,
            max_sessions_per_cycle: 50,
            enable_identity_resolution: true,
            enable_preference_aggregation: true,
            enable_knowledge_synthesis: true,
            enable_behavior_profiling: true,
            session_similarity_threshold: 0.7,
        }
    }
}

/// Unified user profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedUserProfile {
    pub user_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub identity_signals: Vec<IdentitySignal>,
    pub aggregated_preferences: Vec<AggregatedPreference>,
    pub behavior_profile: BehaviorProfile,
    pub knowledge_summary: KnowledgeSummary,
    pub session_count: usize,
    pub total_interactions: u64,
}

/// Signal for identity resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentitySignal {
    pub signal_type: IdentitySignalType,
    pub value: String,
    pub confidence: f32,
    pub source_sessions: Vec<String>,
    pub first_seen: DateTime<Utc>,
}

/// Type of identity signal
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum IdentitySignalType {
    ExplicitName,
    EmailPattern,
    WritingStyle,
    TopicPreference,
    ToolPreference,
    TemporalPattern,
}

/// Aggregated preference across sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedPreference {
    pub preference_type: String,
    pub value: String,
    pub confidence: f32,
    pub supporting_evidence: Vec<String>,
    pub session_sources: Vec<String>,
    pub first_observed: DateTime<Utc>,
    pub last_observed: DateTime<Utc>,
    pub stability_score: f32, // How consistent across sessions
}

/// Long-term behavior profile
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BehaviorProfile {
    pub dominant_topics: Vec<TopicAffinity>,
    pub interaction_patterns: Vec<InteractionPattern>,
    pub expertise_areas: Vec<ExpertiseArea>,
    pub communication_style: CommunicationStyle,
    pub learning_preferences: LearningPreferences,
}

/// Topic affinity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicAffinity {
    pub topic: String,
    pub affinity_score: f32,
    pub frequency: u32,
    pub depth: f32, // How deep do they go into this topic
}

/// Interaction pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionPattern {
    pub pattern_type: String,
    pub frequency: u32,
    pub typical_duration: Duration,
    pub preferred_times: Vec<u8>, // Hours of day (0-23)
}

/// Expertise area
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpertiseArea {
    pub domain: String,
    pub level: ExpertiseLevel,
    pub evidence: Vec<String>,
}

/// Expertise level
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExpertiseLevel {
    Beginner,
    Intermediate,
    Advanced,
    Expert,
}

/// Communication style
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommunicationStyle {
    pub formality: f32, // 0.0 = casual, 1.0 = formal
    pub detail_preference: f32, // 0.0 = brief, 1.0 = detailed
    pub technical_depth: f32, // 0.0 = high-level, 1.0 = technical
    pub example_preference: bool,
    pub question_frequency: f32,
}

/// Learning preferences
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LearningPreferences {
    pub prefers_examples: bool,
    pub prefers_theory: bool,
    pub step_by_step: bool,
    pub visual_learner: bool,
    pub hands_on: bool,
}

/// Knowledge summary
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KnowledgeSummary {
    pub key_learnings: Vec<String>,
    pub recurring_questions: Vec<String>,
    pub knowledge_gaps: Vec<String>,
    pub mastered_concepts: Vec<String>,
}

/// Session metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub session_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub message_count: usize,
    pub topics: Vec<String>,
    pub tools_used: Vec<String>,
}

/// Statistics for Cross-Session Fusion
#[derive(Debug, Clone, Default)]
pub struct CrossSessionStats {
    pub total_fusion_runs: u64,
    pub sessions_analyzed: u64,
    pub profiles_created: u64,
    pub profiles_updated: u64,
    pub preferences_discovered: u64,
    pub last_fusion: Option<DateTime<Utc>>,
    pub avg_fusion_duration_ms: u64,
}

/// Cross-Session Fusion system
pub struct CrossSessionFusion {
    config: CrossSessionConfig,
    event_bus: Arc<EventBus>,
    memory_manager: Arc<MemoryManager>,
    vector_store: Option<Arc<VectorStore>>,
    llm: Arc<Box<dyn LlmClient>>,
    /// User profiles
    profiles: Arc<RwLock<HashMap<String, UnifiedUserProfile>>>,
    /// Session index
    sessions: Arc<RwLock<Vec<SessionMetadata>>>,
    /// Statistics
    stats: Arc<RwLock<CrossSessionStats>>,
    /// Shutdown signal
    shutdown: Arc<RwLock<bool>>,
}

impl CrossSessionFusion {
    pub fn new(
        config: CrossSessionConfig,
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
            profiles: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(CrossSessionStats::default())),
            shutdown: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the fusion loop
    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            info!(
                "Cross-Session Fusion started (interval: {:?})",
                self.config.fusion_interval
            );

            let mut interval = tokio::time::interval(self.config.fusion_interval);

            loop {
                interval.tick().await;

                if *self.shutdown.read().await {
                    info!("Cross-Session Fusion shutting down");
                    break;
                }

                if let Err(e) = self.run_fusion().await {
                    warn!("Cross-session fusion failed: {}", e);
                }
            }
        });
    }

    /// Stop the fusion system
    pub async fn stop(&self) {
        *self.shutdown.write().await = true;
    }

    /// Register a new session
    pub async fn register_session(&self, session_id: String, topics: Vec<String>) {
        let session = SessionMetadata {
            session_id,
            start_time: Utc::now(),
            end_time: None,
            message_count: 0,
            topics,
            tools_used: Vec::new(),
        };

        self.sessions.write().await.push(session);
    }

    /// Update session metadata
    pub async fn update_session(&self, session_id: &str, message_count: usize, tools: Vec<String>) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.iter_mut().find(|s| s.session_id == session_id) {
            session.message_count = message_count;
            session.tools_used = tools;
        }
    }

    /// Close a session
    pub async fn close_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.iter_mut().find(|s| s.session_id == session_id) {
            session.end_time = Some(Utc::now());
        }
    }

    /// Run a fusion cycle
    pub async fn run_fusion(&self) -> Result<()> {
        let start_time = std::time::Instant::now();
        info!("Starting cross-session fusion");

        let sessions = self.sessions.read().await;
        
        if sessions.len() < self.config.min_sessions_for_fusion {
            debug!("Not enough sessions for fusion ({} < {})", 
                   sessions.len(), self.config.min_sessions_for_fusion);
            return Ok(());
        }

        let sessions_to_analyze: Vec<_> = sessions
            .iter()
            .take(self.config.max_sessions_per_cycle)
            .cloned()
            .collect();
        drop(sessions);

        // 1. Identity Resolution
        let identity_groups = if self.config.enable_identity_resolution {
            self.resolve_identities(&sessions_to_analyze).await?
        } else {
            vec![sessions_to_analyze]
        };

        // 2. For each identity group, build/update profile
        for group in identity_groups {
            self.fuse_sessions(&group).await?;
        }

        // Update stats
        let duration_ms = start_time.elapsed().as_millis() as u64;
        {
            let mut stats = self.stats.write().await;
            stats.total_fusion_runs += 1;
            stats.sessions_analyzed += sessions_to_analyze.len() as u64;
            stats.last_fusion = Some(Utc::now());

            if stats.total_fusion_runs == 1 {
                stats.avg_fusion_duration_ms = duration_ms;
            } else {
                stats.avg_fusion_duration_ms =
                    (stats.avg_fusion_duration_ms * (stats.total_fusion_runs - 1) + duration_ms)
                    / stats.total_fusion_runs;
            }
        }

        info!("Cross-session fusion completed in {}ms", duration_ms);

        Ok(())
    }

    /// Resolve identities across sessions
    async fn resolve_identities(&self, sessions: &[SessionMetadata]) -> Result<Vec<Vec<SessionMetadata>>> {
        // Group sessions by similarity
        let mut groups: Vec<Vec<SessionMetadata>> = Vec::new();
        let mut assigned: HashSet<String> = HashSet::new();

        for session in sessions {
            if assigned.contains(&session.session_id) {
                continue;
            }

            let mut group = vec![session.clone()];
            assigned.insert(session.session_id.clone());

            // Find similar sessions
            for other in sessions {
                if assigned.contains(&other.session_id) {
                    continue;
                }

                let similarity = self.calculate_session_similarity(session, other).await?;
                
                if similarity >= self.config.session_similarity_threshold {
                    group.push(other.clone());
                    assigned.insert(other.session_id.clone());
                }
            }

            if !group.is_empty() {
                groups.push(group);
            }
        }

        Ok(groups)
    }

    /// Calculate similarity between two sessions
    async fn calculate_session_similarity(&self, session_a: &SessionMetadata, session_b: &SessionMetadata) -> Result<f32> {
        // Topic overlap
        let topic_overlap: HashSet<_> = session_a.topics.iter().collect();
        let common_topics: HashSet<_> = session_b.topics.iter()
            .filter(|t| topic_overlap.contains(t))
            .collect();
        
        let topic_similarity = if session_a.topics.is_empty() || session_b.topics.is_empty() {
            0.0
        } else {
            (common_topics.len() as f32 * 2.0) / 
                (session_a.topics.len() + session_b.topics.len()) as f32
        };

        // Tool overlap
        let tool_overlap: HashSet<_> = session_a.tools_used.iter().collect();
        let common_tools: HashSet<_> = session_b.tools_used.iter()
            .filter(|t| tool_overlap.contains(t))
            .collect();
        
        let tool_similarity = if session_a.tools_used.is_empty() || session_b.tools_used.is_empty() {
            0.0
        } else {
            (common_tools.len() as f32 * 2.0) / 
                (session_a.tools_used.len() + session_b.tools_used.len()) as f32
        };

        // Temporal proximity (sessions close in time are more likely same user)
        let temporal_similarity = if let (Some(end_a), Some(start_b)) = (session_a.end_time, Some(session_b.start_time)) {
            let gap = (start_b - end_a).num_hours().abs() as f32;
            (1.0 / (1.0 + gap / 24.0)).min(1.0) // Decay over 24 hours
        } else {
            0.5 // Unknown
        };

        // Weighted combination
        let similarity = topic_similarity * 0.5 + tool_similarity * 0.3 + temporal_similarity * 0.2;
        
        Ok(similarity)
    }

    /// Fuse sessions into a unified profile
    async fn fuse_sessions(&self, sessions: &[SessionMetadata]) -> Result<()> {
        let user_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        // Build or update profile
        let mut profiles = self.profiles.write().await;
        
        let profile = profiles.entry(user_id.clone()).or_insert_with(|| {
            UnifiedUserProfile {
                user_id: user_id.clone(),
                created_at: now,
                updated_at: now,
                identity_signals: Vec::new(),
                aggregated_preferences: Vec::new(),
                behavior_profile: BehaviorProfile::default(),
                knowledge_summary: KnowledgeSummary::default(),
                session_count: 0,
                total_interactions: 0,
            }
        });

        // Update profile
        profile.updated_at = now;
        profile.session_count = sessions.len();

        // Aggregate preferences
        if self.config.enable_preference_aggregation {
            self.aggregate_preferences(profile, sessions).await?;
        }

        // Build behavior profile
        if self.config.enable_behavior_profiling {
            self.build_behavior_profile(profile, sessions).await?;
        }

        // Synthesize knowledge
        if self.config.enable_knowledge_synthesis {
            self.synthesize_knowledge(profile, sessions).await?;
        }

        // Update stats
        if profile.session_count == sessions.len() {
            self.stats.write().await.profiles_created += 1;
        } else {
            self.stats.write().await.profiles_updated += 1;
        }

        // Publish event
        self.event_bus.publish(AgentEvent::SystemLog(format!(
            "Unified profile updated for user {}: {} sessions, {} preferences",
            user_id, sessions.len(), profile.aggregated_preferences.len()
        )));

        Ok(())
    }

    /// Aggregate preferences across sessions
    async fn aggregate_preferences(&self, profile: &mut UnifiedUserProfile, sessions: &[SessionMetadata]) -> Result<()> {
        // Use LLM to extract and aggregate preferences
        let session_summary = sessions.iter()
            .map(|s| format!("Session {}: topics={:?}, tools={:?}", 
                s.session_id, s.topics, s.tools_used))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "Analyze the following session data and extract user preferences:\n\n{}\n\n\
            Respond with JSON:\n\
            {{\n\
              \"preferences\": [\n\
                {{\n\
                  \"type\": \"preference_category\",\n\
                  \"value\": \"preference_value\",\n\
                  \"confidence\": 0.0-1.0\n\
                }}\n\
              ]\n\
            }}",
            session_summary
        );

        match self.llm.chat_complete(&[Message::system(&prompt)]).await {
            Ok(response) => {
                // Parse preferences
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response) {
                    if let Some(prefs) = json.get("preferences").and_then(|p| p.as_array()) {
                        for pref in prefs {
                            if let (Some(pref_type), Some(value), Some(confidence)) = (
                                pref.get("type").and_then(|t| t.as_str()),
                                pref.get("value").and_then(|v| v.as_str()),
                                pref.get("confidence").and_then(|c| c.as_f64())
                            ) {
                                let aggregated = AggregatedPreference {
                                    preference_type: pref_type.to_string(),
                                    value: value.to_string(),
                                    confidence: confidence as f32,
                                    supporting_evidence: vec!["cross_session_analysis".to_string()],
                                    session_sources: sessions.iter().map(|s| s.session_id.clone()).collect(),
                                    first_observed: sessions.iter().map(|s| s.start_time).min().unwrap_or(Utc::now()),
                                    last_observed: sessions.iter().filter_map(|s| s.end_time).max().unwrap_or(Utc::now()),
                                    stability_score: 0.8,
                                };

                                profile.aggregated_preferences.push(aggregated);
                                self.stats.write().await.preferences_discovered += 1;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to aggregate preferences: {}", e);
            }
        }

        Ok(())
    }

    /// Build behavior profile from sessions
    async fn build_behavior_profile(&self, profile: &mut UnifiedUserProfile, sessions: &[SessionMetadata]) -> Result<()> {
        // Analyze dominant topics
        let mut topic_counts: HashMap<String, u32> = HashMap::new();
        for session in sessions {
            for topic in &session.topics {
                *topic_counts.entry(topic.clone()).or_insert(0) += 1;
            }
        }

        let total_sessions = sessions.len() as f32;
        profile.behavior_profile.dominant_topics = topic_counts
            .into_iter()
            .map(|(topic, count)| TopicAffinity {
                topic,
                affinity_score: count as f32 / total_sessions,
                frequency: count,
                depth: 0.5, // Placeholder
            })
            .collect();

        // Analyze interaction patterns
        let mut hour_counts: HashMap<u8, u32> = HashMap::new();
        for session in sessions {
            let hour = session.start_time.hour() as u8;
            *hour_counts.entry(hour).or_insert(0) += 1;
        }

        let preferred_hours: Vec<u8> = hour_counts
            .into_iter()
            .filter(|(_, count)| *count >= 2)
            .map(|(hour, _)| hour)
            .collect();

        profile.behavior_profile.interaction_patterns.push(InteractionPattern {
            pattern_type: "time_preference".to_string(),
            frequency: sessions.len() as u32,
            typical_duration: Duration::from_secs(1800), // 30 min placeholder
            preferred_times: preferred_hours,
        });

        Ok(())
    }

    /// Synthesize knowledge from sessions
    async fn synthesize_knowledge(&self, profile: &mut UnifiedUserProfile, sessions: &[SessionMetadata]) -> Result<()> {
        // Identify recurring themes and knowledge areas
        let all_topics: HashSet<_> = sessions.iter()
            .flat_map(|s| s.topics.iter())
            .cloned()
            .collect();

        profile.knowledge_summary.mastered_concepts = all_topics.into_iter().collect();

        // Identify knowledge gaps (topics mentioned but not deeply explored)
        // This would require deeper analysis of conversation content

        Ok(())
    }

    /// Get unified profile for a user
    pub async fn get_profile(&self, user_id: &str) -> Option<UnifiedUserProfile> {
        self.profiles.read().await.get(user_id).cloned()
    }

    /// Get all profiles
    pub async fn get_all_profiles(&self) -> Vec<UnifiedUserProfile> {
        self.profiles.read().await.values().cloned().collect()
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> CrossSessionStats {
        self.stats.read().await.clone()
    }

    /// Force fusion run
    pub async fn force_fusion(&self) -> Result<()> {
        self.run_fusion().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_user_profile_creation() {
        let profile = UnifiedUserProfile {
            user_id: "user1".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            identity_signals: vec![],
            aggregated_preferences: vec![],
            behavior_profile: BehaviorProfile::default(),
            knowledge_summary: KnowledgeSummary::default(),
            session_count: 5,
            total_interactions: 100,
        };

        assert_eq!(profile.session_count, 5);
        assert_eq!(profile.total_interactions, 100);
    }

    #[test]
    fn test_aggregated_preference_creation() {
        let pref = AggregatedPreference {
            preference_type: "communication".to_string(),
            value: "concise".to_string(),
            confidence: 0.9,
            supporting_evidence: vec!["evidence1".to_string()],
            session_sources: vec!["session1".to_string(), "session2".to_string()],
            first_observed: Utc::now(),
            last_observed: Utc::now(),
            stability_score: 0.85,
        };

        assert_eq!(pref.preference_type, "communication");
        assert_eq!(pref.stability_score, 0.85);
    }

    #[test]
    fn test_session_metadata_creation() {
        let session = SessionMetadata {
            session_id: "sess1".to_string(),
            start_time: Utc::now(),
            end_time: None,
            message_count: 50,
            topics: vec!["rust".to_string(), "programming".to_string()],
            tools_used: vec!["cargo".to_string()],
        };

        assert_eq!(session.message_count, 50);
        assert_eq!(session.topics.len(), 2);
    }

    #[test]
    fn test_cross_session_config_default() {
        let config = CrossSessionConfig::default();
        assert_eq!(config.fusion_interval, Duration::from_secs(21600));
        assert_eq!(config.min_sessions_for_fusion, 3);
        assert!(config.enable_identity_resolution);
        assert!(config.enable_preference_aggregation);
    }
}
