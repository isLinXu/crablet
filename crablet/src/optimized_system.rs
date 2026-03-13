//! Optimized System Integration
//!
//! This module demonstrates how to integrate all the optimization components
//! into the existing Crablet system.

use std::sync::Arc;
use anyhow::Result;
use tokio::sync::RwLock;

use crate::cognitive::{
    adaptive_router::{AdaptiveRouter, RoutingDecision, TargetSystem},
    feedback_learning::{FeedbackLearningSystem, FeedbackCollector},
    answer_validator::{AnswerValidator, SourceDocument},
    intent_classifier::IntentClassifier,
};

use crate::skills::{
    hybrid_matcher::{HybridMatcher, ConversationContext},
    registry::SkillRegistry,
};

use crate::evaluation::{
    ab_testing::{ABTestManager, TestPresets},
};

/// Optimized Crablet system with all improvements
pub struct OptimizedCrablet {
    /// Intent classification
    intent_classifier: IntentClassifier,
    /// Hybrid skill matcher
    hybrid_matcher: Arc<RwLock<HybridMatcher>>,
    /// Adaptive router
    adaptive_router: AdaptiveRouter,
    /// Feedback learning system
    feedback_system: Arc<FeedbackLearningSystem>,
    /// Answer validator
    answer_validator: AnswerValidator,
    /// A/B test manager
    ab_test_manager: ABTestManager,
    /// Skill registry
    skill_registry: SkillRegistry,
}

impl OptimizedCrablet {
    /// Create a new optimized system
    pub fn new() -> Self {
        let feedback_system = Arc::new(FeedbackLearningSystem::new());

        Self {
            intent_classifier: IntentClassifier::new(),
            hybrid_matcher: Arc::new(RwLock::new(HybridMatcher::new())),
            adaptive_router: AdaptiveRouter::new(),
            feedback_system: feedback_system.clone(),
            answer_validator: AnswerValidator::new(),
            ab_test_manager: ABTestManager::new(),
            skill_registry: SkillRegistry::new(),
        }
    }

    /// Initialize the optimized system
    pub async fn initialize(&mut self) -> Result<()> {
        // Initialize semantic matcher with embedder
        // This would typically load a pre-trained embedding model
        // For now, we skip this step as it requires the embedder setup

        // Register built-in skills with semantic information
        self.register_builtin_skills().await?;

        // Set up default A/B tests if none are running
        self.setup_default_tests().await?;

        info!("Optimized Crablet system initialized successfully");
        Ok(())
    }

    /// Register built-in skills with rich metadata
    async fn register_builtin_skills(&mut self) -> Result<()> {
        let mut matcher = self.hybrid_matcher.write().await;

        // Register weather skill
        matcher.register_skill(
            "weather",
            "Get current weather information for a location",
            vec![
                "weather".to_string(),
                "temperature".to_string(),
                "forecast".to_string(),
                "rain".to_string(),
                "sunny".to_string(),
                "climate".to_string(),
            ],
            vec![
                "What's the weather today?".to_string(),
                "Will it rain tomorrow?".to_string(),
                "How hot is it in Tokyo?".to_string(),
            ],
        ).await?;

        // Register code search skill
        matcher.register_skill(
            "code_search",
            "Search for code examples and documentation",
            vec![
                "code".to_string(),
                "search".to_string(),
                "example".to_string(),
                "documentation".to_string(),
                "snippet".to_string(),
            ],
            vec![
                "Find examples of async Rust".to_string(),
                "How to use HashMap in Python?".to_string(),
                "Search for React hooks examples".to_string(),
            ],
        ).await?;

        // Register file analyzer skill
        matcher.register_skill(
            "file_analyzer",
            "Analyze files and directories",
            vec![
                "file".to_string(),
                "directory".to_string(),
                "analyze".to_string(),
                "folder".to_string(),
                "structure".to_string(),
            ],
            vec![
                "Analyze this directory structure".to_string(),
                "What files are in the project?".to_string(),
                "Show me the file tree".to_string(),
            ],
        ).await?;

        info!("Registered {} built-in skills", 3);
        Ok(())
    }

    /// Set up default A/B tests
    async fn setup_default_tests(&self) -> Result<()> {
        let active_tests = self.ab_test_manager.get_active_tests().await;

        if active_tests.is_empty() {
            // Create semantic vs keyword matching test
            let semantic_test = TestPresets::semantic_vs_keyword_test();
            self.ab_test_manager.create_test(semantic_test).await?;

            // Create routing threshold test
            let routing_test = TestPresets::routing_threshold_test();
            self.ab_test_manager.create_test(routing_test).await?;

            info!("Created default A/B tests");
        }

        Ok(())
    }

