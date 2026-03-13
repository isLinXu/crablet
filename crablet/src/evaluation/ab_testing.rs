//! A/B Testing Framework
//!
//! Provides comprehensive A/B testing capabilities for evaluating
//! different routing strategies, skill matching algorithms, and response quality.

use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::info;
use chrono::{DateTime, Utc};
use rand::Rng;

/// A/B test configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABTestConfig {
    pub test_id: String,
    pub test_name: String,
    pub description: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub variants: Vec<VariantConfig>,
    pub traffic_split: Vec<f32>, // Must sum to 1.0
    pub success_metrics: Vec<SuccessMetric>,
    pub minimum_sample_size: usize,
    pub confidence_level: f32, // e.g., 0.95 for 95%
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantConfig {
    pub variant_id: String,
    pub variant_name: String,
    pub description: String,
    pub configuration: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuccessMetric {
    SkillRecallRate,
    IntentAccuracy,
    ResponseRelevance,
    UserSatisfaction,
    LatencyMs,
    TaskCompletionRate,
    ErrorRate,
    Custom(String),
}

/// A/B test manager
pub struct ABTestManager {
    /// Active tests
    active_tests: Arc<RwLock<HashMap<String, ABTest>>>,
    /// Test results storage
    results_store: Arc<RwLock<HashMap<String, TestResults>>>,
    /// User assignments (user_id -> test_id -> variant_id)
    user_assignments: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
}

/// Running A/B test
#[derive(Debug, Clone)]
struct ABTest {
    config: ABTestConfig,
    status: TestStatus,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
enum TestStatus {
    Running,
    Paused,
    Completed,
    Cancelled,
}

/// Test results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResults {
    pub test_id: String,
    pub test_name: String,
    pub status: TestResultStatus,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub variant_results: HashMap<String, VariantResults>,
    pub winner: Option<String>,
    pub confidence: f32,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestResultStatus {
    InProgress,
    SignificantResult,
    Inconclusive,
    InsufficientData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantResults {
    pub variant_id: String,
    pub variant_name: String,
    pub sample_size: usize,
    pub metrics: HashMap<String, MetricStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricStats {
    pub metric_name: String,
    pub mean: f32,
    pub std_dev: f32,
    pub min: f32,
    pub max: f32,
    pub count: usize,
    pub confidence_interval: (f32, f32),
}

/// Test assignment for a user
#[derive(Debug, Clone)]
pub struct TestAssignment {
    pub test_id: String,
    pub variant_id: String,
    pub variant_config: VariantConfig,
}

impl ABTestManager {
    /// Create a new A/B test manager
    pub fn new() -> Self {
        Self {
            active_tests: Arc::new(RwLock::new(HashMap::new())),
            results_store: Arc::new(RwLock::new(HashMap::new())),
            user_assignments: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new A/B test
    pub async fn create_test(&self, config: ABTestConfig) -> Result<String> {
        // Validate traffic split
        let total_split: f32 = config.traffic_split.iter().sum();
        if (total_split - 1.0).abs() > 0.001 {
            anyhow::bail!("Traffic split must sum to 1.0, got {}", total_split);
        }

        // Validate variants match traffic split
        if config.variants.len() != config.traffic_split.len() {
            anyhow::bail!("Number of variants must match traffic split length");
        }

        let test = ABTest {
            config: config.clone(),
            status: TestStatus::Running,
            created_at: Utc::now(),
        };

        let mut tests = self.active_tests.write().await;
        tests.insert(config.test_id.clone(), test);

        info!("Created A/B test: {} ({})", config.test_name, config.test_id);
        Ok(config.test_id)
    }

    /// Assign user to a variant
    pub async fn assign_user(&self, user_id: &str, test_id: &str) -> Option<TestAssignment> {
        let tests = self.active_tests.read().await;
        let test = tests.get(test_id)?;

        if test.status != TestStatus::Running {
            return None;
        }

        // Check if user already assigned
        {
            let assignments = self.user_assignments.read().await;
            if let Some(user_tests) = assignments.get(user_id) {
                if let Some(variant_id) = user_tests.get(test_id) {
                    return self.create_assignment(test_id, variant_id).await;
                }
            }
        }

        // Assign to variant based on traffic split
        let variant_id = self.select_variant(&test.config).await;

        // Store assignment
        {
            let mut assignments = self.user_assignments.write().await;
            let user_tests = assignments.entry(user_id.to_string()).or_default();
            user_tests.insert(test_id.to_string(), variant_id.clone());
        }

        self.create_assignment(test_id, &variant_id).await
    }

    /// Select variant based on traffic split
    async fn select_variant(&self, config: &ABTestConfig) -> String {
        let mut rng = rand::thread_rng();
        let random_value: f32 = rng.gen();

        let mut cumulative = 0.0;
        for (i, &split) in config.traffic_split.iter().enumerate() {
            cumulative += split;
            if random_value <= cumulative {
                return config.variants[i].variant_id.clone();
            }
        }

        // Fallback to last variant
        config.variants.last().unwrap().variant_id.clone()
    }

    /// Create test assignment
    async fn create_assignment(&self, test_id: &str, variant_id: &str) -> Option<TestAssignment> {
        let tests = self.active_tests.read().await;
        let test = tests.get(test_id)?;

        let variant = test.config.variants.iter()
            .find(|v| v.variant_id == variant_id)?;

        Some(TestAssignment {
            test_id: test_id.to_string(),
            variant_id: variant_id.to_string(),
            variant_config: variant.clone(),
        })
    }

    /// Record metric for a variant
    pub async fn record_metric(
        &self,
        test_id: &str,
        variant_id: &str,
        metric_name: &str,
        value: f32,
    ) -> Result<()> {
        let mut results = self.results_store.write().await;
        
        let test_results = results.entry(test_id.to_string()).or_insert_with(|| {
            TestResults {
                test_id: test_id.to_string(),
                test_name: String::new(),
                status: TestResultStatus::InProgress,
                start_time: Utc::now(),
                end_time: None,
                variant_results: HashMap::new(),
                winner: None,
                confidence: 0.0,
                recommendation: String::new(),
            }
        });

        let variant_results = test_results.variant_results
            .entry(variant_id.to_string())
            .or_insert_with(|| VariantResults {
                variant_id: variant_id.to_string(),
                variant_name: String::new(),
                sample_size: 0,
                metrics: HashMap::new(),
            });

        let metric_stats = variant_results.metrics
            .entry(metric_name.to_string())
            .or_insert_with(|| MetricStats {
                metric_name: metric_name.to_string(),
                mean: 0.0,
                std_dev: 0.0,
                min: value,
                max: value,
                count: 0,
                confidence_interval: (0.0, 0.0),
            });

        // Update running statistics
        let n = metric_stats.count as f32;
        let new_mean = (metric_stats.mean * n + value) / (n + 1.0);
        let new_variance = ((metric_stats.count as f32 * metric_stats.std_dev.powi(2)) + 
            (value - metric_stats.mean) * (value - new_mean)) / (n + 1.0);

        metric_stats.mean = new_mean;
        metric_stats.std_dev = new_variance.sqrt();
        metric_stats.min = metric_stats.min.min(value);
        metric_stats.max = metric_stats.max.max(value);
        metric_stats.count += 1;
        variant_results.sample_size += 1;

        Ok(())
    }

    /// Analyze test results
    pub async fn analyze_test(&self, test_id: &str) -> Result<TestResults> {
        let tests = self.active_tests.read().await;
        let test = tests.get(test_id)
            .ok_or_else(|| anyhow::anyhow!("Test not found: {}", test_id))?;

        let results = self.results_store.read().await;
        let mut test_results = results.get(test_id)
            .cloned()
            .unwrap_or_else(|| TestResults {
                test_id: test_id.to_string(),
                test_name: test.config.test_name.clone(),
                status: TestResultStatus::InProgress,
                start_time: test.config.start_time,
                end_time: None,
                variant_results: HashMap::new(),
                winner: None,
                confidence: 0.0,
                recommendation: "Test is still running".to_string(),
            });

        // Check if we have enough data
        let total_samples: usize = test_results.variant_results.values()
            .map(|v| v.sample_size)
            .sum();

        if total_samples < test.config.minimum_sample_size {
            test_results.status = TestResultStatus::InsufficientData;
            test_results.recommendation = format!(
                "Need {} more samples (current: {})",
                test.config.minimum_sample_size - total_samples,
                total_samples
            );
            return Ok(test_results);
        }

        // Calculate confidence intervals and determine winner
        let winner = self.determine_winner(&test_results, &test.config).await;
        test_results.winner = winner.clone();

        if let Some(ref winner_id) = winner {
            test_results.status = TestResultStatus::SignificantResult;
            test_results.confidence = test.config.confidence_level;
            test_results.recommendation = format!(
                "Variant '{}' shows significant improvement. Recommend rolling out to 100% traffic.",
                winner_id
            );
        } else {
            test_results.status = TestResultStatus::Inconclusive;
            test_results.recommendation = "No significant difference between variants. Consider running longer or testing different variations.".to_string();
        }

        Ok(test_results)
    }

    /// Determine winning variant
    async fn determine_winner(&self, results: &TestResults, config: &ABTestConfig) -> Option<String> {
        // Simple implementation: find variant with best primary metric
        // In production, use proper statistical tests (t-test, chi-square, etc.)
        
        let primary_metric = match config.success_metrics.first() {
            Some(m) => format!("{:?}", m),
            None => return None,
        };

        let mut best_variant: Option<(String, f32)> = None;

        for (variant_id, variant_results) in &results.variant_results {
            if let Some(metric) = variant_results.metrics.get(&primary_metric) {
                let current_best = best_variant.as_ref().map(|(_, best)| *best);
                let is_better = match config.success_metrics.first() {
                    Some(SuccessMetric::LatencyMs) | Some(SuccessMetric::ErrorRate) => {
                        // Lower is better
                        current_best.map(|best| metric.mean < best).unwrap_or(true)
                    }
                    _ => {
                        // Higher is better
                        current_best.map(|best| metric.mean > best).unwrap_or(true)
                    }
                };

                if is_better {
                    best_variant = Some((variant_id.clone(), metric.mean));
                }
            }
        }

        best_variant.map(|(id, _)| id)
    }

    /// Get active tests
    pub async fn get_active_tests(&self) -> Vec<ABTestConfig> {
        let tests = self.active_tests.read().await;
        tests.values()
            .filter(|t| t.status == TestStatus::Running)
            .map(|t| t.config.clone())
            .collect()
    }

    /// Get test results
    pub async fn get_test_results(&self, test_id: &str) -> Option<TestResults> {
        let results = self.results_store.read().await;
        results.get(test_id).cloned()
    }

    /// Stop a test
    pub async fn stop_test(&self, test_id: &str) -> Result<()> {
        let mut tests = self.active_tests.write().await;
        if let Some(test) = tests.get_mut(test_id) {
            test.status = TestStatus::Completed;
            
            let mut results = self.results_store.write().await;
            if let Some(result) = results.get_mut(test_id) {
                result.end_time = Some(Utc::now());
            }
            
            info!("Stopped A/B test: {}", test_id);
        }
        Ok(())
    }

    /// Export all test data
    pub async fn export_test_data(&self) -> Result<serde_json::Value> {
        let results = self.results_store.read().await;
        let tests = self.active_tests.read().await;

        let export = serde_json::json!({
            "tests": tests.values().map(|t| &t.config).collect::<Vec<_>>(),
            "results": results.values().collect::<Vec<_>>(),
            "export_time": Utc::now(),
        });

        Ok(export)
    }
}

impl Default for ABTestManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Predefined test configurations for common optimizations
pub struct TestPresets;

impl TestPresets {
    /// Test for semantic vs keyword skill matching
    pub fn semantic_vs_keyword_test() -> ABTestConfig {
        ABTestConfig {
            test_id: uuid::Uuid::new_v4().to_string(),
            test_name: "Semantic vs Keyword Skill Matching".to_string(),
            description: "Compare semantic matching against keyword-only matching for skill recall".to_string(),
            start_time: Utc::now(),
            end_time: None,
            variants: vec![
                VariantConfig {
                    variant_id: "control".to_string(),
                    variant_name: "Keyword Only".to_string(),
                    description: "Use only keyword matching".to_string(),
                    configuration: serde_json::json!({
                        "matching_strategy": "keyword_only",
                        "threshold": 0.5
                    }),
                },
                VariantConfig {
                    variant_id: "semantic".to_string(),
                    variant_name: "Semantic Matching".to_string(),
                    description: "Use semantic embedding matching".to_string(),
                    configuration: serde_json::json!({
                        "matching_strategy": "semantic",
                        "threshold": 0.65
                    }),
                },
                VariantConfig {
                    variant_id: "hybrid".to_string(),
                    variant_name: "Hybrid RRF".to_string(),
                    description: "Use RRF fusion of keyword and semantic".to_string(),
                    configuration: serde_json::json!({
                        "matching_strategy": "hybrid_rrf",
                        "threshold": 0.55
                    }),
                },
            ],
            traffic_split: vec![0.33, 0.33, 0.34],
            success_metrics: vec![
                SuccessMetric::SkillRecallRate,
                SuccessMetric::IntentAccuracy,
                SuccessMetric::UserSatisfaction,
            ],
            minimum_sample_size: 100,
            confidence_level: 0.95,
        }
    }

    /// Test for routing threshold optimization
    pub fn routing_threshold_test() -> ABTestConfig {
        ABTestConfig {
            test_id: uuid::Uuid::new_v4().to_string(),
            test_name: "Routing Threshold Optimization".to_string(),
            description: "Find optimal confidence threshold for automatic skill execution".to_string(),
            start_time: Utc::now(),
            end_time: None,
            variants: vec![
                VariantConfig {
                    variant_id: "low".to_string(),
                    variant_name: "Low Threshold (0.7)".to_string(),
                    description: "More aggressive auto-execution".to_string(),
                    configuration: serde_json::json!({ "threshold": 0.7 }),
                },
                VariantConfig {
                    variant_id: "medium".to_string(),
                    variant_name: "Medium Threshold (0.8)".to_string(),
                    description: "Balanced approach".to_string(),
                    configuration: serde_json::json!({ "threshold": 0.8 }),
                },
                VariantConfig {
                    variant_id: "high".to_string(),
                    variant_name: "High Threshold (0.9)".to_string(),
                    description: "Conservative auto-execution".to_string(),
                    configuration: serde_json::json!({ "threshold": 0.9 }),
                },
            ],
            traffic_split: vec![0.33, 0.33, 0.34],
            success_metrics: vec![
                SuccessMetric::TaskCompletionRate,
                SuccessMetric::UserSatisfaction,
                SuccessMetric::ErrorRate,
            ],
            minimum_sample_size: 200,
            confidence_level: 0.95,
        }
    }

    /// Test for answer validation
    pub fn answer_validation_test() -> ABTestConfig {
        ABTestConfig {
            test_id: uuid::Uuid::new_v4().to_string(),
            test_name: "Answer Validation Impact".to_string(),
            description: "Measure impact of answer validation on response quality".to_string(),
            start_time: Utc::now(),
            end_time: None,
            variants: vec![
                VariantConfig {
                    variant_id: "no_validation".to_string(),
                    variant_name: "No Validation".to_string(),
                    description: "Generate answers without validation".to_string(),
                    configuration: serde_json::json!({ "validation_enabled": false }),
                },
                VariantConfig {
                    variant_id: "with_validation".to_string(),
                    variant_name: "With Validation".to_string(),
                    description: "Validate and improve answers".to_string(),
                    configuration: serde_json::json!({ 
                        "validation_enabled": true,
                        "max_iterations": 3
                    }),
                },
            ],
            traffic_split: vec![0.5, 0.5],
            success_metrics: vec![
                SuccessMetric::ResponseRelevance,
                SuccessMetric::UserSatisfaction,
                SuccessMetric::LatencyMs,
            ],
            minimum_sample_size: 150,
            confidence_level: 0.95,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_test() {
        let manager = ABTestManager::new();
        let config = TestPresets::semantic_vs_keyword_test();
        
        let test_id = manager.create_test(config).await.unwrap();
        assert!(!test_id.is_empty());
        
        let active_tests = manager.get_active_tests().await;
        assert_eq!(active_tests.len(), 1);
    }

    #[tokio::test]
    async fn test_user_assignment() {
        let manager = ABTestManager::new();
        let config = TestPresets::routing_threshold_test();
        let test_id = manager.create_test(config).await.unwrap();
        
        let assignment = manager.assign_user("user-1", &test_id).await;
        assert!(assignment.is_some());
        
        // Same user should get same assignment
        let assignment2 = manager.assign_user("user-1", &test_id).await;
        assert_eq!(assignment.unwrap().variant_id, assignment2.unwrap().variant_id);
    }

    #[tokio::test]
    async fn test_record_metric() {
        let manager = ABTestManager::new();
        let config = TestPresets::semantic_vs_keyword_test();
        let test_id = manager.create_test(config).await.unwrap();
        
        manager.record_metric(&test_id, "control", "SkillRecallRate", 0.75).await.unwrap();
        manager.record_metric(&test_id, "control", "SkillRecallRate", 0.80).await.unwrap();
        
        let results = manager.get_test_results(&test_id).await;
        assert!(results.is_some());
        
        let variant_results = results.unwrap().variant_results.get("control").cloned();
        assert!(variant_results.is_some());
        assert_eq!(variant_results.unwrap().sample_size, 2);
    }

    #[test]
    fn test_preset_configs() {
        let semantic_test = TestPresets::semantic_vs_keyword_test();
        assert_eq!(semantic_test.variants.len(), 3);
        
        let routing_test = TestPresets::routing_threshold_test();
        assert_eq!(routing_test.variants.len(), 3);
        
        let validation_test = TestPresets::answer_validation_test();
        assert_eq!(validation_test.variants.len(), 2);
    }
}
