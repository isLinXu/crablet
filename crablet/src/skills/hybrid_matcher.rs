//! Hybrid Skill Matcher with RRF (Reciprocal Rank Fusion)
//! 
//! Combines multiple matching strategies using RRF to provide optimal
//! recall and precision for skill matching.

use std::collections::HashMap;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, debug};

use super::semantic_matcher::{SemanticMatcher, SemanticMatch};

/// Hybrid match result combining multiple signals
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridMatch {
    pub skill_name: String,
    pub final_score: f32,
    pub semantic_score: f32,
    pub keyword_score: f32,
    pub usage_score: f32,
    pub context_score: f32,
    pub rank_positions: Vec<usize>,
    pub matched_keywords: Vec<String>,
    pub confidence_tier: ConfidenceTier,
}

/// Confidence tiers for matches
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConfidenceTier {
    High,      // > 0.8 - Auto-execute
    Medium,    // 0.6 - 0.8 - Suggest with confirmation
    Low,       // 0.4 - 0.6 - Show as option
    VeryLow,   // < 0.4 - Not shown unless explicitly requested
}

impl ConfidenceTier {
    pub fn from_score(score: f32) -> Self {
        match score {
            s if s > 0.8 => ConfidenceTier::High,
            s if s > 0.6 => ConfidenceTier::Medium,
            s if s > 0.4 => ConfidenceTier::Low,
            _ => ConfidenceTier::VeryLow,
        }
    }

    pub fn should_auto_execute(&self) -> bool {
        matches!(self, ConfidenceTier::High)
    }

    pub fn should_suggest(&self) -> bool {
        matches!(self, ConfidenceTier::High | ConfidenceTier::Medium | ConfidenceTier::Low)
    }
}

/// Hybrid matcher using RRF fusion
pub struct HybridMatcher {
    semantic_matcher: SemanticMatcher,
    /// Historical usage statistics
    usage_stats: HashMap<String, UsageStats>,
    /// Context-based scoring weights
    context_weights: ContextWeights,
    /// RRF constant k (typically 60)
    rrf_k: f32,
    /// Minimum score threshold
    threshold: f32,
}

#[derive(Debug, Clone, Default)]
struct UsageStats {
    invocation_count: u32,
    success_count: u32,
    last_used: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
struct ContextWeights {
    conversation_topic: f32,
    recent_skills: f32,
    user_preferences: f32,
}

impl Default for ContextWeights {
    fn default() -> Self {
        Self {
            conversation_topic: 0.3,
            recent_skills: 0.4,
            user_preferences: 0.3,
        }
    }
}

impl HybridMatcher {
    /// Create a new hybrid matcher
    pub fn new() -> Self {
        Self {
            semantic_matcher: SemanticMatcher::new(),
            usage_stats: HashMap::new(),
            context_weights: ContextWeights::default(),
            rrf_k: 60.0,
            threshold: 0.5,
        }
    }

    /// Set RRF constant
    pub fn with_rrf_k(mut self, k: f32) -> Self {
        self.rrf_k = k.max(1.0);
        self
    }

    /// Set threshold
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Initialize semantic matcher with embedder
    #[cfg(feature = "knowledge")]
    pub async fn init_semantic(&mut self, embedder: crate::knowledge::embedder::Embedder) -> Result<()> {
        self.semantic_matcher = self.semantic_matcher.clone().with_embedder(embedder).await?;
        Ok(())
    }

    /// Register a skill
    pub async fn register_skill(
        &mut self,
        name: &str,
        description: &str,
        keywords: Vec<String>,
        examples: Vec<String>,
    ) -> Result<()> {
        self.semantic_matcher.register_skill(name, description, keywords, examples).await?;
        
        // Initialize usage stats
        self.usage_stats.entry(name.to_string()).or_default();
        
        Ok(())
    }

    /// Find matches using hybrid approach
    pub async fn find_matches(
        &self,
        query: &str,
        context: &ConversationContext,
        top_k: usize,
    ) -> Result<Vec<HybridMatch>> {
        // 1. Get semantic matches
        let semantic_matches = self.semantic_matcher.find_matches(query, top_k * 2).await?;
        
        // 2. Get keyword matches
        let keyword_matches = self.keyword_match(query);
        
        // 3. Get usage-based matches
        let usage_matches = self.usage_based_match(query);
        
        // 4. Get context-based matches
        let context_matches = self.context_based_match(query, context);

        // 5. Fuse using RRF
        let fused_results = self.reciprocal_rank_fusion(
            &semantic_matches,
            &keyword_matches,
            &usage_matches,
            &context_matches,
        );

        // 6. Build final results
        let mut results: Vec<HybridMatch> = fused_results
            .into_iter()
            .filter(|(_, score)| *score >= self.threshold)
            .map(|(skill_name, score)| {
                self.build_hybrid_match(&skill_name, score, &semantic_matches, &keyword_matches)
            })
            .collect();

        // Sort by final score
        results.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap());
        results.truncate(top_k);

