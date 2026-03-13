//! Adaptive Routing System
//!
//! Dynamically adjusts routing decisions based on historical performance,
//! user feedback, and real-time context analysis.

use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{info, debug};

use super::intent_classifier::{Intent, IntentClassifier, ClassificationResult};
use crate::skills::hybrid_matcher::{HybridMatcher, HybridMatch, ConversationContext};

/// Routing decision with confidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub target_system: TargetSystem,
    pub confidence: f32,
    pub reasoning: String,
    pub estimated_latency_ms: u32,
    pub fallback_system: Option<TargetSystem>,
    pub requires_confirmation: bool,
}

/// Target processing systems
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TargetSystem {
    System1,           // Fast, simple responses
    System2,           // Balanced reasoning
    System3,           // Deep research
    SkillExecution(String), // Execute specific skill
    Clarification,     // Ask user for clarification
    MultiStep,         // Break into multiple steps
}

/// Performance metrics for adaptive learning
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemPerformance {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub average_latency_ms: f32,
    pub user_satisfaction: f32, // 0-5 rating
    pub error_rate: f32,
    pub last_updated: Option<chrono::DateTime<chrono::Utc>>,
}

/// Adaptive router with learning capabilities
pub struct AdaptiveRouter {
    intent_classifier: IntentClassifier,
    hybrid_matcher: Arc<RwLock<HybridMatcher>>,
    /// Performance metrics per system
    performance_metrics: Arc<RwLock<HashMap<TargetSystem, SystemPerformance>>>,
    /// Routing thresholds (adaptable)
    thresholds: Arc<RwLock<RoutingThresholds>>,
    /// Historical routing decisions
    decision_history: Arc<RwLock<Vec<RoutingHistoryEntry>>>,
}

#[derive(Debug, Clone)]
struct RoutingThresholds {
    system1_confidence: f32,
    system2_confidence: f32,
    system3_confidence: f32,
    skill_auto_execute: f32,
    clarification_threshold: f32,
}

impl Default for RoutingThresholds {
    fn default() -> Self {
        Self {
            system1_confidence: 0.9,
            system2_confidence: 0.7,
            system3_confidence: 0.6,
            skill_auto_execute: 0.8,
            clarification_threshold: 0.4,
        }
    }
}

