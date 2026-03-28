//! Execution Path Explorer - Beam Search for Optimal Execution Paths
//!
//! This module implements intelligent execution path exploration using
//! Beam Search algorithm. Instead of committing to a single path, it:
//! 1. Generates multiple candidate paths at each step
//! 2. Evaluates each path using a scoring function
//! 3. Keeps the top-k (beam width) candidates
//! 4. Supports backtracking when paths fail
//!
//! This enables agents to explore different approaches before committing,
//! reducing failures on complex tasks.

use std::collections::BinaryHeap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use rand::Rng;

// ============================================================================
// Execution Path Types
// ============================================================================

/// A single step in an execution path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathStep {
    /// Step index (0-indexed)
    pub index: usize,
    /// The action/tool taken at this step
    pub action: String,
    /// Arguments to the action
    pub arguments: serde_json::Value,
    /// Expected outcome description
    pub expected_outcome: String,
    /// Confidence score (0-1) for this step
    pub confidence: f64,
    /// Estimated cost in tokens
    pub estimated_cost: u32,
}

/// A candidate execution path
#[derive(Debug, Clone)]
pub struct ExecutionPath {
    /// Unique path identifier
    pub id: String,
    /// Steps in this path
    pub steps: Vec<PathStep>,
    /// Total estimated cost
    pub total_cost: u32,
    /// Expected success rate (0-1)
    pub expected_success_rate: f64,
    /// Composite score (higher is better)
    pub score: f64,
    /// Whether this path is complete
    pub is_complete: bool,
    /// Parent path ID (for backtracking)
    pub parent_id: Option<String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
}

impl ExecutionPath {
    /// Create a new execution path
    pub fn new(id: String) -> Self {
        Self {
            id,
            steps: Vec::new(),
            total_cost: 0,
            expected_success_rate: 1.0,
            score: 0.0,
            is_complete: false,
            parent_id: None,
            created_at: Utc::now(),
        }
    }

    /// Add a step to the path
    pub fn add_step(&mut self, step: PathStep) {
        self.total_cost += step.estimated_cost;
        self.expected_success_rate *= step.confidence;
        self.steps.push(step);
    }

    /// Finalize the score
    pub fn finalize_score(&mut self) {
        // Score = success_rate * (1 / (1 + cost_normalized))
        let cost_factor = 1.0_f64 / (1.0 + (self.total_cost as f64 / 1000.0));
        self.score = self.expected_success_rate * 0.7 + cost_factor * 0.3;
    }

    /// Mark path as complete
    pub fn complete(&mut self) {
        self.is_complete = true;
        self.finalize_score();
    }
}

/// Comparison for BinaryHeap (max-heap by score)
impl PartialEq for ExecutionPath {
    fn eq(&self, other: &Self) -> bool {
        self.score == other.score
    }
}

impl Eq for ExecutionPath {}

impl PartialOrd for ExecutionPath {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.score.partial_cmp(&other.score).unwrap_or(std::cmp::Ordering::Equal))
    }
}