    /// Process a user query with full optimization pipeline
    pub async fn process_query(
        &self,
        query: &str,
        user_id: Option<&str>,
        session_id: &str,
    ) -> Result<OptimizedResponse> {
        let start_time = std::time::Instant::now();

        // 1. Build conversation context
        let context = ConversationContext::new()
            .with_recent_skills(self.get_recent_skills(session_id).await);

        // 2. Route the query
        let routing_decision = self.adaptive_router.route(query, &context).await?;
        info!("Routing decision: {:?} (confidence: {:.2})",
            routing_decision.target_system, routing_decision.confidence);

        // 3. Execute based on routing decision
        let (response, skills_used) = self.execute_routing_decision(
            query,
            &routing_decision,
            &context,
        ).await?;

        // 4. Validate answer if using knowledge sources
        let validated_response = if self.should_validate(&routing_decision) {
            self.validate_response(query, &response, &[]).await?
        } else {
            response
        };

        // 5. Record metrics
        let latency_ms = start_time.elapsed().as_millis() as u64;
        self.record_metrics(query, &routing_decision, latency_ms).await?;

        // 6. Prepare response with metadata
        let optimized_response = OptimizedResponse {
            content: validated_response,
            routing_info: RoutingInfo {
                target_system: format!("{:?}", routing_decision.target_system),
                confidence: routing_decision.confidence,
                reasoning: routing_decision.reasoning,
            },
            skills_used,
            latency_ms,
            feedback_token: uuid::Uuid::new_v4().to_string(),
        };

        Ok(optimized_response)
    }

    /// Execute based on routing decision
    async fn execute_routing_decision(
        &self,
        query: &str,
        decision: &RoutingDecision,
        context: &ConversationContext,
    ) -> Result<(String, Vec<String>)> {
        match &decision.target_system {
            TargetSystem::SkillExecution(skill_name) => {
                // Execute the skill
                let result = self.execute_skill(skill_name, query).await?;
                Ok((result, vec![skill_name.clone()]))
            }

            TargetSystem::System1 => {
                // Fast, simple response
                let response = self.generate_simple_response(query).await?;
                Ok((response, vec![]))
            }

            TargetSystem::System2 => {
                // Balanced reasoning with potential skill usage
                let matches = self.find_matching_skills(query, context).await?;
                if let Some(top_match) = matches.first() {
                    if top_match.confidence_tier.should_suggest() {
                        let result = self.execute_skill(&top_match.skill_name, query).await?;
                        return Ok((result, vec![top_match.skill_name.clone()]));
                    }
                }
                let response = self.generate_balanced_response(query).await?;
                Ok((response, vec![]))
            }

            TargetSystem::System3 => {
                // Deep research - would use full reasoning pipeline
                let response = self.generate_research_response(query).await?;
                Ok((response, vec![]))
            }

            TargetSystem::Clarification => {
                let response = self.generate_clarification_request(query).await?;
                Ok((response, vec![]))
            }

            TargetSystem::MultiStep => {
                let response = self.handle_multi_step(query).await?;
                Ok((response, vec![]))
            }
        }
    }

    /// Find matching skills
    async fn find_matching_skills(
        &self,
        query: &str,
        context: &ConversationContext,
    ) -> Result<Vec<crate::skills::hybrid_matcher::HybridMatch>> {
        let matcher = self.hybrid_matcher.read().await;
        matcher.find_matches(query, context, 5).await
    }

    /// Execute a skill
    async fn execute_skill(&self, skill_name: &str, query: &str) -> Result<String> {
        // This would integrate with the actual skill execution
        // For now, return a placeholder
        Ok(format!("Executed skill '{}' for query: {}", skill_name, query))
    }

    /// Generate simple response (System 1)
    async fn generate_simple_response(&self, query: &str) -> Result<String> {
        // Fast response generation
        Ok(format!("Quick response to: {}", query))
    }

    /// Generate balanced response (System 2)
    async fn generate_balanced_response(&self, query: &str) -> Result<String> {
        // Balanced reasoning
        Ok(format!("Balanced analysis of: {}", query))
    }

    /// Generate research response (System 3)
    async fn generate_research_response(&self, query: &str) -> Result<String> {
        // Deep research
        Ok(format!("Deep research on: {}", query))
    }

    /// Generate clarification request
    async fn generate_clarification_request(&self, query: &str) -> Result<String> {
        Ok(format!(
            "I'm not sure I understand. Could you clarify what you mean by '{}'?
            
You might be asking about:
1. A specific skill or tool
2. General information
3. Code or technical help

Please provide more details so I can help you better.",
            query
        ))
    }

