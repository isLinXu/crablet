//! RL-based Adaptive Cognitive Router
//!
//! Implements Q-learning based routing between System 1/2/3/4 layers.
//! Learns optimal routing decisions based on task complexity features.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

// ============================================================================
// Task Feature Encoding
// ============================================================================

/// Encoded task features for Q-learning input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskFeatures {
    pub complexity: f32,
    pub urgency: f32,
    pub compute_cost: f32,
    pub memory_req: f32,
    pub tool_use_prob: f32,
    pub multimodal: f32,
    pub iteration_est: f32,
}

impl TaskFeatures {
    pub fn from_task(metadata: &TaskMetadata) -> Self {
        Self {
            complexity: normalize(metadata.token_count, 0, 128000),
            urgency: normalize(metadata.priority as u32, 1, 5),
            compute_cost: estimate_compute_cost(metadata),
            memory_req: normalize(metadata.context_size, 0, 100000),
            tool_use_prob: if metadata.requires_tools { 0.8 } else { 0.2 },
            multimodal: if metadata.has_images { 0.9 } else { 0.1 },
            iteration_est: estimate_iterations(metadata),
        }
    }

    pub fn to_vector(&self) -> Vec<f32> {
        vec![
            self.complexity,
            self.urgency,
            self.compute_cost,
            self.memory_req,
            self.tool_use_prob,
            self.multimodal,
            self.iteration_est,
        ]
    }
}

fn normalize(value: u32, min: u32, max: u32) -> f32 {
    if max <= min { return 0.5; }
    ((value - min) as f32 / (max - min) as f32).clamp(0.0, 1.0)
}

fn estimate_compute_cost(metadata: &TaskMetadata) -> f32 {
    let base = normalize(metadata.token_count, 0, 128000);
    let multiplier = if metadata.requires_tools { 2.0 } else { 1.0 };
    (base * multiplier).min(1.0)
}

fn estimate_iterations(metadata: &TaskMetadata) -> f32 {
    match metadata.task_type {
        TaskType::Simple => 0.1,
        TaskType::Complex => 0.5,
        TaskType::Creative => 0.8,
        TaskType::Code => 0.6,
        TaskType::Analysis => 0.4,
    }
}

// ============================================================================
// Task Metadata
// ============================================================================

#[derive(Debug, Clone)]
pub struct TaskMetadata {
    pub token_count: u32,
    pub context_size: u32,
    pub priority: u32,
    pub requires_tools: bool,
    pub has_images: bool,
    pub task_type: TaskType,
    pub deadline: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskType {
    Simple, Complex, Creative, Code, Analysis,
}

// ============================================================================
// Q-Learning Components
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CognitiveAction {
    System1, System2, System3, System4,
}

impl CognitiveAction {
    pub fn all() -> &'static [CognitiveAction] {
        &[CognitiveAction::System1, CognitiveAction::System2, CognitiveAction::System3, CognitiveAction::System4]
    }

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(CognitiveAction::System1),
            1 => Some(CognitiveAction::System2),
            2 => Some(CognitiveAction::System3),
            3 => Some(CognitiveAction::System4),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QValue {
    pub action: CognitiveAction,
    pub value: f32,
    pub confidence: f32,
    pub update_count: u32,
}

impl QValue {
    pub fn new(action: CognitiveAction) -> Self {
        Self {
            action,
            value: 0.5,
            confidence: 0.1,
            update_count: 0,
        }
    }