impl Ord for ExecutionPath {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

// ============================================================================
// Action Generation
// ============================================================================

/// Possible action to take at a decision point
#[derive(Debug, Clone)]
pub struct CandidateAction {
    pub action: String,
    pub arguments: serde_json::Value,
    pub expected_outcome: String,
    pub confidence: f64,
    pub estimated_cost: u32,
    pub reasoning: String,
}

/// Protocol for generating candidate actions
pub trait ActionGenerator: Send + Sync {
    /// Generate candidate actions for the current state
    fn generate_candidates(
        &self,
        task: &str,
        context: &[PathStep],
        num_candidates: usize,
    ) -> Vec<CandidateAction>;
}

/// LLM-based action generator
pub struct LlmActionGenerator {
    available_tools: Vec<String>,
}

impl LlmActionGenerator {
    pub fn new(tools: Vec<String>) -> Self {
        Self { available_tools: tools }
    }
}

impl ActionGenerator for LlmActionGenerator {
    fn generate_candidates(
        &self,
        task: &str,
        _context: &[PathStep],
        num_candidates: usize,
    ) -> Vec<CandidateAction> {
        let mut rng = rand::thread_rng();
        let mut candidates = Vec::new();

        // Generate candidates based on available tools and context
        let base_actions = vec![
            "search",
            "read_file",
            "write_to_file",
            "execute_command",
            "analyze",
            "summarize",
            "plan",
        ];

        for (i, action) in base_actions.iter().take(num_candidates).enumerate() {
            let confidence = rng.gen_range(0.5..0.95);
            candidates.push(CandidateAction {
                action: action.to_string(),
                arguments: serde_json::json!({"query": format!("Step {} for: {}", i, task)}),
                expected_outcome: format!("Progress toward completing: {}", task),
                confidence,
                estimated_cost: rng.gen_range(100..500),
                reasoning: format!("{} is a reasonable action for this task", action),
            });
        }

        candidates
    }
}

/// Heuristic-based action generator (rule-based)
pub struct HeuristicActionGenerator {
    rules: Vec<ActionRule>,
}

#[derive(Debug, Clone)]
pub struct ActionRule {
    pub condition: String,
    pub action: String,
    pub arguments_template: serde_json::Value,
    pub confidence: f64,
}

impl HeuristicActionGenerator {
    pub fn new() -> Self {
        Self {
            rules: vec![
                ActionRule {
                    condition: "debug".to_string(),
                    action: "analyze".to_string(),
                    arguments_template: serde_json::json!({"focus": "error_cause"}),
                    confidence: 0.85,
                },
                ActionRule {
                    condition: "create".to_string(),
                    action: "write_to_file".to_string(),
                    arguments_template: serde_json::json!({"content": "placeholder"}),
                    confidence: 0.8,
                },
                ActionRule {
                    condition: "find".to_string(),
                    action: "search".to_string(),
                    arguments_template: serde_json::json!({"query": "task query"}),
                    confidence: 0.75,
                },
            ],
        }
    }
}

impl ActionGenerator for HeuristicActionGenerator {
    fn generate_candidates(
        &self,
        task: &str,
        _context: &[PathStep],
        num_candidates: usize,
    ) -> Vec<CandidateAction> {
        let task_lower = task.to_lowercase();
        let mut candidates = Vec::new();

        for rule in &self.rules {
            if task_lower.contains(&rule.condition) {
                candidates.push(CandidateAction {
                    action: rule.action.clone(),
                    arguments: rule.arguments_template.clone(),
                    expected_outcome: format!("Apply {} rule", rule.action),
                    confidence: rule.confidence,
                    estimated_cost: 200,
                    reasoning: format!("Matched rule: {}", rule.condition),
                });
            }
        }

        // If no rules match, add generic actions
        if candidates.is_empty() {
            candidates.push(CandidateAction {
                action: "plan".to_string(),
                arguments: serde_json::json!({"task": task}),
                expected_outcome: "Create a step-by-step plan".to_string(),
                confidence: 0.7,
                estimated_cost: 150,
                reasoning: "No specific rule matched, creating a plan".to_string(),
            });
        }

        candidates.truncate(num_candidates);
        candidates
    }
}

// ============================================================================
// Path Evaluation
// ============================================================================

/// Result of evaluating an execution path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathEvaluation {
    pub path_id: String,
    pub score: f64,
    pub success_probability: f64,
    pub estimated_cost: u32,
    pub risk_factors: Vec<String>,
    pub recommendations: Vec<String>,
}

/// Protocol for evaluating paths
pub trait PathEvaluator: Send + Sync {
    fn evaluate(&self, path: &ExecutionPath, task: &str) -> PathEvaluation;
}

/// Default evaluator using heuristics
pub struct HeuristicPathEvaluator {
    max_steps: usize,
    max_cost: u32,
}

impl HeuristicPathEvaluator {
    pub fn new(max_steps: usize, max_cost: u32) -> Self {
        Self { max_steps, max_cost }
    }
}

