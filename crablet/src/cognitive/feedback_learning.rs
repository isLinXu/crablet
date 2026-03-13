//! Feedback Learning System
//!
//! Collects and learns from user feedback to continuously improve
//! routing decisions, skill matching, and answer quality.

use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::info;
use chrono::{DateTime, Utc, Duration};

/// Types of feedback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedbackType {
    /// Thumbs up/down on answer
    AnswerRating { rating: i8, reason: Option<String> },
    /// Skill execution feedback
    SkillFeedback { skill_name: String, helpful: bool, correction: Option<String> },
    /// Routing feedback
    RoutingFeedback { would_reroute: bool, preferred_route: Option<String> },
    /// Explicit correction
    ExplicitCorrection { original: String, corrected: String },
    /// Implicit signal (e.g., follow-up question indicates previous answer incomplete)
    ImplicitSignal { signal_type: String, strength: f32 },
}

/// Feedback entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEntry {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub query: String,
    pub response: String,
    pub feedback_type: FeedbackType,
    pub context: FeedbackContext,
    pub processed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackContext {
    pub session_id: String,
    pub user_id: Option<String>,
    pub intent: Option<String>,
    pub skills_used: Vec<String>,
    pub routing_decision: Option<String>,
    pub latency_ms: u64,
}

/// Learning model for continuous improvement
pub struct FeedbackLearningSystem {
    /// Feedback storage
    feedback_store: Arc<RwLock<Vec<FeedbackEntry>>>,
    /// Skill performance metrics
    skill_metrics: Arc<RwLock<HashMap<String, SkillMetrics>>>,
    /// Pattern learning
    learned_patterns: Arc<RwLock<LearnedPatterns>>,
    /// User preferences
    user_preferences: Arc<RwLock<HashMap<String, UserPreferenceProfile>>>,
    /// Configuration
    config: LearningConfig,
}

#[derive(Debug, Clone)]
struct SkillMetrics {
    total_invocations: u64,
    successful_invocations: u64,
    average_rating: f32,
    common_failures: Vec<String>,
    last_improved: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default)]
struct LearnedPatterns {
    /// Query patterns that lead to specific skills
    query_to_skill_patterns: HashMap<String, Vec<String>>,
    /// Intent patterns that need clarification
    ambiguous_intents: Vec<String>,
    /// Successful routing patterns
    successful_routes: Vec<(String, String)>, // (query_pattern, route)
}

#[derive(Debug, Clone, Default)]
struct UserPreferenceProfile {
    preferred_skills: Vec<String>,
    preferred_response_style: ResponseStyle,
    common_topics: Vec<String>,
    feedback_history: Vec<FeedbackEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseStyle {
    Concise,
    Detailed,
    Technical,
    Casual,
}

impl Default for ResponseStyle {
    fn default() -> Self {
        ResponseStyle::Detailed
    }
}

#[derive(Debug, Clone)]
struct LearningConfig {
    min_feedback_for_learning: usize,
    pattern_expiry_days: i64,
    learning_rate: f32,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            min_feedback_for_learning: 5,
            pattern_expiry_days: 30,
            learning_rate: 0.1,
        }
    }
}

impl FeedbackLearningSystem {
    /// Create a new learning system
    pub fn new() -> Self {
        Self {
            feedback_store: Arc::new(RwLock::new(Vec::new())),
            skill_metrics: Arc::new(RwLock::new(HashMap::new())),
            learned_patterns: Arc::new(RwLock::new(LearnedPatterns::default())),
            user_preferences: Arc::new(RwLock::new(HashMap::new())),
            config: LearningConfig::default(),
        }
    }

    /// Submit feedback
    pub async fn submit_feedback(&self, entry: FeedbackEntry) -> Result<()> {
        let mut store = self.feedback_store.write().await;
        store.push(entry.clone());
        
        // Update user preferences
        if let Some(ref user_id) = entry.context.user_id {
            let mut prefs = self.user_preferences.write().await;
            let profile = prefs.entry(user_id.clone()).or_default();
            profile.feedback_history.push(entry.clone());
        }

        // Process feedback immediately if we have enough
        if store.len() >= self.config.min_feedback_for_learning {
            drop(store); // Release lock before processing
            self.process_feedback_batch().await?;
        }

        info!("Feedback recorded: {} (type: {:?})", entry.id, entry.feedback_type);
        Ok(())
    }

