//! Adaptive Skill Orchestrator
//!
//! Implements intelligent skill selection based on context and historical performance.
//! Uses multi-armed bandit approach with Thompson Sampling for exploration/exploitation balance.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tracing::info;
use rand::prelude::*;
use rand_distr::{Beta, Distribution};

use super::{SkillManifest, SkillRegistry};

/// Context features for skill selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSelectionContext {
    /// User query text
    pub query: String,
    /// Detected intent category
    pub intent: Option<String>,
    /// Complexity score (0.0 - 1.0)
    pub complexity: f32,
    /// Domain tags detected
    pub domain_tags: Vec<String>,
    /// Previous skills used in this session
    pub recent_skills: Vec<String>,
    /// Session success rate
    pub session_success_rate: f32,
}

impl Default for SkillSelectionContext {
    fn default() -> Self {
        Self {
            query: String::new(),
            intent: None,
            complexity: 0.5,
            domain_tags: vec![],
            recent_skills: vec![],
            session_success_rate: 0.8,
        }
    }
}

/// Performance record for a skill
#[derive(Debug, Clone)]
pub struct SkillPerformance {
    /// Total successful executions
    successes: u32,
    /// Total failed executions
    failures: u32,
    /// Average execution time in ms
    avg_latency_ms: f64,
    /// Last execution timestamp
    last_used: Option<i64>,
}

impl SkillPerformance {
    fn new() -> Self {
        Self {
            successes: 0,
            failures: 0,
            avg_latency_ms: 0.0,
            last_used: None,
        }
    }

    fn success_rate(&self) -> f64 {
        let total = self.successes + self.failures;
        if total == 0 { 0.5 } else { self.successes as f64 / total as f64 }
    }

    fn sample_thompson(&self) -> f64 {
        let alpha = (self.successes as f64).max(1.0);
        let beta = (self.failures as f64).max(1.0);
        let dist = Beta::new(alpha, beta).unwrap();
        let mut rng = rand::thread_rng();
        dist.sample(&mut rng)
    }
}

/// Skill selection candidate with score
#[derive(Debug, Clone)]
pub struct SkillCandidate {
    pub skill_name: String,
    pub manifest: SkillManifest,
    pub selection_score: f64,
    pub source: SkillSelectionSource,
}

#[derive(Debug, Clone, Copy)]
pub enum SkillSelectionSource {
    /// Selected based on historical performance
    ThompsonSampling,
    /// Selected based on intent/domain match
    IntentMatch,
    /// Selected for exploration (random)
    Exploration,
    /// Fallback selection
    Fallback,
}

/// Statistics for skill selection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSelectionStats {
    pub total_selections: u64,
    pub thompson_selections: u64,
    pub intent_selections: u64,
    pub exploration_selections: u64,
    pub fallback_selections: u64,
    pub exploration_rate: f64,
}

impl Default for SkillSelectionStats {
    fn default() -> Self {
        Self {
            total_selections: 0,
            thompson_selections: 0,
            intent_selections: 0,
            exploration_selections: 0,
            fallback_selections: 0,
            exploration_rate: 0.1,
        }
    }
}

/// Configuration for adaptive orchestrator
#[derive(Debug, Clone)]
pub struct AdaptiveOrchestratorConfig {
    /// Initial exploration rate (epsilon)
    pub exploration_rate: f64,
    /// Minimum exploration rate
    pub min_exploration_rate: f64,
    /// Decay rate for exploration
    pub exploration_decay: f64,
    /// Number of top candidates to consider
    pub top_k: usize,
    /// Enable intent-based matching
    pub enable_intent_matching: bool,
    /// Enable Thompson Sampling
    pub enable_thompson_sampling: bool,
}

impl Default for AdaptiveOrchestratorConfig {
    fn default() -> Self {
        Self {
            exploration_rate: 0.1,
            min_exploration_rate: 0.02,
            exploration_decay: 0.995,
            top_k: 5,
            enable_intent_matching: true,
            enable_thompson_sampling: true,
        }
    }
}

/// Adaptive Skill Orchestrator
pub struct AdaptiveSkillOrchestrator {
    config: AdaptiveOrchestratorConfig,
    performance: Arc<RwLock<HashMap<String, SkillPerformance>>>,
    stats: Arc<RwLock<SkillSelectionStats>>,
    intent_index: Arc<RwLock<IntentIndex>>,
}

struct IntentIndex {
    /// intent -> skill names mapping
    intent_skills: HashMap<String, Vec<String>>,
    /// domain -> skill names mapping
    domain_skills: HashMap<String, Vec<String>>,
}

impl Default for IntentIndex {
    fn default() -> Self {
        Self {
            intent_skills: HashMap::new(),
            domain_skills: HashMap::new(),
        }
    }
}