impl PathEvaluator for HeuristicPathEvaluator {
    fn evaluate(&self, path: &ExecutionPath, _task: &str) -> PathEvaluation {
        let mut risk_factors = Vec::new();
        let mut recommendations = Vec::new();

        // Check step count
        if path.steps.len() > self.max_steps {
            risk_factors.push(format!(
                "Path has {} steps, exceeds limit of {}",
                path.steps.len(),
                self.max_steps
            ));
        }

        // Check cost
        if path.total_cost > self.max_cost {
            risk_factors.push(format!(
                "Path cost {} exceeds budget {}",
                path.total_cost, self.max_cost
            ));
        }

        // Check for repetition
        let actions: Vec<_> = path.steps.iter().map(|s| s.action.clone()).collect();
        let unique_actions: std::collections::HashSet<_> = actions.iter().collect();
        if unique_actions.len() < actions.len() / 2 {
            risk_factors.push("Path has many repeated actions".to_string());
            recommendations.push("Consider consolidating repeated operations".to_string());
        }

        // Generate recommendations
        if path.expected_success_rate < 0.5 {
            recommendations.push("Low success probability - consider alternative approach".to_string());
        }

        if path.steps.is_empty() {
            recommendations.push("Path has no steps - starting from scratch".to_string());
        }

        PathEvaluation {
            path_id: path.id.clone(),
            score: path.score,
            success_probability: path.expected_success_rate,
            estimated_cost: path.total_cost,
            risk_factors,
            recommendations,
        }
    }
}

// ============================================================================
// Execution Path Explorer
// ============================================================================

/// Configuration for the explorer
#[derive(Debug, Clone)]
pub struct ExplorerConfig {
    /// Beam width - number of candidate paths to keep
    pub beam_width: usize,
    /// Maximum depth/steps per path
    pub max_depth: usize,
    /// Maximum total cost per path
    pub max_cost: u32,
    /// Probability of exploring alternatives vs exploiting best
    pub exploration_rate: f64,
    /// Enable backtracking on failure
    pub enable_backtrack: bool,
    /// Maximum backtrack attempts
    pub max_backtrack: usize,
}

impl Default for ExplorerConfig {
    fn default() -> Self {
        Self {
            beam_width: 3,
            max_depth: 10,
            max_cost: 5000,
            exploration_rate: 0.2,
            enable_backtrack: true,
            max_backtrack: 2,
        }
    }
}

/// Exploration result
#[derive(Debug, Clone)]
pub struct ExplorationResult {
    /// The best path found
    pub best_path: ExecutionPath,
    /// All explored paths (for analysis)
    pub explored_paths: Vec<ExecutionPath>,
    /// Number of paths explored
    pub paths_explored: usize,
    /// Total exploration time in ms
    pub exploration_time_ms: u64,
    /// Whether backtracking was used
    pub used_backtrack: bool,
    /// Final evaluation of best path
    pub evaluation: PathEvaluation,
}

/// The main execution path explorer
pub struct ExecutionPathExplorer {
    config: ExplorerConfig,
    action_generator: Box<dyn ActionGenerator>,
    path_evaluator: Box<dyn PathEvaluator>,
}

impl ExecutionPathExplorer {
    /// Create a new explorer with default components
    pub fn new(config: ExplorerConfig) -> Self {
        let max_depth = config.max_depth;
        let max_cost = config.max_cost;
        Self {
            config,
            action_generator: Box::new(HeuristicActionGenerator::new()),
            path_evaluator: Box::new(HeuristicPathEvaluator::new(max_depth, max_cost)),
        }
    }

    /// Create with custom action generator
    pub fn with_generator(
        config: ExplorerConfig,
        generator: Box<dyn ActionGenerator>,
    ) -> Self {
        let max_depth = config.max_depth;
        let max_cost = config.max_cost;
        Self {
            config,
            action_generator: generator,
            path_evaluator: Box::new(HeuristicPathEvaluator::new(max_depth, max_cost)),
        }
    }

    /// Generate initial candidate paths
    fn generate_initial_paths(&self, task: &str) -> Vec<ExecutionPath> {
        let candidates = self.action_generator.generate_candidates(task, &[], self.config.beam_width);

        candidates
            .into_iter()
            .enumerate()
            .map(|(i, candidate)| {
                let mut path = ExecutionPath::new(format!("init_{}", i));
                path.add_step(PathStep {
                    index: 0,
                    action: candidate.action,
                    arguments: candidate.arguments,
                    expected_outcome: candidate.expected_outcome,
                    confidence: candidate.confidence,
                    estimated_cost: candidate.estimated_cost,
                });
                path.finalize_score();
                path
            })
            .collect()
    }