    /// Handle multi-step tasks
    async fn handle_multi_step(&self, query: &str) -> Result<String> {
        Ok(format!(
            "I'll break this down into steps for you:\n\n{}",
            query
        ))
    }

    /// Validate response
    async fn validate_response(
        &self,
        query: &str,
        response: &str,
        sources: &[SourceDocument],
    ) -> Result<String> {
        let validation = self.answer_validator.validate(response, query, sources).await?;

        if validation.is_valid {
            Ok(response.to_string())
        } else {
            // In a real implementation, this would trigger regeneration
            // For now, append validation feedback
            Ok(format!(
                "{}\n\n[Validation: Score {:.2}/1.0]",
                response,
                validation.overall_score
            ))
        }
    }

    /// Check if response should be validated
    fn should_validate(&self, decision: &RoutingDecision) -> bool {
        // Validate responses from System 2 and System 3
        matches!(
            decision.target_system,
            TargetSystem::System2 | TargetSystem::System3
        )
    }

    /// Record metrics for learning
    async fn record_metrics(
        &self,
        query: &str,
        decision: &RoutingDecision,
        latency_ms: u64,
    ) -> Result<()> {
        // Record in adaptive router
        self.adaptive_router.record_latency(query, latency_ms).await;

        // Record in A/B test manager if applicable
        // This would check if user is part of any test and record accordingly

        Ok(())
    }

    /// Submit user feedback
    pub async fn submit_feedback(
        &self,
        feedback_token: &str,
        rating: i8,
        comment: Option<String>,
    ) -> Result<()> {
        let collector = FeedbackCollector::new(self.feedback_system.clone());

        // In a real implementation, look up the original query/response
        // using the feedback_token

        info!("Received feedback: rating={}, token={}", rating, feedback_token);
        Ok(())
    }

    /// Get recent skills for context
    async fn get_recent_skills(&self, session_id: &str) -> Vec<String> {
        // In a real implementation, this would track session history
        vec![]
    }

    /// Get system statistics
    pub async fn get_statistics(&self) -> SystemStatistics {
        let router_stats = self.adaptive_router.get_routing_stats().await;
        let performance_stats = self.adaptive_router.get_performance_stats().await;

        SystemStatistics {
            total_routing_decisions: router_stats.total_decisions,
            average_routing_confidence: router_stats.average_confidence,
            performance_by_system: performance_stats,
        }
    }

    /// Start A/B test
    pub async fn start_ab_test(&self, test_config: crate::evaluation::ab_testing::ABTestConfig) -> Result<String> {
        self.ab_test_manager.create_test(test_config).await
    }

    /// Get A/B test results
    pub async fn get_ab_test_results(&self, test_id: &str) -> Option<crate::evaluation::ab_testing::TestResults> {
        self.ab_test_manager.get_test_results(test_id).await
    }
}

impl Default for OptimizedCrablet {
    fn default() -> Self {
        Self::new()
    }
}

/// Optimized response with metadata
#[derive(Debug, Clone)]
pub struct OptimizedResponse {
    pub content: String,
    pub routing_info: RoutingInfo,
    pub skills_used: Vec<String>,
    pub latency_ms: u64,
    pub feedback_token: String,
}

#[derive(Debug, Clone)]
pub struct RoutingInfo {
    pub target_system: String,
    pub confidence: f32,
    pub reasoning: String,
}

/// System statistics
#[derive(Debug, Clone)]
pub struct SystemStatistics {
    pub total_routing_decisions: u64,
    pub average_routing_confidence: f32,
    pub performance_by_system: std::collections::HashMap<String, crate::cognitive::adaptive_router::SystemPerformance>,
}

use tracing::info;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_optimized_system_creation() {
        let mut system = OptimizedCrablet::new();
        assert!(system.initialize().await.is_ok());
    }

    #[tokio::test]
    async fn test_query_processing() {
        let mut system = OptimizedCrablet::new();
        system.initialize().await.unwrap();

        let response = system.process_query("Hello!", None, "test-session").await;
        assert!(response.is_ok());

        let response = response.unwrap();
        assert!(!response.content.is_empty());
        assert!(response.latency_ms > 0);
    }

    #[tokio::test]
    async fn test_skill_execution_routing() {
        let mut system = OptimizedCrablet::new();
        system.initialize().await.unwrap();

        // This should route to skill execution
        let response = system.process_query("use weather", None, "test-session").await;
        assert!(response.is_ok());

        let response = response.unwrap();
        assert!(response.skills_used.contains(&"weather".to_string()));
    }
}