#[derive(Debug, Clone)]
struct RoutingHistoryEntry {
    timestamp: chrono::DateTime<chrono::Utc>,
    query: String,
    decision: RoutingDecision,
    user_feedback: Option<UserFeedback>,
    actual_latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFeedback {
    pub rating: u8, // 1-5
    pub comments: Option<String>,
    pub would_reroute: bool,
    pub preferred_system: Option<TargetSystem>,
}

impl AdaptiveRouter {
    /// Create a new adaptive router
    pub fn new() -> Self {
        let mut metrics = HashMap::new();
        metrics.insert(TargetSystem::System1, SystemPerformance::default());
        metrics.insert(TargetSystem::System2, SystemPerformance::default());
        metrics.insert(TargetSystem::System3, SystemPerformance::default());
        metrics.insert(TargetSystem::Clarification, SystemPerformance::default());
        metrics.insert(TargetSystem::MultiStep, SystemPerformance::default());

        Self {
            intent_classifier: IntentClassifier::new(),
            hybrid_matcher: Arc::new(RwLock::new(HybridMatcher::new())),
            performance_metrics: Arc::new(RwLock::new(metrics)),
            thresholds: Arc::new(RwLock::new(RoutingThresholds::default())),
            decision_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Route a query to the appropriate system
    pub async fn route(
        &self,
        query: &str,
        context: &ConversationContext,
    ) -> Result<RoutingDecision> {
        // 1. Classify intent
        let intent_result = self.intent_classifier.classify(query);
        debug!("Intent classification: {:?} (confidence: {:.2})", 
            intent_result.intent, intent_result.confidence);

        // 2. Check for skill invocation
        let skill_matches = self.find_skill_matches(query, context).await?;

        // 3. Make routing decision
        let decision = self.make_routing_decision(query, &intent_result, &skill_matches).await;

        // 4. Record decision
        self.record_decision(query, decision.clone()).await;

        Ok(decision)
    }

    /// Find matching skills
    async fn find_skill_matches(
        &self,
        query: &str,
        context: &ConversationContext,
    ) -> Result<Vec<HybridMatch>> {
        let matcher = self.hybrid_matcher.read().await;
        matcher.find_matches(query, context, 5).await
    }

    /// Make routing decision based on all signals
    async fn make_routing_decision(
        &self,
        query: &str,
        intent_result: &ClassificationResult,
        skill_matches: &[HybridMatch],
    ) -> RoutingDecision {
        let thresholds = self.thresholds.read().await;

        // Check if we have a high-confidence skill match
        if let Some(top_skill) = skill_matches.first() {
            if top_skill.confidence_tier.should_auto_execute() && 
               top_skill.final_score >= thresholds.skill_auto_execute {
                return RoutingDecision {
                    target_system: TargetSystem::SkillExecution(top_skill.skill_name.clone()),
                    confidence: top_skill.final_score,
                    reasoning: format!("High-confidence skill match: {} (score: {:.2})", 
                        top_skill.skill_name, top_skill.final_score),
                    estimated_latency_ms: 500, // Typical skill execution time
                    fallback_system: Some(TargetSystem::System2),
                    requires_confirmation: false,
                };
            }
        }

        // Check for explicit skill invocation
        if let Some(skill_name) = self.intent_classifier.is_skill_invocation(query) {
            return RoutingDecision {
                target_system: TargetSystem::SkillExecution(skill_name),
                confidence: 0.95,
                reasoning: "Explicit skill invocation detected".to_string(),
                estimated_latency_ms: 500,
                fallback_system: Some(TargetSystem::System2),
                requires_confirmation: false,
            };
        }

        // Route based on intent
        match &intent_result.intent {
            Intent::Greeting | Intent::Help | Intent::Status => {
                RoutingDecision {
                    target_system: TargetSystem::System1,
                    confidence: intent_result.confidence,
                    reasoning: format!("Simple intent: {:?}", intent_result.intent),
                    estimated_latency_ms: 200,
                    fallback_system: None,
                    requires_confirmation: false,
                }
            }

            Intent::DeepResearch => {
                RoutingDecision {
                    target_system: TargetSystem::System3,
                    confidence: intent_result.confidence,
                    reasoning: "Deep research intent detected".to_string(),
                    estimated_latency_ms: 10000,
                    fallback_system: Some(TargetSystem::System2),
                    requires_confirmation: intent_result.confidence < thresholds.system3_confidence,
                }
            }

            Intent::MultiStep => {
                RoutingDecision {
                    target_system: TargetSystem::MultiStep,
                    confidence: intent_result.confidence,
                    reasoning: "Multi-step task detected".to_string(),
                    estimated_latency_ms: 5000,
                    fallback_system: Some(TargetSystem::System2),
                    requires_confirmation: false,
                }
            }

            Intent::Coding | Intent::Analysis => {
                let (target, latency) = if intent_result.confidence >= thresholds.system2_confidence {
                    (TargetSystem::System2, 3000)
                } else {
                    (TargetSystem::System1, 500)
                };
                
                RoutingDecision {
                    target_system: target,
                    confidence: intent_result.confidence,
                    reasoning: format!("{:?} task with confidence {:.2}", 
                        intent_result.intent, intent_result.confidence),
                    estimated_latency_ms: latency,
                    fallback_system: Some(TargetSystem::System1),
                    requires_confirmation: false,
                }
            }

            Intent::Creative | Intent::Math => {
                RoutingDecision {
                    target_system: TargetSystem::System2,
                    confidence: intent_result.confidence,
                    reasoning: format!("{:?} task", intent_result.intent),
                    estimated_latency_ms: 2000,
                    fallback_system: Some(TargetSystem::System1),
                    requires_confirmation: false,
                }
            }

            Intent::SkillExecution(skill_name) => {
                RoutingDecision {
                    target_system: TargetSystem::SkillExecution(skill_name.clone()),
                    confidence: 0.9,
                    reasoning: format!("Explicit skill execution: {}", skill_name),
                    estimated_latency_ms: 500,
                    fallback_system: Some(TargetSystem::System2),
                    requires_confirmation: false,
                }
            }

            Intent::General | Intent::Unknown => {
                // Check if we need clarification
                if intent_result.requires_clarification || 
                   intent_result.confidence < thresholds.clarification_threshold {
                    RoutingDecision {
                        target_system: TargetSystem::Clarification,
                        confidence: 1.0 - intent_result.confidence,
                        reasoning: "Low confidence, needs clarification".to_string(),
                        estimated_latency_ms: 100,
                        fallback_system: Some(TargetSystem::System2),
                        requires_confirmation: false,
                    }
                } else {
                    // Default to System2 for general queries
                    RoutingDecision {
                        target_system: TargetSystem::System2,
                        confidence: intent_result.confidence,
                        reasoning: "General query, using balanced system".to_string(),
                        estimated_latency_ms: 2000,
                        fallback_system: Some(TargetSystem::System1),
                        requires_confirmation: false,
                    }
                }
            }
        }
    }

    /// Record routing decision for learning
    async fn record_decision(&self, query: &str, decision: RoutingDecision) {
        let entry = RoutingHistoryEntry {
            timestamp: chrono::Utc::now(),
            query: query.to_string(),
            decision,
            user_feedback: None,
            actual_latency_ms: None,
        };

        let mut history = self.decision_history.write().await;
        history.push(entry);
        
        // Keep only last 1000 decisions
        if history.len() > 1000 {
            history.remove(0);
        }
    }

    /// Update with user feedback
    pub async fn record_feedback(&self, query: &str, feedback: UserFeedback) {
        let mut history = self.decision_history.write().await;
        
        // Find the most recent matching entry
        if let Some(entry) = history.iter_mut().rev().find(|e| e.query == query) {
            entry.user_feedback = Some(feedback.clone());
            
            // Update performance metrics
            let mut metrics = self.performance_metrics.write().await;
            if let Some(perf) = metrics.get_mut(&entry.decision.target_system) {
                perf.total_requests += 1;
                if feedback.rating >= 3 {
                    perf.successful_requests += 1;
                }
                perf.user_satisfaction = (perf.user_satisfaction * (perf.total_requests - 1) as f32 
                    + feedback.rating as f32) / perf.total_requests as f32;
                perf.last_updated = Some(chrono::Utc::now());
            }
        }

        // Adapt thresholds based on feedback
        self.adapt_thresholds().await;
    }

    /// Record actual latency for a decision
    pub async fn record_latency(&self, query: &str, latency_ms: u64) {
        let mut history = self.decision_history.write().await;
        
        if let Some(entry) = history.iter_mut().rev().find(|e| e.query == query) {
            entry.actual_latency_ms = Some(latency_ms);
            
            // Update average latency
            let mut metrics = self.performance_metrics.write().await;
            if let Some(perf) = metrics.get_mut(&entry.decision.target_system) {
                perf.average_latency_ms = (perf.average_latency_ms * (perf.total_requests - 1) as f32 
                    + latency_ms as f32) / perf.total_requests as f32;
            }
        }
    }

    /// Adapt thresholds based on performance
    async fn adapt_thresholds(&self) {
        let metrics = self.performance_metrics.read().await;
        let mut thresholds = self.thresholds.write().await;

        for (system, perf) in metrics.iter() {
            if perf.total_requests < 10 {
                continue; // Not enough data
            }

            let success_rate = perf.successful_requests as f32 / perf.total_requests as f32;
            let target_rate = 0.85;

            match system {
                TargetSystem::System1 => {
                    if success_rate < target_rate {
                        thresholds.system1_confidence = (thresholds.system1_confidence * 1.05).min(0.95);
                    } else {
                        thresholds.system1_confidence = (thresholds.system1_confidence * 0.98).max(0.8);
                    }
                }
                TargetSystem::System2 => {
                    if success_rate < target_rate {
                        thresholds.system2_confidence = (thresholds.system2_confidence * 1.05).min(0.9);
                    } else {
                        thresholds.system2_confidence = (thresholds.system2_confidence * 0.98).max(0.6);
                    }
                }
                TargetSystem::SkillExecution(_) => {
                    if success_rate < target_rate {
                        thresholds.skill_auto_execute = (thresholds.skill_auto_execute * 1.05).min(0.9);
                    } else {
                        thresholds.skill_auto_execute = (thresholds.skill_auto_execute * 0.98).max(0.7);
                    }
                }
                _ => {}
            }
        }

        info!("Adapted routing thresholds: {:?}", *thresholds);
    }

    /// Get current performance statistics
    pub async fn get_performance_stats(&self) -> HashMap<String, SystemPerformance> {
        let metrics = self.performance_metrics.read().await;
        metrics
            .iter()
            .map(|(k, v)| (format!("{:?}", k), v.clone()))
            .collect()
    }

    /// Get routing statistics
    pub async fn get_routing_stats(&self) -> RoutingStats {
        let history = self.decision_history.read().await;
        let total = history.len();
        
        if total == 0 {
            return RoutingStats::default();
        }

        let with_feedback = history.iter().filter(|e| e.user_feedback.is_some()).count();
        let avg_confidence = history.iter().map(|e| e.decision.confidence).sum::<f32>() / total as f32;

        RoutingStats {
            total_decisions: total as u64,
            decisions_with_feedback: with_feedback as u64,
            average_confidence: avg_confidence,
        }
    }
}

impl Default for AdaptiveRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Routing statistics
#[derive(Debug, Clone, Default)]
pub struct RoutingStats {
    pub total_decisions: u64,
    pub decisions_with_feedback: u64,
    pub average_confidence: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_routing_decision() {
        let router = AdaptiveRouter::new();
        let context = ConversationContext::new();
        
        // Test greeting routing
        let decision = router.route("Hello!", &context).await.unwrap();
        assert!(matches!(decision.target_system, TargetSystem::System1));
        
        // Test deep research routing
        let decision = router.route("Research quantum computing in depth", &context).await.unwrap();
        assert!(matches!(decision.target_system, TargetSystem::System3));
    }

    #[tokio::test]
    async fn test_explicit_skill_invocation() {
        let router = AdaptiveRouter::new();
        let context = ConversationContext::new();
        
        let decision = router.route("use weather", &context).await.unwrap();
        assert!(matches!(decision.target_system, TargetSystem::SkillExecution(_)));
    }

    #[tokio::test]
    async fn test_feedback_recording() {
        let router = AdaptiveRouter::new();
        let context = ConversationContext::new();
        
        // Make a decision
        let decision = router.route("test query", &context).await.unwrap();
        
        // Record feedback
        let feedback = UserFeedback {
            rating: 5,
            comments: None,
            would_reroute: false,
            preferred_system: None,
        };
        
        router.record_feedback("test query", feedback).await;
        
        let stats = router.get_performance_stats().await;
        assert!(!stats.is_empty());
    }
}
