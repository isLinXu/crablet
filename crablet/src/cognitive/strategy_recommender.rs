//! Real-time Strategy Recommendation System
//!
//! Provides proactive strategy recommendations based on:
//! - Historical performance data
//! - Current execution context
//! - Task type classification
//!
//! This enables the agent to autonomously select optimal strategies
//! rather than being purely reactive to failures.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::types::Message;

// ============================================================================
// Task Type Classification
// ============================================================================

/// Types of tasks that require different strategy approaches
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskType {
    /// Code generation or implementation tasks
    CodeGeneration,
    /// Data analysis or processing tasks
    DataAnalysis,
    /// Research or information gathering tasks
    Research,
    /// Creative writing or brainstorming tasks
    Creative,
    /// Debugging or error fixing tasks
    Debugging,
    /// General purpose tasks
    General,
}

impl TaskType {
    /// Classify a task based on its description
    pub fn classify(task_description: &str) -> Self {
        let desc_lower = task_description.to_lowercase();

        // Debugging indicators
        if desc_lower.contains("debug")
            || desc_lower.contains("fix")
            || desc_lower.contains("error")
            || desc_lower.contains("bug")
            || desc_lower.contains("crash")
            || desc_lower.contains("issue") {
            return TaskType::Debugging;
        }

        // Code generation indicators
        if desc_lower.contains("implement")
            || desc_lower.contains("write")
            || desc_lower.contains("create")
            || desc_lower.contains("build")
            || desc_lower.contains("generate")
            || desc_lower.contains("function")
            || desc_lower.contains("class")
            || desc_lower.contains("api") {
            return TaskType::CodeGeneration;
        }

        // Data analysis indicators
        if desc_lower.contains("analyze")
            || desc_lower.contains("data")
            || desc_lower.contains("statistics")
            || desc_lower.contains("chart")
            || desc_lower.contains("graph")
            || desc_lower.contains("report")
            || desc_lower.contains("metric") {
            return TaskType::DataAnalysis;
        }

        // Research indicators
        if desc_lower.contains("research")
            || desc_lower.contains("find")
            || desc_lower.contains("search")
            || desc_lower.contains("lookup")
            || desc_lower.contains("information")
            || desc_lower.contains("compare") {
            return TaskType::Research;
        }

        // Creative indicators
        if desc_lower.contains("creative")
            || desc_lower.contains("write")
            || desc_lower.contains("story")
            || desc_lower.contains("design")
            || desc_lower.contains("brainstorm")
            || desc_lower.contains("idea") {
            return TaskType::Creative;
        }

        TaskType::General
    }

    /// Get the canonical name for this task type
    pub fn name(&self) -> &'static str {
        match self {
            TaskType::CodeGeneration => "CodeGeneration",
            TaskType::DataAnalysis => "DataAnalysis",
            TaskType::Research => "Research",
            TaskType::Creative => "Creative",
            TaskType::Debugging => "Debugging",
            TaskType::General => "General",
        }
    }
}

// ============================================================================
// Strategy Performance Tracking
// ============================================================================

/// Score for a specific strategy on a specific task type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyScore {
    /// Number of times this strategy was used for this task type
    pub usage_count: u32,
    /// Number of successful executions
    pub success_count: u32,
    /// Average execution time in milliseconds
    pub avg_duration_ms: f64,
    /// Average quality score (0-1)
    pub avg_quality: f64,
    /// Composite score computed from above metrics
    pub composite_score: f64,
    /// Last updated timestamp
    pub last_updated: DateTime<Utc>,
}

impl StrategyScore {
    pub fn new() -> Self {
        Self {
            usage_count: 0,
            success_count: 0,
            avg_duration_ms: 0.0,
            avg_quality: 0.0,
            composite_score: 0.0,
            last_updated: Utc::now(),
        }
    }

