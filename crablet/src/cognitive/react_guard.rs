//! ReAct Execution Guard - Enhanced Loop Prevention and Confidence Monitoring
//! 
//! This module provides advanced protection against infinite loops in ReAct reasoning:
//! - Multi-level loop detection (exact, semantic, resource-based)
//! - Confidence decay monitoring with branch pruning
//! - Step timeout enforcement
//! - Forced summarization fallback

use std::collections::{HashSet, HashMap, VecDeque};
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{warn};
use serde::{Deserialize, Serialize};

/// Configuration for ReAct execution guards
#[derive(Debug, Clone)]
pub struct ReactGuardConfig {
    /// Maximum number of reasoning steps before forced termination
    pub max_steps: usize,
    /// Timeout for each individual step
    pub step_timeout: Duration,
    /// Total execution timeout
    pub total_timeout: Duration,
    /// Confidence threshold for early termination
    pub confidence_threshold: f32,
    /// Number of steps to track for confidence trend
    pub confidence_window: usize,
    /// Threshold for confidence decline rate
    pub confidence_decline_threshold: f32,
}

impl Default for ReactGuardConfig {
    fn default() -> Self {
        Self {
            max_steps: 15,
            step_timeout: Duration::from_secs(10),
            total_timeout: Duration::from_secs(120),
            confidence_threshold: 0.6,
            confidence_window: 3,
            confidence_decline_threshold: 0.15,
        }
    }
}

/// Tracks confidence scores and detects declining trends
#[derive(Debug)]
pub struct ConfidenceTracker {
    window: VecDeque<f32>,
    max_window: usize,
}

impl ConfidenceTracker {
    pub fn new(max_window: usize) -> Self {
        Self {
            window: VecDeque::with_capacity(max_window),
            max_window,
        }
    }
    
    /// Add a confidence score and return true if declining trend detected
    pub fn record(&mut self, confidence: f32) -> ConfidenceTrend {
        self.window.push_back(confidence);
        
        if self.window.len() > self.max_window {
            self.window.pop_front();
        }
        
        // Need at least 3 samples to detect trend
        if self.window.len() < 3 {
            return ConfidenceTrend::Stable;
        }
        
        // Calculate trend using linear regression slope
        let trend = self.calculate_trend();
        
        if trend < -0.15 {
            ConfidenceTrend::Declining(trend)
        } else if trend > 0.15 {
            ConfidenceTrend::Improving(trend)
        } else {
            ConfidenceTrend::Stable
        }
    }
    
    fn calculate_trend(&self) -> f32 {
        let n = self.window.len() as f32;
        let sum_x: f32 = (0..self.window.len()).map(|i| i as f32).sum();
        let sum_y: f32 = self.window.iter().sum();
        let sum_xy: f32 = self.window.iter()
            .enumerate()
            .map(|(i, &y)| i as f32 * y)
            .sum();
        let sum_x2: f32 = (0..self.window.len())
            .map(|i| (i as f32).powi(2))
            .sum();
        
        let denominator = n * sum_x2 - sum_x.powi(2);
        if denominator.abs() < 1e-6 {
            return 0.0;
        }
        
        (n * sum_xy - sum_x * sum_y) / denominator
    }
    
    pub fn average_confidence(&self) -> Option<f32> {
        if self.window.is_empty() {
            return None;
        }
        Some(self.window.iter().sum::<f32>() / self.window.len() as f32)
    }
}

/// Confidence trend direction and magnitude
#[derive(Debug, Clone, PartialEq)]
pub enum ConfidenceTrend {
    Improving(f32),
    Stable,
    Declining(f32),
}

/// Enhanced loop detector with multi-level analysis
#[derive(Debug)]
pub struct EnhancedLoopDetector {
    /// Exact match history (tool_name + args_hash)
    exact_history: HashSet<(String, String)>,
    /// Resource usage tracking (tool_name -> resource_id -> count)
    resource_usage: HashMap<String, HashMap<String, usize>>,
    /// Semantic similarity sliding window
    semantic_window: VecDeque<String>,
    /// Consecutive repeat counter
    consecutive_repeats: usize,
    /// Configuration
    max_window: usize,
    similarity_threshold: f32,
    max_resource_usage: usize,
}

impl EnhancedLoopDetector {
    pub fn new() -> Self {
        Self {
            exact_history: HashSet::new(),
            resource_usage: HashMap::new(),
            semantic_window: VecDeque::new(),
            consecutive_repeats: 0,
            max_window: 5,
            similarity_threshold: 0.85,
            max_resource_usage: 3,
        }
    }
    
    /// Check if action indicates looping behavior
    pub fn is_looping(&mut self, tool: &str, args: &str) -> LoopDetectionResult {
        // Level 1: Exact match detection
        if let Some(repeat_count) = self.check_exact_match(tool, args) {
            if repeat_count >= 2 {
                return LoopDetectionResult::ExactLoop { count: repeat_count };
            }
        }
        
        // Level 2: Resource-level semantic detection (for tools like 'see')
        if let Some(resource_loop) = self.check_resource_loop(tool, args) {
            return LoopDetectionResult::ResourceLoop { 
                resource: resource_loop.0, 
                count: resource_loop.1 
            };
        }
        
        // Level 3: Semantic similarity detection
        if let Some(similarity) = self.check_semantic_loop(tool, args) {
            if similarity > self.similarity_threshold {
                return LoopDetectionResult::SemanticLoop { similarity };
            }
        }
        
        LoopDetectionResult::NoLoop
    }
    