    /// Expand paths by one step
    fn expand_paths(&self, paths: &[ExecutionPath], task: &str) -> Vec<ExecutionPath> {
        let mut expanded = Vec::new();

        for path in paths {
            if path.is_complete || path.steps.len() >= self.config.max_depth {
                // Mark complete and keep
                let mut complete_path = path.clone();
                complete_path.complete();
                expanded.push(complete_path);
                continue;
            }

            // Generate candidates for this path
            let candidates = self.action_generator.generate_candidates(
                task,
                &path.steps,
                self.config.beam_width,
            );

            for (i, candidate) in candidates.into_iter().enumerate() {
                let mut new_path = path.clone();
                new_path.id = format!("{}_step{}_{}", path.id, path.steps.len(), i);
                new_path.parent_id = Some(path.id.clone());
                new_path.add_step(PathStep {
                    index: path.steps.len(),
                    action: candidate.action,
                    arguments: candidate.arguments,
                    expected_outcome: candidate.expected_outcome,
                    confidence: candidate.confidence,
                    estimated_cost: candidate.estimated_cost,
                });
                new_path.finalize_score();
                expanded.push(new_path);
            }
        }

        expanded
    }

    /// Select top-k paths using beam search
    fn select_top_k(&self, paths: &mut Vec<ExecutionPath>) -> Vec<ExecutionPath> {
        // Use BinaryHeap for efficient top-k selection
        let mut heap = BinaryHeap::new();
        for path in paths.drain(..) {
            heap.push(path);
        }

        let mut selected = Vec::new();
        for _ in 0..self.config.beam_width.min(heap.len()) {
            if let Some(path) = heap.pop() {
                selected.push(path);
            }
        }

        selected
    }

    /// Check if any path is complete and good enough
    fn should_stop(&self, paths: &[ExecutionPath]) -> bool {
        paths.iter().any(|p| p.is_complete && p.score > 0.8)
    }

    /// Backtrack to explore alternative branches
    fn backtrack(&self, paths: &[ExecutionPath]) -> Vec<ExecutionPath> {
        if !self.config.enable_backtrack || paths.is_empty() {
            return Vec::new();
        }

        // Find paths that can be branched differently
        let mut alternatives = Vec::new();

        for path in paths {
            if let Some(parent_id) = &path.parent_id {
                // This is a child path - generate sibling alternatives
                let siblings = self.action_generator.generate_candidates(
                    "",
                    &[], // Would need parent context in real impl
                    2,
                );

                for (i, sibling) in siblings.into_iter().enumerate() {
                    let mut alt_path = ExecutionPath::new(format!("bt_{}_{}", parent_id, i));
                    alt_path.parent_id = Some(parent_id.clone());
                    alt_path.add_step(PathStep {
                        index: 0,
                        action: sibling.action,
                        arguments: sibling.arguments,
                        expected_outcome: sibling.expected_outcome,
                        confidence: sibling.confidence * 0.9, // Slight penalty for backtrack
                        estimated_cost: sibling.estimated_cost,
                    });
                    alt_path.finalize_score();
                    alternatives.push(alt_path);
                }
            }
        }

        alternatives
    }

    /// Main exploration loop using Beam Search
    pub fn explore(&self, task: &str) -> ExplorationResult {
        let start_time = std::time::Instant::now();
        let mut current_paths = self.generate_initial_paths(task);
        let mut all_explored = current_paths.clone();
        let mut used_backtrack = false;
        let mut backtrack_count = 0;

        // Beam search loop
        for _depth in 0..self.config.max_depth {
            // Check stopping condition
            if self.should_stop(&current_paths) {
                break;
            }

            // Expand paths
            current_paths = self.expand_paths(&current_paths, task);
            all_explored.extend(current_paths.clone());

            // Select top-k
            current_paths = self.select_top_k(&mut current_paths.clone());

            // Exploration vs exploitation
            if rand::random::<f64>() < self.config.exploration_rate {
                let alternatives = self.backtrack(&current_paths);
                if !alternatives.is_empty() && backtrack_count < self.config.max_backtrack {
                    current_paths.extend(alternatives);
                    used_backtrack = true;
                    backtrack_count += 1;
                }
            }
        }

        // Mark remaining paths as complete
        for path in &mut current_paths {
            if !path.is_complete {
                path.complete();
            }
        }

        // Select final best path
        current_paths.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let best_path = current_paths
            .pop()
            .unwrap_or_else(|| ExecutionPath::new("empty".to_string()));

        let evaluation = self.path_evaluator.evaluate(&best_path, task);
        let paths_explored_count = all_explored.len();

        ExplorationResult {
            best_path,
            explored_paths: all_explored,
            paths_explored: paths_explored_count,
            exploration_time_ms: start_time.elapsed().as_millis() as u64,
            used_backtrack,
            evaluation,
        }
    }