    /// Update score with a new observation
    pub fn update(&mut self, success: bool, duration_ms: f64, quality: f64) {
        self.usage_count += 1;

        if success {
            self.success_count += 1;
        }

        // Running average for duration (lower is better)
        self.avg_duration_ms = (self.avg_duration_ms * (self.usage_count - 1) as f64 + duration_ms)
            / self.usage_count as f64;

        // Running average for quality
        self.avg_quality = (self.avg_quality * (self.usage_count - 1) as f64 + quality)
            / self.usage_count as f64;

        // Compute composite score: success_rate * 0.5 + quality * 0.3 + speed_factor * 0.2
        let success_rate = self.success_count as f64 / self.usage_count as f64;
        let speed_factor = 1.0_f64.min(10000.0 / self.avg_duration_ms.max(1.0)); // Normalize to ~1.0

        self.composite_score = success_rate * 0.5 + self.avg_quality * 0.3 + speed_factor.min(1.0) * 0.2;
        self.last_updated = Utc::now();
    }
}

impl Default for StrategyScore {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Strategy Recommendation
// ============================================================================

/// Result of a strategy recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyRecommendation {
    /// The recommended strategy name
    pub recommended_strategy: String,
    /// Confidence score (0-1)
    pub confidence: f64,
    /// Alternative strategies with their scores
    pub alternatives: Vec<StrategyAlternative>,
    /// Reasoning for the recommendation
    pub reasoning: String,
    /// Task type that was classified
    pub task_type: String,
    /// Factors that influenced the decision
    pub factors: Vec<String>,
}

/// An alternative strategy with its score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyAlternative {
    pub strategy_name: String,
    pub score: f64,
    pub reason: String,
}

// ============================================================================
// Strategy Recommender
// ============================================================================

/// Configuration for the strategy recommender
#[derive(Debug, Clone)]
pub struct RecommenderConfig {
    /// Minimum usage count before trusting a score
    pub min_usage_for_trust: u32,
    /// Weight for historical performance
    pub history_weight: f64,
    /// Weight for current context
    pub context_weight: f64,
    /// Exploration probability (for trying new strategies)
    pub exploration_rate: f64,
}

impl Default for RecommenderConfig {
    fn default() -> Self {
        Self {
            min_usage_for_trust: 3,
            history_weight: 0.7,
            context_weight: 0.3,
            exploration_rate: 0.1,
        }
    }
}

/// Historical performance record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRecord {
    pub task_type: TaskType,
    pub strategy: String,
    pub success: bool,
    pub duration_ms: f64,
    pub quality: f64,
    pub timestamp: DateTime<Utc>,
}

/// Real-time Strategy Recommender
pub struct StrategyRecommender {
    config: RecommenderConfig,
    /// Historical performance: (task_type, strategy) -> score
    performance: Arc<RwLock<HashMap<(TaskType, String), StrategyScore>>>,
    /// Recent performance records for context
    recent_records: Arc<RwLock<Vec<PerformanceRecord>>>,
    /// Available strategies
    available_strategies: Vec<String>,
    /// Strategy descriptions for reasoning
    strategy_descriptions: HashMap<String, String>,
}

impl StrategyRecommender {
    /// Create a new strategy recommender
    pub fn new(config: RecommenderConfig) -> Self {
        Self {
            config,
            performance: Arc::new(RwLock::new(HashMap::new())),
            recent_records: Arc::new(RwLock::new(Vec::new())),
            available_strategies: vec![
                "System1".to_string(),
                "System2".to_string(),
                "System3".to_string(),
                "System4".to_string(),
                "React".to_string(),
                "Plan".to_string(),
                "Debug".to_string(),
                "Research".to_string(),
            ],
            strategy_descriptions: HashMap::from([
                ("System1".to_string(), "Fast, intuitive responses for simple tasks".to_string()),
                ("System2".to_string(), "Deliberate reasoning for complex tasks".to_string()),
                ("System3".to_string(), "Memory-augmented reasoning".to_string()),
                ("System4".to_string(), "Creative and exploratory thinking".to_string()),
                ("React".to_string(), "ReAct: Reasoning + Acting for interactive tasks".to_string()),
                ("Plan".to_string(), "Planning-based approach for multi-step tasks".to_string()),
                ("Debug".to_string(), "Systematic debugging and error fixing".to_string()),
                ("Research".to_string(), "Deep research and information gathering".to_string()),
            ]),
        }
    }