    fn check_exact_match(&mut self, tool: &str, args: &str) -> Option<usize> {
        let key = (tool.to_string(), args.to_string());
        
        if self.exact_history.contains(&key) {
            self.consecutive_repeats += 1;
            Some(self.consecutive_repeats)
        } else {
            self.consecutive_repeats = 0;
            self.exact_history.insert(key);
            None
        }
    }
    
    fn check_resource_loop(&mut self, tool: &str, args: &str) -> Option<(String, usize)> {
        // Try to extract resource identifier from args
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(args) {
            // Common resource fields
            let resource_fields = ["image_path", "file_path", "url", "path", "resource"];
            
            for field in resource_fields {
                if let Some(resource) = val.get(field).and_then(|v| v.as_str()) {
                    let counts = self.resource_usage
                        .entry(tool.to_string())
                        .or_default();
                    
                    let count = counts.entry(resource.to_string()).or_insert(0);
                    *count += 1;
                    
                    if *count > self.max_resource_usage {
                        warn!("Resource loop detected: {} used {} times for {}", 
                              tool, count, resource);
                        return Some((resource.to_string(), *count));
                    }
                }
            }
        }
        
        None
    }
    
    fn check_semantic_loop(&mut self, tool: &str, args: &str) -> Option<f32> {
        let action_str = format!("{} {}", tool, args);
        
        for prev in &self.semantic_window {
            let similarity = self.jaccard_similarity(prev, &action_str);
            if similarity > self.similarity_threshold {
                return Some(similarity);
            }
        }
        
        // Add to window
        if self.semantic_window.len() >= self.max_window {
            self.semantic_window.pop_front();
        }
        self.semantic_window.push_back(action_str);
        
        None
    }
    
    fn jaccard_similarity(&self, s1: &str, s2: &str) -> f32 {
        let set1: HashSet<&str> = s1.split_whitespace().collect();
        let set2: HashSet<&str> = s2.split_whitespace().collect();
        
        let intersection = set1.intersection(&set2).count();
        let union = set1.union(&set2).count();
        
        if union == 0 { 0.0 } else { intersection as f32 / union as f32 }
    }
    
    pub fn reset(&mut self) {
        self.exact_history.clear();
        self.resource_usage.clear();
        self.semantic_window.clear();
        self.consecutive_repeats = 0;
    }
}

impl Default for EnhancedLoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Loop detection result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LoopDetectionResult {
    NoLoop,
    ExactLoop { count: usize },
    ResourceLoop { resource: String, count: usize },
    SemanticLoop { similarity: f32 },
}

/// Execution state for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionState {
    pub current_step: usize,
    pub elapsed_time_ms: u64,
    pub confidence_scores: Vec<f32>,
    pub actions_taken: Vec<String>,
    pub loop_detections: Vec<LoopDetectionResult>,
}

/// Result from forced summarization fallback
#[derive(Debug)]
pub struct ForcedSummary {
    pub summary: String,
    pub reason: TerminationReason,
    pub partial_results: Vec<String>,
    pub confidence: Option<f32>,
}

/// Reason for terminating ReAct execution
#[derive(Debug, Clone, PartialEq)]
pub enum TerminationReason {
    MaxStepsReached,
    Timeout,
    ConfidenceDecay,
    LoopDetected(LoopDetectionResult),
    UserRequest,
    Success,
}

/// Main guard for ReAct execution
pub struct ReActGuard {
    config: ReactGuardConfig,
    start_time: Instant,
    loop_detector: EnhancedLoopDetector,
    confidence_tracker: ConfidenceTracker,
    actions_history: Vec<String>,
    loop_detections: Vec<LoopDetectionResult>,
}

impl ReActGuard {
    pub fn new(config: ReactGuardConfig) -> Self {
        Self {
            config: config.clone(),
            start_time: Instant::now(),
            loop_detector: EnhancedLoopDetector::new(),
            confidence_tracker: ConfidenceTracker::new(config.confidence_window),
            actions_history: Vec::new(),
            loop_detections: Vec::new(),
        }
    }
    
    /// Check if execution should continue
    pub fn should_continue(&mut self, tool: &str, args: &str, confidence: Option<f32>) 
        -> Result<(), TerminationReason> 
    {
        let elapsed = self.start_time.elapsed();
        
        // Check total timeout
        if elapsed > self.config.total_timeout {
            return Err(TerminationReason::Timeout);
        }
        
        // Check loop detection
        let loop_result = self.loop_detector.is_looping(tool, args);
        self.loop_detections.push(loop_result.clone());
        
        if !matches!(loop_result, LoopDetectionResult::NoLoop) {
            return Err(TerminationReason::LoopDetected(loop_result));
        }
        
        // Track confidence if provided
        if let Some(conf) = confidence {
            let trend = self.confidence_tracker.record(conf);
            
            if matches!(trend, ConfidenceTrend::Declining(_)) {
                if let Some(avg) = self.confidence_tracker.average_confidence() {
                    if avg < self.config.confidence_threshold {
                        return Err(TerminationReason::ConfidenceDecay);
                    }
                }
            }
        }
        
        // Record action
        self.actions_history.push(format!("{} {}", tool, args));
        
        Ok(())
    }
    