    pub fn update(&mut self, reward: f32, learning_rate: f32) {
        self.value += learning_rate * (reward - self.value);
        self.update_count += 1;
        let target_confidence = 1.0 - (0.9_f32.powi(self.update_count as i32));
        self.confidence = self.confidence * 0.9 + target_confidence * 0.1;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QTable {
    entries: HashMap<String, Vec<QValue>>,
    learning_rate: f32,
    discount_factor: f32,
    exploration_rate: f32,
}

impl QTable {
    pub fn new(learning_rate: f32, discount_factor: f32, exploration_rate: f32) -> Self {
        let mut qtable = Self {
            entries: HashMap::new(),
            learning_rate,
            discount_factor,
            exploration_rate,
        };
        qtable.add_priors();
        qtable
    }

    fn add_priors(&mut self) {
        // Simple tasks -> System1
        let simple_key = "0400100";
        let qvalues = vec![
            QValue { action: CognitiveAction::System1, value: 0.9, confidence: 0.8, update_count: 10 },
            QValue { action: CognitiveAction::System2, value: 0.4, confidence: 0.5, update_count: 5 },
            QValue { action: CognitiveAction::System3, value: 0.2, confidence: 0.3, update_count: 2 },
            QValue { action: CognitiveAction::System4, value: 0.1, confidence: 0.2, update_count: 1 },
        ];
        self.entries.insert(simple_key.to_string(), qvalues);

        // Complex tasks -> System2/3
        let complex_key = "4233363";
        let qvalues = vec![
            QValue { action: CognitiveAction::System1, value: 0.2, confidence: 0.3, update_count: 2 },
            QValue { action: CognitiveAction::System2, value: 0.8, confidence: 0.7, update_count: 8 },
            QValue { action: CognitiveAction::System3, value: 0.7, confidence: 0.6, update_count: 6 },
            QValue { action: CognitiveAction::System4, value: 0.5, confidence: 0.4, update_count: 3 },
        ];
        self.entries.insert(complex_key.to_string(), qvalues);
    }

    fn discretize_features(&self, features: &TaskFeatures) -> String {
        let q = |v: f32| ((v * 5.0) as u8).min(4);
        format!(
            "{:01}{:01}{:01}{:01}{:01}{:01}{:01}",
            q(features.complexity),
            q(features.urgency),
            q(features.compute_cost),
            q(features.memory_req),
            q(features.tool_use_prob),
            q(features.multimodal),
            q(features.iteration_est)
        )
    }

    pub fn get_or_create(&mut self, features: &TaskFeatures) -> &mut Vec<QValue> {
        let key = self.discretize_features(features);
        if !self.entries.contains_key(&key) {
            let qvalues = CognitiveAction::all().iter().map(|&action| QValue::new(action)).collect();
            self.entries.insert(key.clone(), qvalues);
        }
        self.entries.get_mut(&key).unwrap()
    }

    pub fn select_action(&mut self, features: &TaskFeatures) -> CognitiveAction {
        let exploration_rate = self.exploration_rate;
        let qvalues = self.get_or_create(features);
        if fastrand::f32() < exploration_rate {
            CognitiveAction::from_index(fastrand::usize(..4)).unwrap()
        } else {
            qvalues.iter()
                .max_by(|a, b| {
                    let a_exp = a.value * a.confidence;
                    let b_exp = b.value * b.confidence;
                    a_exp.partial_cmp(&b_exp).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|qv| qv.action)
                .unwrap_or(CognitiveAction::System2)
        }
    }

    pub fn update(&mut self, features: &TaskFeatures, action: CognitiveAction, reward: f32) {
        let learning_rate = self.learning_rate;
        let qvalues = self.get_or_create(features);
        if let Some(qv) = qvalues.iter_mut().find(|qv| qv.action == action) {
            qv.update(reward, learning_rate);
        }
    }
}

impl Default for QTable {
    fn default() -> Self {
        Self::new(0.1, 0.99, 0.3)
    }
}

// ============================================================================
// RL Router
// ============================================================================

pub struct RLCognitiveRouter {
    qtable: Arc<RwLock<QTable>>,
}

impl RLCognitiveRouter {
    pub fn new(qtable: QTable) -> Self {
        Self {
            qtable: Arc::new(RwLock::new(qtable)),
        }
    }

    pub async fn route(&self, metadata: &TaskMetadata) -> CognitiveAction {
        let features = TaskFeatures::from_task(metadata);
        let mut qtable = self.qtable.write().await;
        qtable.select_action(&features)
    }

    pub async fn update(&self, metadata: &TaskMetadata, action: CognitiveAction, reward: f32) {
        let features = TaskFeatures::from_task(metadata);
        let mut qtable = self.qtable.write().await;
        qtable.update(&features, action, reward);
    }
}

impl Default for RLCognitiveRouter {
    fn default() -> Self {
        Self::new(QTable::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qvalue_update() {
        let mut qv = QValue::new(CognitiveAction::System1);
        qv.update(0.8, 0.5);
        assert!(qv.value > 0.5);
    }

    #[tokio::test]
    async fn test_router_basic() {
        let router = RLCognitiveRouter::default();
        let metadata = TaskMetadata {
            token_count: 5000,
            context_size: 10000,
            priority: 3,
            requires_tools: false,
            has_images: false,
            task_type: TaskType::Simple,
            deadline: None,
        };
        let action = router.route(&metadata).await;
        assert!(matches!(action, CognitiveAction::System1 | CognitiveAction::System2));
    }
}