    /// Async version of explore
    pub async fn explore_async(&self, task: &str) -> ExplorationResult {
        // In a real implementation, this would spawn tasks for parallel exploration
        // For now, delegate to sync version
        let task_str = task.to_string();
        let config = self.config.clone();

        tokio::task::spawn_blocking(move || {
            let explorer = ExecutionPathExplorer::new(config);
            explorer.explore(&task_str)
        })
        .await
        .unwrap_or_else(|_| ExplorationResult {
            best_path: ExecutionPath::new("error".to_string()),
            explored_paths: Vec::new(),
            paths_explored: 0,
            exploration_time_ms: 0,
            used_backtrack: false,
            evaluation: PathEvaluation {
                path_id: "error".to_string(),
                score: 0.0,
                success_probability: 0.0,
                estimated_cost: 0,
                risk_factors: vec!["Exploration failed".to_string()],
                recommendations: vec!["Try again".to_string()],
            },
        })
    }
}

// ============================================================================
// Preset Configurations
// ============================================================================

/// Preset configurations for different use cases
pub struct ExplorerPresets;

impl ExplorerPresets {
    /// Fast exploration - few steps, high exploration rate
    pub fn fast() -> ExplorerConfig {
        ExplorerConfig {
            beam_width: 2,
            max_depth: 5,
            max_cost: 2000,
            exploration_rate: 0.3,
            enable_backtrack: false,
            max_backtrack: 0,
        }
    }

    /// Thorough exploration - many steps, low exploration rate
    pub fn thorough() -> ExplorerConfig {
        ExplorerConfig {
            beam_width: 5,
            max_depth: 15,
            max_cost: 10000,
            exploration_rate: 0.1,
            enable_backtrack: true,
            max_backtrack: 3,
        }
    }

    /// Balanced exploration
    pub fn balanced() -> ExplorerConfig {
        ExplorerConfig::default()
    }
}

// ============================================================================
// Global Explorer Instance
// ============================================================================

use std::sync::OnceLock;

static GLOBAL_EXPLORER: OnceLock<Arc<ExecutionPathExplorer>> = OnceLock::new();

/// Get or initialize the global explorer
pub fn global_explorer() -> Arc<ExecutionPathExplorer> {
    GLOBAL_EXPLORER
        .get_or_init(|| Arc::new(ExecutionPathExplorer::new(ExplorerConfig::default())))
        .clone()
}

/// Initialize global explorer with custom config
pub fn init_global_explorer(config: ExplorerConfig) -> Arc<ExecutionPathExplorer> {
    GLOBAL_EXPLORER
        .get_or_init(|| Arc::new(ExecutionPathExplorer::new(config)))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_creation() {
        let path = ExecutionPath::new("test_1".to_string());
        assert_eq!(path.id, "test_1");
        assert!(!path.is_complete);
        assert_eq!(path.steps.len(), 0);
    }

    #[test]
    fn test_path_scoring() {
        let mut path = ExecutionPath::new("test_1".to_string());
        path.add_step(PathStep {
            index: 0,
            action: "test".to_string(),
            arguments: serde_json::json!({}),
            expected_outcome: "test".to_string(),
            confidence: 0.9,
            estimated_cost: 100,
        });
        path.finalize_score();
        assert!(path.score > 0.0);
        assert!(path.expected_success_rate < 1.0);
    }

    #[test]
    fn test_beam_search() {
        let explorer = ExecutionPathExplorer::new(ExplorerPresets::fast());
        let result = explorer.explore("test task");

        assert!(result.paths_explored > 0);
        assert!(!result.best_path.id.is_empty());
    }

    #[test]
    fn test_heuristic_generator() {
        let generator = HeuristicActionGenerator::new();
        let candidates = generator.generate_candidates(
            "debug the login bug",
            &[],
            3,
        );

        assert!(!candidates.is_empty());
        assert!(candidates.iter().any(|c| c.action == "analyze" || c.action == "plan"));
    }
}