    /// Process accumulated feedback
    async fn process_feedback_batch(&self) -> Result<()> {
        let store = self.feedback_store.read().await;
        let unprocessed: Vec<FeedbackEntry> = store
            .iter()
            .filter(|e| !e.processed)
            .cloned()
            .collect();
        drop(store);

        if unprocessed.is_empty() {
            return Ok(());
        }

        info!("Processing {} feedback entries", unprocessed.len());

        // Update skill metrics
        self.update_skill_metrics(&unprocessed).await;

        // Learn patterns
        self.learn_patterns(&unprocessed).await;

        // Update user preferences
        self.update_user_preferences(&unprocessed).await;

        // Mark as processed
        let mut store = self.feedback_store.write().await;
        for entry in &unprocessed {
            if let Some(e) = store.iter_mut().find(|e| e.id == entry.id) {
                e.processed = true;
            }
        }

        info!("Feedback processing complete");
        Ok(())
    }

    /// Update skill performance metrics
    async fn update_skill_metrics(&self, feedback_entries: &[FeedbackEntry]) {
        let mut metrics = self.skill_metrics.write().await;

        for entry in feedback_entries {
            match &entry.feedback_type {
                FeedbackType::SkillFeedback { skill_name, helpful, correction } => {
                    let skill_metric = metrics.entry(skill_name.clone()).or_insert(SkillMetrics {
                        total_invocations: 0,
                        successful_invocations: 0,
                        average_rating: 3.0,
                        common_failures: Vec::new(),
                        last_improved: None,
                    });

                    skill_metric.total_invocations += 1;
                    if *helpful {
                        skill_metric.successful_invocations += 1;
                        skill_metric.average_rating = 
                            (skill_metric.average_rating * (skill_metric.total_invocations - 1) as f32 + 5.0)
                            / skill_metric.total_invocations as f32;
                    } else {
                        skill_metric.average_rating = 
                            (skill_metric.average_rating * (skill_metric.total_invocations - 1) as f32 + 1.0)
                            / skill_metric.total_invocations as f32;
                        
                        if let Some(ref corr) = correction {
                            skill_metric.common_failures.push(corr.clone());
                            // Keep only last 10 failures
                            if skill_metric.common_failures.len() > 10 {
                                skill_metric.common_failures.remove(0);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Learn patterns from feedback
    async fn learn_patterns(&self, feedback_entries: &[FeedbackEntry]) {
        let mut patterns = self.learned_patterns.write().await;

        for entry in feedback_entries {
            match &entry.feedback_type {
                FeedbackType::AnswerRating { rating, .. } if *rating > 0 => {
                    // Learn successful query patterns
                    let query_pattern = self.extract_query_pattern(&entry.query);
                    if let Some(ref skills) = entry.context.skills_used.first() {
                            patterns.query_to_skill_patterns
                            .entry(query_pattern)
                            .or_default()
                            .push(skills.to_string());
                    }
                }
                FeedbackType::ExplicitCorrection { original, corrected } => {
                    // Learn from corrections
                    patterns.successful_routes.push((original.clone(), corrected.clone()));
                }
                FeedbackType::RoutingFeedback { would_reroute: true, preferred_route: Some(route) } => {
                    // Learn better routing
                    patterns.successful_routes.push((entry.query.clone(), route.clone()));
                }
                _ => {}
            }
        }

        // Clean up old patterns
        self.cleanup_old_patterns(&mut patterns).await;
    }

    /// Update user preferences
    async fn update_user_preferences(&self, feedback_entries: &[FeedbackEntry]) {
        let mut prefs = self.user_preferences.write().await;

        for entry in feedback_entries {
            if let Some(ref user_id) = entry.context.user_id {
                let profile = prefs.entry(user_id.clone()).or_default();

                match &entry.feedback_type {
                    FeedbackType::AnswerRating { rating, .. } => {
                        // Infer response style preference
                        if entry.response.len() < 100 && *rating > 0 {
                            profile.preferred_response_style = ResponseStyle::Concise;
                        } else if entry.response.len() > 500 && *rating > 0 {
                            profile.preferred_response_style = ResponseStyle::Detailed;
                        }
                    }
                    FeedbackType::SkillFeedback { skill_name, helpful, .. } => {
                        if *helpful && !profile.preferred_skills.contains(skill_name) {
                            profile.preferred_skills.push(skill_name.clone());
                        }
                    }
                    _ => {}
                }

                // Extract common topics
                let topic = self.extract_topic(&entry.query);
                if !profile.common_topics.contains(&topic) {
                    profile.common_topics.push(topic);
                }
            }
        }
    }

    /// Extract query pattern for learning
    fn extract_query_pattern(&self, query: &str) -> String {
        // Simplified pattern extraction
        let query_lower = query.to_lowercase();
        let words: Vec<&str> = query_lower
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .take(5)
            .collect();
        words.join(" ")
    }

    /// Extract topic from query
    fn extract_topic(&self, query: &str) -> String {
        // Simplified topic extraction
        let keywords = ["code", "weather", "data", "analysis", "research", "help"];
        let query_lower = query.to_lowercase();
        
        for keyword in &keywords {
            if query_lower.contains(keyword) {
                return keyword.to_string();
            }
        }
        
        "general".to_string()
    }

    /// Clean up old patterns
    async fn cleanup_old_patterns(&self, patterns: &mut LearnedPatterns) {
        let expiry = Utc::now() - Duration::days(self.config.pattern_expiry_days);
        
        // In a real implementation, patterns would have timestamps
        // For now, we just limit the size
        if patterns.successful_routes.len() > 1000 {
            patterns.successful_routes.drain(0..patterns.successful_routes.len() - 1000);
        }
    }

    /// Get skill recommendations based on learned patterns
    pub async fn get_skill_recommendations(&self, query: &str) -> Vec<(String, f32)> {
        let patterns = self.learned_patterns.read().await;
        let query_pattern = self.extract_query_pattern(query);
        
        let mut recommendations: HashMap<String, f32> = HashMap::new();
        
        // Check learned patterns
        if let Some(skills) = patterns.query_to_skill_patterns.get(&query_pattern) {
            for skill in skills {
                *recommendations.entry(skill.clone()).or_insert(0.0) += 1.0;
            }
        }
        
        // Normalize scores
        let max_score = recommendations.values().cloned().fold(0.0, f32::max);
        let mut result: Vec<(String, f32)> = recommendations
            .into_iter()
            .map(|(skill, score)| (skill, if max_score > 0.0 { score / max_score } else { 0.0 }))
            .collect();
        
        result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        result
    }

    /// Get user-specific recommendations
    pub async fn get_user_recommendations(&self, user_id: &str, query: &str) -> UserRecommendations {
        let prefs = self.user_preferences.read().await;
        let metrics = self.skill_metrics.read().await;
        
        let profile = prefs.get(user_id);
        
        UserRecommendations {
            preferred_skills: profile.map(|p| p.preferred_skills.clone()).unwrap_or_default(),
            response_style: profile.map(|p| p.preferred_response_style.clone()).unwrap_or_default(),
            suggested_skills: self.get_skill_recommendations(query).await,
            high_performing_skills: metrics
                .iter()
                .filter(|(_, m)| m.average_rating > 4.0)
                .map(|(name, _)| name.clone())
                .collect(),
        }
    }

    /// Get skill performance report
    pub async fn get_skill_performance_report(&self) -> Vec<SkillPerformanceReport> {
        let metrics = self.skill_metrics.read().await;
        
        metrics
            .iter()
            .map(|(name, m)| SkillPerformanceReport {
                skill_name: name.clone(),
                total_invocations: m.total_invocations,
                success_rate: if m.total_invocations > 0 {
                    m.successful_invocations as f32 / m.total_invocations as f32
                } else {
                    0.0
                },
                average_rating: m.average_rating,
                common_issues: m.common_failures.clone(),
            })
            .collect()
    }

    /// Export learning data
    pub async fn export_learning_data(&self) -> Result<LearningDataExport> {
        let store = self.feedback_store.read().await;
        let patterns = self.learned_patterns.read().await;
        let prefs = self.user_preferences.read().await;
        
        Ok(LearningDataExport {
            total_feedback_entries: store.len(),
            learned_patterns: patterns.query_to_skill_patterns.clone(),
            user_profiles: prefs.len(),
            export_timestamp: Utc::now(),
        })
    }

    /// Import learning data
    pub async fn import_learning_data(&self, data: LearningDataExport) -> Result<()> {
        let mut patterns = self.learned_patterns.write().await;
        patterns.query_to_skill_patterns = data.learned_patterns;
        
        info!("Imported learning data with {} patterns", data.user_profiles);
        Ok(())
    }
}

impl Default for FeedbackLearningSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// User-specific recommendations
#[derive(Debug, Clone)]
pub struct UserRecommendations {
    pub preferred_skills: Vec<String>,
    pub response_style: ResponseStyle,
    pub suggested_skills: Vec<(String, f32)>,
    pub high_performing_skills: Vec<String>,
}

/// Skill performance report
#[derive(Debug, Clone, Serialize)]
pub struct SkillPerformanceReport {
    pub skill_name: String,
    pub total_invocations: u64,
    pub success_rate: f32,
    pub average_rating: f32,
    pub common_issues: Vec<String>,
}

/// Learning data export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningDataExport {
    pub total_feedback_entries: usize,
    pub learned_patterns: HashMap<String, Vec<String>>,
    pub user_profiles: usize,
    pub export_timestamp: DateTime<Utc>,
}

/// Feedback collection helper
pub struct FeedbackCollector {
    system: Arc<FeedbackLearningSystem>,
}

impl FeedbackCollector {
    pub fn new(system: Arc<FeedbackLearningSystem>) -> Self {
        Self { system }
    }

    /// Create a simple rating feedback
    pub async fn submit_rating(
        &self,
        query: &str,
        response: &str,
        rating: i8,
        reason: Option<String>,
        context: FeedbackContext,
    ) -> Result<()> {
        let entry = FeedbackEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            query: query.to_string(),
            response: response.to_string(),
            feedback_type: FeedbackType::AnswerRating { rating, reason },
            context,
            processed: false,
        };
        
        self.system.submit_feedback(entry).await
    }

    /// Create skill feedback
    pub async fn submit_skill_feedback(
        &self,
        query: &str,
        response: &str,
        skill_name: &str,
        helpful: bool,
        correction: Option<String>,
        context: FeedbackContext,
    ) -> Result<()> {
        let entry = FeedbackEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            query: query.to_string(),
            response: response.to_string(),
            feedback_type: FeedbackType::SkillFeedback {
                skill_name: skill_name.to_string(),
                helpful,
                correction,
            },
            context,
            processed: false,
        };
        
        self.system.submit_feedback(entry).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_feedback_submission() {
        let system = FeedbackLearningSystem::new();
        
        let entry = FeedbackEntry {
            id: "test-1".to_string(),
            timestamp: Utc::now(),
            query: "What is the weather?".to_string(),
            response: "It's sunny today".to_string(),
            feedback_type: FeedbackType::AnswerRating { rating: 5, reason: None },
            context: FeedbackContext {
                session_id: "session-1".to_string(),
                user_id: Some("user-1".to_string()),
                intent: Some("weather_query".to_string()),
                skills_used: vec!["weather".to_string()],
                routing_decision: Some("System1".to_string()),
                latency_ms: 200,
            },
            processed: false,
        };
        
        system.submit_feedback(entry).await.unwrap();
        
        let store = system.feedback_store.read().await;
        assert_eq!(store.len(), 1);
    }

    #[tokio::test]
    async fn test_skill_metrics_update() {
        let system = FeedbackLearningSystem::new();
        
        let entries = vec![
            FeedbackEntry {
                id: "test-1".to_string(),
                timestamp: Utc::now(),
                query: "test".to_string(),
                response: "test".to_string(),
                feedback_type: FeedbackType::SkillFeedback {
                    skill_name: "weather".to_string(),
                    helpful: true,
                    correction: None,
                },
                context: FeedbackContext {
                    session_id: "s1".to_string(),
                    user_id: None,
                    intent: None,
                    skills_used: vec![],
                    routing_decision: None,
                    latency_ms: 0,
                },
                processed: false,
            },
        ];
        
        system.update_skill_metrics(&entries).await;
        
        let metrics = system.skill_metrics.read().await;
        assert!(metrics.contains_key("weather"));
    }

    #[tokio::test]
    async fn test_skill_recommendations() {
        let system = FeedbackLearningSystem::new();
        
        // Add some patterns
        {
            let mut patterns = system.learned_patterns.write().await;
            patterns.query_to_skill_patterns.insert(
                "what weather".to_string(),
                vec!["weather".to_string()],
            );
        }
        
        let recommendations = system.get_skill_recommendations("what is the weather today").await;
        assert!(!recommendations.is_empty());
    }
}