    /// Create with default configuration
    pub fn with_strategies(strategies: Vec<String>) -> Self {
        let mut recommender = Self::new(RecommenderConfig::default());
        recommender.available_strategies = strategies;
        recommender
    }

    /// Record a performance observation
    pub async fn record(&self, record: PerformanceRecord) {
        // Update performance map
        let key = (record.task_type.clone(), record.strategy.clone());
        let mut perf = self.performance.write().await;
        let score = perf.entry(key).or_insert_with(StrategyScore::new);
        score.update(record.success, record.duration_ms, record.quality);

        // Add to recent records (keep last 100)
        let mut recent = self.recent_records.write().await;
        recent.push(record);
        if recent.len() > 100 {
            recent.remove(0);
        }
    }

    /// Record a performance observation with individual parameters
    pub async fn record_performance(
        &self,
        task_type: TaskType,
        strategy: String,
        success: bool,
        duration_ms: f64,
        quality: f64,
    ) {
        self.record(PerformanceRecord {
            task_type,
            strategy,
            success,
            duration_ms,
            quality,
            timestamp: Utc::now(),
        }).await;
    }

    /// Get the best strategy for a task type from historical data
    async fn get_best_for_task_type(&self, task_type: TaskType) -> Option<(String, f64)> {
        let perf = self.performance.read().await;
        let mut best: Option<(String, f64)> = None;

        for ((tt, strategy), score) in perf.iter() {
            if *tt == task_type && score.usage_count >= self.config.min_usage_for_trust {
                if let Some((_, best_score)) = &best {
                    if score.composite_score > *best_score {
                        best = Some((strategy.clone(), score.composite_score));
                    }
                } else {
                    best = Some((strategy.clone(), score.composite_score));
                }
            }
        }

        best
    }

    /// Analyze recent context to adjust recommendations
    async fn analyze_context(&self, task: &str, messages: &[Message]) -> ContextAnalysis {
        let mut factors = Vec::new();
        let mut complexity_score = 0.5_f64;
        let mut requires_memory = false;
        let mut requires_planning = false;

        // Analyze task complexity
        let task_len = task.len();
        if task_len > 500 {
            complexity_score = 0.8;
            factors.push("Complex task: long description".to_string());
        } else if task_len > 200 {
            complexity_score = 0.6;
            factors.push("Moderate task complexity".to_string());
        }

        // Check for planning indicators
        let task_lower = task.to_lowercase();
        if task_lower.contains("step")
            || task_lower.contains("plan")
            || task_lower.contains("first")
            || task_lower.contains("then")
            || task_lower.contains("finally") {
            requires_planning = true;
            factors.push("Planning required: multi-step task".to_string());
        }

        // Check for debugging indicators
        if task_lower.contains("debug")
            || task_lower.contains("fix")
            || task_lower.contains("error")
            || task_lower.contains("bug") {
            factors.push("Debugging task detected".to_string());
        }

        // Analyze message context
        let msg_count = messages.len();
        if msg_count > 5 {
            requires_memory = true;
            complexity_score = (complexity_score + 0.7) / 2.0;
            factors.push("Long conversation context".to_string());
        }

        // Check for repeated failures in recent messages
        let recent = self.recent_records.read().await;
        let recent_failures: usize = recent.iter()
            .rev()
            .take(10)
            .filter(|r| !r.success)
            .count();

        if recent_failures >= 3 {
            factors.push(format!("Recent failures: {}/10", 10 - recent_failures));
            complexity_score = 0.9; // Increase complexity estimate
        }

        ContextAnalysis {
            factors,
            complexity_score,
            requires_memory,
            requires_planning,
            recent_success_rate: if !recent.is_empty() {
                recent.iter().rev().take(10).filter(|r| r.success).count() as f64 / 10.0
            } else {
                0.5
            },
        }
    }