    /// Check if max steps reached
    pub fn check_step_limit(&self, current_step: usize) -> Result<(), TerminationReason> {
        if current_step >= self.config.max_steps {
            return Err(TerminationReason::MaxStepsReached);
        }
        Ok(())
    }
    
    /// Execute action with timeout protection
    pub async fn execute_with_timeout<F, T>(&self, future: F) -> Result<T, anyhow::Error>
    where
        F: futures::Future<Output = Result<T, anyhow::Error>>,
    {
        match timeout(self.config.step_timeout, future).await {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!(
                "Step timed out after {} seconds", 
                self.config.step_timeout.as_secs()
            )),
        }
    }
    
    /// Get current execution state
    pub fn get_state(&self) -> ExecutionState {
        ExecutionState {
            current_step: self.actions_history.len(),
            elapsed_time_ms: self.start_time.elapsed().as_millis() as u64,
            confidence_scores: vec![], // Not exposing internal tracker state
            actions_taken: self.actions_history.clone(),
            loop_detections: self.loop_detections.clone(),
        }
    }
    
    /// Create forced summary when termination is required
    pub fn create_forced_summary(
        &self,
        partial_observations: &[String],
        reason: TerminationReason,
    ) -> ForcedSummary {
        let summary = match &reason {
            TerminationReason::MaxStepsReached => {
                format!(
                    "After {} reasoning steps, I've gathered the following:\n{}\n\n\
                     Based on this analysis, my best assessment is that the task \
                     requires more information or a different approach.",
                    self.config.max_steps,
                    partial_observations.join("\n")
                )
            }
            TerminationReason::Timeout => {
                format!(
                    "The reasoning process exceeded the time limit of {} seconds.\n\
                     Partial findings:\n{}",
                    self.config.total_timeout.as_secs(),
                    partial_observations.join("\n")
                )
            }
            TerminationReason::ConfidenceDecay => {
                format!(
                    "My confidence in the reasoning path has been decreasing.\n\
                     This suggests I may be pursuing an incorrect approach.\n\
                     Current observations:\n{}",
                    partial_observations.join("\n")
                )
            }
            TerminationReason::LoopDetected(loop_result) => {
                format!(
                    "I detected repetitive reasoning patterns:\n{:?}\n\n\
                     To avoid wasting resources, I'm stopping here with partial results:\n{}",
                    loop_result,
                    partial_observations.join("\n")
                )
            }
            _ => partial_observations.join("\n"),
        };
        
        ForcedSummary {
            summary,
            reason,
            partial_results: partial_observations.to_vec(),
            confidence: self.confidence_tracker.average_confidence(),
        }
    }
    
    /// Reset the guard for reuse
    pub fn reset(&mut self) {
        self.start_time = Instant::now();
        self.loop_detector.reset();
        self.confidence_tracker = ConfidenceTracker::new(self.config.confidence_window);
        self.actions_history.clear();
        self.loop_detections.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_tracker_trend_detection() {
        let mut tracker = ConfidenceTracker::new(5);
        
        // Stable trend
        assert_eq!(tracker.record(0.8), ConfidenceTrend::Stable);
        assert_eq!(tracker.record(0.8), ConfidenceTrend::Stable);
        assert_eq!(tracker.record(0.8), ConfidenceTrend::Stable);
        
        // Declining trend
        let mut tracker2 = ConfidenceTracker::new(5);
        tracker2.record(0.9);
        tracker2.record(0.7);
        tracker2.record(0.5);
        assert!(matches!(tracker2.record(0.3), ConfidenceTrend::Declining(_)));
    }

    #[test]
    fn test_loop_detector_exact_match() {
        let mut detector = EnhancedLoopDetector::new();
        
        // First occurrence - no loop
        assert!(matches!(detector.is_looping("search", "{\"q\": \"test\"}"), 
                         LoopDetectionResult::NoLoop));
        
        // Second occurrence - still allowed
        assert!(matches!(detector.is_looping("search", "{\"q\": \"test\"}"), 
                         LoopDetectionResult::ExactLoop { count: 1 }));
        
        // Third occurrence - loop detected
        assert!(matches!(detector.is_looping("search", "{\"q\": \"test\"}"), 
                         LoopDetectionResult::ExactLoop { count: 2 }));
    }

    #[test]
    fn test_react_guard_step_limit() {
        let config = ReactGuardConfig {
            max_steps: 3,
            ..Default::default()
        };
        let mut guard = ReActGuard::new(config);
        
        assert!(guard.check_step_limit(0).is_ok());
        assert!(guard.check_step_limit(2).is_ok());
        assert!(matches!(guard.check_step_limit(3), Err(TerminationReason::MaxStepsReached)));
    }
}