        debug!("Hybrid matcher found {} matches for query: {}", results.len(), query);
        
        Ok(results)
    }

    /// Keyword matching
    fn keyword_match(&self, _query: &str) -> Vec<(String, f32)> {
        // Delegate to semantic matcher's keyword matching
        // This is a simplified version - in production, use the semantic matcher's method
        vec![]
    }

    /// Usage-based matching (popularity/recency)
    fn usage_based_match(&self, _query: &str) -> Vec<(String, f32)> {
        let mut matches: Vec<(String, f32)> = self.usage_stats
            .iter()
            .map(|(skill_name, stats)| {
                let score = self.calculate_usage_score(stats);
                (skill_name.clone(), score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        matches
    }

    /// Calculate usage score based on frequency and recency
    fn calculate_usage_score(&self, stats: &UsageStats) -> f32 {
        if stats.invocation_count == 0 {
            return 0.0;
        }

        // Frequency score (logarithmic to prevent dominance)
        let freq_score = (stats.invocation_count as f32).ln_1p() / 5.0;

        // Success rate
        let success_rate = if stats.invocation_count > 0 {
            stats.success_count as f32 / stats.invocation_count as f32
        } else {
            0.0
        };

        // Recency score
        let recency_score = if let Some(last_used) = stats.last_used {
            let hours_ago = (chrono::Utc::now() - last_used).num_hours() as f32;
            (-hours_ago / 168.0).exp() // Decay over a week
        } else {
            0.0
        };

        (freq_score * 0.4 + success_rate * 0.4 + recency_score * 0.2).min(1.0)
    }

    /// Context-based matching
    fn context_based_match(&self, query: &str, context: &ConversationContext) -> Vec<(String, f32)> {
        let mut scores: HashMap<String, f32> = HashMap::new();

        // Topic similarity
        if let Some(ref topic) = context.current_topic {
            for (skill_name, _) in &self.usage_stats {
                let similarity = self.calculate_topic_similarity(query, topic, skill_name);
                if similarity > 0.0 {
                    *scores.entry(skill_name.clone()).or_insert(0.0) += similarity * self.context_weights.conversation_topic;
                }
            }
        }

        // Recent skills boost
        for (i, recent_skill) in context.recent_skills.iter().enumerate() {
            let recency_weight = 1.0 - (i as f32 / context.recent_skills.len() as f32);
            *scores.entry(recent_skill.clone()).or_insert(0.0) += recency_weight * self.context_weights.recent_skills;
        }

        // User preferences
        for (skill_name, preference) in &context.user_preferences {
            *scores.entry(skill_name.clone()).or_insert(0.0) += preference * self.context_weights.user_preferences;
        }

        let mut matches: Vec<(String, f32)> = scores.into_iter().collect();
        matches.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        matches
    }

    /// Calculate topic similarity
    fn calculate_topic_similarity(&self, _query: &str, _topic: &str, _skill_name: &str) -> f32 {
        // Simplified implementation - would use embeddings in production
        0.5
    }

    /// Reciprocal Rank Fusion
    fn reciprocal_rank_fusion(
        &self,
        semantic: &[SemanticMatch],
        keyword: &[(String, f32)],
        usage: &[(String, f32)],
        context: &[(String, f32)],
    ) -> Vec<(String, f32)> {
        let mut rrf_scores: HashMap<String, f32> = HashMap::new();

        // Helper to add RRF scores
        let mut add_rrf = |matches: &[(String, f32)], weight: f32| {
            for (rank, (skill_name, _)) in matches.iter().enumerate() {
                let rrf_score = weight * (1.0 / (self.rrf_k + rank as f32));
                *rrf_scores.entry(skill_name.clone()).or_insert(0.0) += rrf_score;
            }
        };

        // Add scores from each source
        let semantic_tuples: Vec<(String, f32)> = semantic.iter()
            .map(|m| (m.skill_name.clone(), m.confidence))
            .collect();
        add_rrf(&semantic_tuples, 1.0);
        add_rrf(keyword, 0.8);
        add_rrf(usage, 0.6);
        add_rrf(context, 0.7);

        // Convert to sorted vector
        let mut results: Vec<(String, f32)> = rrf_scores.into_iter().collect();
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results
    }

    /// Build hybrid match from component scores
    fn build_hybrid_match(
        &self,
        skill_name: &str,
        final_score: f32,
        semantic: &[SemanticMatch],
        keyword: &[(String, f32)],
    ) -> HybridMatch {
        let _semantic_match = semantic.iter().find(|m| m.skill_name == skill_name);
        let _keyword_match = keyword.iter().find(|(name, _)| name == skill_name);
        let usage_stat = self.usage_stats.get(skill_name);

        HybridMatch {
            skill_name: skill_name.to_string(),
            final_score,
            semantic_score: _semantic_match.map(|m| m.semantic_score).unwrap_or(0.0),
            keyword_score: _semantic_match.map(|m| m.keyword_score).unwrap_or(0.0),
            usage_score: usage_stat.map(|s| self.calculate_usage_score(s)).unwrap_or(0.0),
            context_score: 0.0, // Calculated separately
            rank_positions: vec![], // Tracked during RRF
            matched_keywords: _semantic_match.map(|m| m.matched_keywords.clone()).unwrap_or_default(),
            confidence_tier: ConfidenceTier::from_score(final_score),
        }
    }

    /// Record skill usage for learning
    pub fn record_usage(&mut self, skill_name: &str, success: bool) {
        let stats = self.usage_stats.entry(skill_name.to_string()).or_default();
        stats.invocation_count += 1;
        if success {
            stats.success_count += 1;
        }
        stats.last_used = Some(chrono::Utc::now());
    }

    /// Get skill suggestions for ambiguous queries
    pub fn suggest_skills(&self, partial_query: &str, limit: usize) -> Vec<String> {
        self.semantic_matcher.suggest_skills(partial_query, limit)
    }

    /// Adapt parameters based on performance
    pub fn adapt_parameters(&mut self, precision: f32, recall: f32) {
        // Adjust RRF weights based on precision/recall trade-off
        if recall < 0.7 {
            // Lower threshold to increase recall
            self.threshold = (self.threshold * 0.9).max(0.3);
            info!("Adapted threshold down to {:.2} to improve recall", self.threshold);
        } else if precision < 0.7 {
            // Raise threshold to increase precision
            self.threshold = (self.threshold * 1.1).min(0.8);
            info!("Adapted threshold up to {:.2} to improve precision", self.threshold);
        }
    }
}

impl Default for HybridMatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Conversation context for context-aware matching
#[derive(Debug, Clone)]
pub struct ConversationContext {
    pub current_topic: Option<String>,
    pub recent_skills: Vec<String>,
    pub user_preferences: HashMap<String, f32>,
    pub conversation_history: Vec<String>,
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self {
            current_topic: None,
            recent_skills: Vec::new(),
            user_preferences: HashMap::new(),
            conversation_history: Vec::new(),
        }
    }
}

impl ConversationContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_topic(mut self, topic: String) -> Self {
        self.current_topic = Some(topic);
        self
    }

    pub fn with_recent_skills(mut self, skills: Vec<String>) -> Self {
        self.recent_skills = skills;
        self
    }

    pub fn add_to_history(&mut self, message: String) {
        self.conversation_history.push(message);
        if self.conversation_history.len() > 10 {
            self.conversation_history.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_tier() {
        assert_eq!(ConfidenceTier::from_score(0.9), ConfidenceTier::High);
        assert_eq!(ConfidenceTier::from_score(0.7), ConfidenceTier::Medium);
        assert_eq!(ConfidenceTier::from_score(0.5), ConfidenceTier::Low);
        assert_eq!(ConfidenceTier::from_score(0.3), ConfidenceTier::VeryLow);
    }

    #[test]
    fn test_usage_score() {
        let matcher = HybridMatcher::new();
        let stats = UsageStats {
            invocation_count: 10,
            success_count: 8,
            last_used: Some(chrono::Utc::now()),
        };
        
        let score = matcher.calculate_usage_score(&stats);
        assert!(score > 0.0 && score <= 1.0);
    }

    #[test]
    fn test_rrf_fusion() {
        let matcher = HybridMatcher::new();
        
        let semantic = vec![
            SemanticMatch {
                skill_name: "weather".to_string(),
                confidence: 0.9,
                matched_keywords: vec![],
                semantic_score: 0.9,
                keyword_score: 0.5,
            }
        ];
        
        let keyword = vec![("weather".to_string(), 0.8)];
        let usage = vec![];
        let context = vec![];
        
        let results = matcher.reciprocal_rank_fusion(&semantic, &keyword, &usage, &context);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "weather");
    }
}