    /// Generate reasoning explanation for a recommendation
    fn generate_reasoning(
        &self,
        task_type: TaskType,
        _best_strategy: &str,
        best_score: f64,
        context: &ContextAnalysis,
    ) -> String {
        let mut reasons = Vec::new();

        reasons.push(format!(
            "Task classified as {} based on content analysis",
            task_type.name()
        ));

        if best_score > 0.7 {
            reasons.push(format!(
                "Strong historical performance ({:.1}% success rate)",
                best_score * 100.0
            ));
        } else if best_score > 0.4 {
            reasons.push("Moderate historical performance".to_string());
        } else {
            reasons.push("Limited historical data, using default strategy".to_string());
        }

        if context.requires_planning {
            reasons.push("Task requires planning capability".to_string());
        }

        if context.complexity_score > 0.7 {
            reasons.push("High complexity suggests deliberate reasoning".to_string());
        }

        if context.recent_success_rate < 0.5 {
            reasons.push("Recent performance issues detected - considering alternatives".to_string());
        }

        reasons.join(". ")
    }

    /// Recommend a strategy for the given task and context
    pub async fn recommend(
        &self,
        task: &str,
        messages: &[Message],
    ) -> StrategyRecommendation {
        let task_type = TaskType::classify(task);
        let context = self.analyze_context(task, messages).await;

        // Get best from history
        let historical_best = self.get_best_for_task_type(task_type).await;

        // Decide on final recommendation
        let (recommended_strategy, confidence, reasoning, historical_best_score) =
            if let Some((strategy, score)) = historical_best.clone() {
            // Use exploration rate to sometimes try alternatives
            let use_exploration = rand::random::<f64>() < self.config.exploration_rate;

            if use_exploration {
                // Pick a random alternative
                let alternatives: Vec<_> = self.available_strategies.iter()
                    .filter(|s| *s != &strategy)
                    .take(3)
                    .cloned()
                    .collect();

                let alt_strategy = alternatives.first().cloned().unwrap_or_else(|| strategy.clone());
                (
                    alt_strategy.clone(),
                    score * 0.7, // Lower confidence for exploration
                    format!("Exploration: trying {} instead of proven {}", alt_strategy, strategy),
                    Some(score),
                )
            } else {
                (strategy.clone(), score, self.generate_reasoning(task_type, &strategy, score, &context), Some(score))
            }
        } else {
            // No historical data - use task type defaults
            let default_strategy = match task_type {
                TaskType::Debugging => "Debug".to_string(),
                TaskType::CodeGeneration => "System2".to_string(),
                TaskType::DataAnalysis => "System3".to_string(),
                TaskType::Research => "Research".to_string(),
                TaskType::Creative => "System4".to_string(),
                TaskType::General => "System1".to_string(),
            };

            (
                default_strategy.clone(),
                0.5,
                format!("No history for {} task, using default: {}", task_type.name(), default_strategy),
                None,
            )
        };

        // Build alternatives list
        let mut alternatives = Vec::new();
        if let Some(score) = historical_best_score {
            for (strategy, desc) in &self.strategy_descriptions {
                if strategy != &recommended_strategy {
                    alternatives.push(StrategyAlternative {
                        strategy_name: strategy.clone(),
                        score: score * 0.9, // Slightly lower than best
                        reason: desc.clone(),
                    });
                }
            }
        }

        // Sort by score
        alternatives.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        alternatives.truncate(3);

        StrategyRecommendation {
            recommended_strategy,
            confidence,
            alternatives,
            reasoning,
            task_type: task_type.name().to_string(),
            factors: context.factors,
        }
    }

    /// Get performance statistics for all strategies
    pub async fn get_statistics(&self) -> StrategyStatistics {
        let perf = self.performance.read().await;

        let mut stats: HashMap<String, TaskTypeStats> = HashMap::new();

        for ((task_type, strategy), score) in perf.iter() {
            let entry = stats.entry(strategy.clone()).or_insert_with(TaskTypeStats::default);
            entry.total_usage += score.usage_count;
            entry.total_success += score.success_count;
            entry.task_types.push(task_type.name().to_string());
        }

        StrategyStatistics {
            by_strategy: stats,
            total_records: self.recent_records.read().await.len(),
        }
    }