impl AdaptiveSkillOrchestrator {
    /// Create a new adaptive orchestrator
    pub fn new(config: AdaptiveOrchestratorConfig) -> Self {
        Self {
            config,
            performance: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(SkillSelectionStats::default())),
            intent_index: Arc::new(RwLock::new(IntentIndex::default())),
        }
    }

    /// Create with default configuration
    pub fn default_config() -> Self {
        Self::new(AdaptiveOrchestratorConfig::default())
    }

    /// Initialize the orchestrator with skills from a registry
    pub async fn initialize(&self, registry: &SkillRegistry) {
        let mut intent_index = self.intent_index.write().await;
        let manifests = registry.list_skills();
        let skill_count = manifests.len();

        for manifest in manifests {
            // Index by description keywords for intent matching
            let desc_lower = manifest.description.to_lowercase();
            let name_lower = manifest.name.to_lowercase();

            // Simple keyword-based intent detection
            let intent_keywords = [
                ("code", vec!["code", "编程", "programming", "写代码", "debug"]),
                ("data", vec!["data", "数据", "分析", "analytics", "统计"]),
                ("file", vec!["file", "文件", "read", "write", "读取", "写入"]),
                ("search", vec!["search", "搜索", "find", "查找", "query"]),
                ("web", vec!["web", "网页", "http", "fetch", "url"]),
                ("image", vec!["image", "图像", "图片", "生成图片", "画"]),
                ("document", vec!["document", "文档", "pdf", "doc", "报告"]),
            ];

            for (intent, keywords) in intent_keywords {
                for keyword in keywords {
                    if desc_lower.contains(keyword) || name_lower.contains(keyword) {
                        intent_index
                            .intent_skills
                            .entry(intent.to_string())
                            .or_default()
                            .push(manifest.name.clone());
                        break;
                    }
                }
            }

            // Initialize performance tracking
            {
                let mut perf = self.performance.write().await;
                perf.insert(manifest.name.clone(), SkillPerformance::new());
            }
        }

        info!("Adaptive orchestrator initialized with {} skills", skill_count);
    }

    /// Select the best skills for the given context
    pub async fn select_skills(
        &self,
        context: &SkillSelectionContext,
        available_skills: &[SkillManifest],
    ) -> Vec<SkillCandidate> {
        let mut rng = rand::thread_rng();
        let exploration_rate = {
            let stats = self.stats.read().await;
            stats.exploration_rate
        };

        let mut candidates: Vec<SkillCandidate> = Vec::new();

        // Exploration: select random skill
        if rng.gen::<f64>() < exploration_rate {
            let idx = rng.gen_range(0..available_skills.len().max(1));
            if let Some(manifest) = available_skills.get(idx) {
                candidates.push(SkillCandidate {
                    skill_name: manifest.name.clone(),
                    manifest: manifest.clone(),
                    selection_score: 0.0,
                    source: SkillSelectionSource::Exploration,
                });
                {
                    let mut stats = self.stats.write().await;
                    stats.exploration_selections += 1;
                    stats.total_selections += 1;
                }
            }
        }

        // Intent-based selection
        if self.config.enable_intent_matching {
            let intent_skills = self.intent_index.read().await;
            let mut intent_scores: Vec<(String, SkillManifest, f64)> = Vec::new();

            for manifest in available_skills {
                let mut score = 0.0;

                // Score based on intent match
                if let Some(ref intent) = context.intent {
                    if let Some(skills) = intent_skills.intent_skills.get(intent) {
                        if skills.contains(&manifest.name) {
                            score += 0.8;
                        }
                    }
                }

                // Score based on domain tags
                for tag in &context.domain_tags {
                    if let Some(skills) = intent_skills.domain_skills.get(tag) {
                        if skills.contains(&manifest.name) {
                            score += 0.5;
                        }
                    }
                }

                // Score based on description matching
                let desc_lower = manifest.description.to_lowercase();
                for tag in &context.domain_tags {
                    if desc_lower.contains(&tag.to_lowercase()) {
                        score += 0.3;
                    }
                }

                // Penalize recently used skills
                if context.recent_skills.contains(&manifest.name) {
                    score *= 0.7;
                }

                if score > 0.0 {
                    intent_scores.push((manifest.name.clone(), manifest.clone(), score));
                }
            }

            // Sort by score and take top K
            intent_scores.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
            for (name, manifest, score) in intent_scores.into_iter().take(self.config.top_k) {
                candidates.push(SkillCandidate {
                    skill_name: name,
                    manifest,
                    selection_score: score,
                    source: SkillSelectionSource::IntentMatch,
                });
            }

            if candidates.len() > 1 {
                let mut stats = self.stats.write().await;
                stats.intent_selections += 1;
                stats.total_selections += 1;
            }
        }

        // Thompson Sampling for exploitation
        if self.config.enable_thompson_sampling {
            let performance = self.performance.read().await;
            let mut thompson_scores: Vec<(String, SkillManifest, f64)> = Vec::new();

            for manifest in available_skills {
                if let Some(perf) = performance.get(&manifest.name) {
                    let score = perf.sample_thompson();
                    // Boost by success rate
                    let adjusted_score = score * (0.5 + 0.5 * perf.success_rate() as f64);
                    thompson_scores.push((manifest.name.clone(), manifest.clone(), adjusted_score));
                }
            }

            // Sort and take top K
            thompson_scores.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
            for (name, manifest, score) in thompson_scores.into_iter().take(self.config.top_k) {
                // Avoid duplicates
                if !candidates.iter().any(|c| c.skill_name == name) {
                    candidates.push(SkillCandidate {
                        skill_name: name,
                        manifest,
                        selection_score: score,
                        source: SkillSelectionSource::ThompsonSampling,
                    });
                }
            }

            let mut stats = self.stats.write().await;
            stats.thompson_selections += 1;
            stats.total_selections += 1;
        }

        // Fallback: if no candidates, use all available skills
        if candidates.is_empty() {
            for manifest in available_skills.iter().take(self.config.top_k) {
                candidates.push(SkillCandidate {
                    skill_name: manifest.name.clone(),
                    manifest: manifest.clone(),
                    selection_score: 0.5,
                    source: SkillSelectionSource::Fallback,
                });
            }
            let mut stats = self.stats.write().await;
            stats.fallback_selections += 1;
            stats.total_selections += 1;
        }

        // Sort final candidates by score
        candidates.sort_by(|a, b| b.selection_score.partial_cmp(&a.selection_score).unwrap_or(std::cmp::Ordering::Equal));

        candidates
    }

    /// Record the result of a skill execution
    pub async fn record_execution(
        &self,
        skill_name: &str,
        success: bool,
        latency_ms: f64,
    ) {
        let mut performance = self.performance.write().await;
        let perf = performance.entry(skill_name.to_string()).or_insert_with(SkillPerformance::new);

        if success {
            perf.successes += 1;
        } else {
            perf.failures += 1;
        }

        // Update average latency
        if perf.avg_latency_ms == 0.0 {
            perf.avg_latency_ms = latency_ms;
        } else {
            perf.avg_latency_ms = 0.9 * perf.avg_latency_ms + 0.1 * latency_ms;
        }

        perf.last_used = Some(chrono::Utc::now().timestamp());

        // Decay exploration rate
        let mut stats = self.stats.write().await;
        stats.exploration_rate = (stats.exploration_rate * self.config.exploration_decay)
            .max(self.config.min_exploration_rate);
    }

    /// Get current statistics
    pub async fn get_stats(&self) -> SkillSelectionStats {
        self.stats.read().await.clone()
    }

    /// Get performance for a specific skill
    pub async fn get_skill_performance(&self, skill_name: &str) -> Option<SkillPerformance> {
        self.performance.read().await.get(skill_name).cloned()
    }

    /// Export performance data for analysis
    pub async fn export_performance_data(&self) -> HashMap<String, serde_json::Value> {
        let performance = self.performance.read().await;
        let mut data = HashMap::new();

        for (name, perf) in performance.iter() {
            data.insert(name.clone(), serde_json::json!({
                "successes": perf.successes,
                "failures": perf.failures,
                "success_rate": perf.success_rate(),
                "avg_latency_ms": perf.avg_latency_ms,
                "last_used": perf.last_used,
            }));
        }

        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_adaptive_selection() {
        let orchestrator = AdaptiveSkillOrchestrator::default_config();
        let context = SkillSelectionContext {
            query: "帮我分析销售数据".to_string(),
            intent: Some("data".to_string()),
            complexity: 0.7,
            domain_tags: vec!["数据分析".to_string()],
            recent_skills: vec![],
            session_success_rate: 0.8,
        };

        let skills = vec![
            SkillManifest {
                name: "data_analyzer".to_string(),
                description: "Analyzes data and generates reports".to_string(),
                version: "1.0.0".to_string(),
                parameters: serde_json::json!({}),
                entrypoint: "python analyze.py".to_string(),
                env: Default::default(),
                requires: vec![],
                runtime: Some("python".to_string()),
                dependencies: None,
                resources: None,
                permissions: vec![],
                conflicts: vec![],
                min_crablet_version: None,
                author: None,
                triggers: vec![],
            },
            SkillManifest {
                name: "image_generator".to_string(),
                description: "Generates images from text prompts".to_string(),
                version: "1.0.0".to_string(),
                parameters: serde_json::json!({}),
                entrypoint: "python generate.py".to_string(),
                env: Default::default(),
                requires: vec![],
                runtime: Some("python".to_string()),
                dependencies: None,
                resources: None,
                permissions: vec![],
                conflicts: vec![],
                min_crablet_version: None,
                author: None,
                triggers: vec![],
            },
        ];

        let candidates = orchestrator.select_skills(&context, &skills).await;
        assert!(!candidates.is_empty());
    }

    #[tokio::test]
    async fn test_record_execution() {
        let orchestrator = AdaptiveSkillOrchestrator::default_config();
        orchestrator.record_execution("test_skill", true, 150.0).await;
        orchestrator.record_execution("test_skill", false, 200.0).await;

        let perf = orchestrator.get_skill_performance("test_skill").await;
        assert!(perf.is_some());
        let perf = perf.unwrap();
        assert_eq!(perf.successes, 1);
        assert_eq!(perf.failures, 1);
    }
}