    /// Clear historical performance data
    pub async fn clear_history(&self) {
        self.performance.write().await.clear();
        self.recent_records.write().await.clear();
    }
}

/// Context analysis result
#[derive(Debug, Clone)]
struct ContextAnalysis {
    factors: Vec<String>,
    complexity_score: f64,
    requires_memory: bool,
    requires_planning: bool,
    recent_success_rate: f64,
}

/// Statistics for a specific strategy
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskTypeStats {
    pub total_usage: u32,
    pub total_success: u32,
    pub task_types: Vec<String>,
}

impl TaskTypeStats {
    pub fn new() -> Self {
        Self {
            total_usage: 0,
            total_success: 0,
            task_types: Vec::new(),
        }
    }
}

/// Overall strategy statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyStatistics {
    pub by_strategy: HashMap<String, TaskTypeStats>,
    pub total_records: usize,
}

// ============================================================================
// Preset Task Definitions
// ============================================================================

/// Preset task definitions for evaluation
pub struct TestTasks;

impl TestTasks {
    /// Get preset test tasks
    pub fn code_generation() -> Vec<&'static str> {
        vec![
            "Write a function to calculate fibonacci numbers",
            "Implement a REST API endpoint for user management",
            "Create a sorting algorithm for large datasets",
        ]
    }

    pub fn debugging() -> Vec<&'static str> {
        vec![
            "Debug why the login page returns 500 error",
            "Fix the memory leak in the image processing module",
            "Investigate why the cache is not invalidating properly",
        ]
    }

    pub fn data_analysis() -> Vec<&'static str> {
        vec![
            "Analyze the sales data and find trends",
            "Generate a report on user engagement metrics",
            "Process the log files to find performance bottlenecks",
        ]
    }

    pub fn research() -> Vec<&'static str> {
        vec![
            "Research best practices for API authentication",
            "Find information about distributed consensus algorithms",
            "Investigate the latest developments in LLM fine-tuning",
        ]
    }
}

// ============================================================================
// Global Strategy Recommender Instance
// ============================================================================

use std::sync::OnceLock;

static GLOBAL_RECOMMENDER: OnceLock<Arc<StrategyRecommender>> = OnceLock::new();

/// Get or initialize the global strategy recommender
pub fn global_recommender() -> Arc<StrategyRecommender> {
    GLOBAL_RECOMMENDER
        .get_or_init(|| Arc::new(StrategyRecommender::new(RecommenderConfig::default())))
        .clone()
}

/// Initialize global recommender with custom config
pub fn init_global_recommender(config: RecommenderConfig) -> Arc<StrategyRecommender> {
    GLOBAL_RECOMMENDER
        .get_or_init(|| Arc::new(StrategyRecommender::new(config)))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_classification() {
        assert_eq!(
            TaskType::classify("Debug why the login is failing"),
            TaskType::Debugging
        );
        assert_eq!(
            TaskType::classify("Implement a new REST API endpoint"),
            TaskType::CodeGeneration
        );
        assert_eq!(
            TaskType::classify("Analyze the sales data for trends"),
            TaskType::DataAnalysis
        );
    }

    #[tokio::test]
    async fn test_recommendation() {
        let recommender = StrategyRecommender::new(RecommenderConfig::default());

        // Record some performance
        recommender.record_performance(
            TaskType::CodeGeneration,
            "System2".to_string(),
            true,
            1000.0,
            0.9,
        ).await;

        recommender.record_performance(
            TaskType::CodeGeneration,
            "System1".to_string(),
            false,
            500.0,
            0.5,
        ).await;

        // Get recommendation
        let rec = recommender.recommend(
            "Write a function to calculate primes",
            &[],
        ).await;

        assert_eq!(rec.task_type, "CodeGeneration");
        assert!(!rec.recommended_strategy.is_empty());
    }
}
