//! RL-based Skill Router
//!
//! Implements a simplified PPO (Proximal Policy Optimization) agent for
//! learning optimal skill routing strategies based on query context.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use tracing::info;
use rand::prelude::*;

/// Feature vector for state representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateVector {
    /// Query embedding features (simplified as counts)
    pub query_length: f32,
    pub has_code_keywords: bool,
    pub has_data_keywords: bool,
    pub has_file_keywords: bool,
    pub has_search_keywords: bool,
    /// Context features
    pub session_history_length: usize,
    pub avg_success_rate: f32,
    /// Skill availability features (one per skill)
    pub skill_features: Vec<bool>,
}

impl Default for StateVector {
    fn default() -> Self {
        Self {
            query_length: 0.0,
            has_code_keywords: false,
            has_data_keywords: false,
            has_file_keywords: false,
            has_search_keywords: false,
            session_history_length: 0,
            avg_success_rate: 0.5,
            skill_features: vec![],
        }
    }
}

/// Action: select which skill to use
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoutingAction {
    pub skill_index: usize,
}

impl RoutingAction {
    pub fn new(skill_index: usize) -> Self {
        Self { skill_index }
    }
}

/// Reward signal for RL training
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardSignal {
    /// Primary reward: task success
    pub success: f32,
    /// Secondary reward: latency penalty
    pub latency_penalty: f32,
    /// Bonus for using underutilized skills
    pub exploration_bonus: f32,
    /// Penalty for repeating failed skills
    pub repeat_penalty: f32,
}

impl RewardSignal {
    pub fn compute(
        success: bool,
        latency_ms: f32,
        skill_use_count: u32,
        was_recently_failed: bool,
    ) -> Self {
        let success_reward = if success { 1.0 } else { -0.5 };
        let latency_penalty = (latency_ms / 1000.0).min(1.0) * -0.1; // Penalize slow responses
        let exploration_bonus = if skill_use_count < 3 { 0.2 } else { 0.0 };
        let repeat_penalty = if was_recently_failed { -0.3 } else { 0.0 };

        Self {
            success: success_reward,
            latency_penalty,
            exploration_bonus,
            repeat_penalty,
        }
    }

    pub fn total(&self) -> f32 {
        self.success + self.latency_penalty + self.exploration_bonus + self.repeat_penalty
    }
}

/// Trajectory for PPO training
#[derive(Debug, Clone)]
struct TrajectoryEntry {
    state: StateVector,
    action: RoutingAction,
    reward: f32,
    log_prob: f32,
    value_estimate: f32,
}

/// Simplified Policy Network
#[derive(Debug, Clone)]
struct PolicyNetwork {
    /// Layer 1 weights: state -> hidden
    w1: Vec<Vec<f32>>,
    /// Layer 1 biases
    b1: Vec<f32>,
    /// Layer 2 weights: hidden -> action
    w2: Vec<Vec<f32>>,
    /// Layer 2 biases
    b2: Vec<f32>,
    /// Number of hidden units
    hidden_size: usize,
    /// Action space size
    action_size: usize,
}

impl PolicyNetwork {
    fn new(state_size: usize, action_size: usize) -> Self {
        let hidden_size = 32.min((state_size + action_size) / 2);
        let mut rng = rand::thread_rng();

        // Xavier initialization
        let scale1 = (2.0 / (state_size + hidden_size) as f32).sqrt();
        let scale2 = (2.0 / (hidden_size + action_size) as f32).sqrt();

        let w1 = (0..hidden_size)
            .map(|_| (0..state_size).map(|_| rng.gen_range(-scale1..scale1)).collect())
            .collect();
        let b1 = (0..hidden_size).map(|_| 0.0).collect();

        let w2 = (0..action_size)
            .map(|_| (0..hidden_size).map(|_| rng.gen_range(-scale2..scale2)).collect())
            .collect();
        let b2 = (0..action_size).map(|_| 0.0).collect();

        Self {
            w1,
            b1,
            w2,
            b2,
            hidden_size,
            action_size,
        }
    }

    /// Forward pass: compute action logits
    fn forward(&self, state: &[f32]) -> Vec<f32> {
        // Hidden layer: ReLU(W1 * state + b1)
        let hidden: Vec<f32> = (0..self.hidden_size)
            .map(|i| {
                let sum = self.w1[i].iter().zip(state.iter()).map(|(w, s)| w * s).sum::<f32>() + self.b1[i];
                sum.max(0.0) // ReLU
            })
            .collect();

        // Output layer: softmax(W2 * hidden + b2)
        let logits: Vec<f32> = (0..self.action_size)
            .map(|i| self.w2[i].iter().zip(hidden.iter()).map(|(w, h)| w * h).sum::<f32>() + self.b2[i])
            .collect();

        // Softmax
        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Vec<f32> = logits.iter().map(|l| (l - max_logit).exp()).collect();
        let sum_exp: f32 = exp_logits.iter().sum();
        exp_logits.iter().map(|e| e / sum_exp).collect()
    }

    /// Sample action from policy
    fn sample_action(&self, probs: &[f32]) -> (RoutingAction, f32, f32) {
        let mut rng = rand::thread_rng();
        let mut cumulative = 0.0;

        // Sample from categorical distribution
        let r: f32 = rng.gen();
        for (i, &p) in probs.iter().enumerate() {
            cumulative += p;
            if r < cumulative {
                let log_prob = p.ln();
                let value_estimate = p; // Simplified: use probability as value estimate
                return (RoutingAction::new(i), log_prob, value_estimate);
            }
        }

        // Fallback: return last action
        (RoutingAction::new(probs.len().saturating_sub(1)), 0.0, 0.5)
    }

    /// Update policy using policy gradient (simplified PPO)
    fn update(&mut self, trajectories: &[TrajectoryEntry], learning_rate: f32) {
        // Simplified update: adjust weights based on reward-weighted gradient
        // In full PPO, this would use clipped surrogate objectives and advantage estimation
        for entry in trajectories {
            let probs = self.forward(&self.state_to_vec(&entry.state));
            let _prob = probs.get(entry.action.skill_index).copied().unwrap_or(0.1);

            // Policy gradient: gradient of log_prob * advantage
            // Simplified: use reward as advantage
            let advantage = entry.reward - entry.value_estimate;

            // Compute gradient step (simplified)
            // In practice, this would involve backpropagation
            let gradient_scale = advantage * learning_rate;

            // Apply gradient to output layer (simplified)
            for i in 0..self.action_size {
                for j in 0..self.hidden_size {
                    let target = if i == entry.action.skill_index { 1.0 } else { 0.0 };
                    let error = target - probs[i];
                    self.w2[i][j] += gradient_scale * error * 0.1;
                }
            }
        }
    }

    fn state_to_vec(&self, state: &StateVector) -> Vec<f32> {
        let mut features = vec![
            state.query_length / 100.0, // Normalize
            if state.has_code_keywords { 1.0 } else { 0.0 },
            if state.has_data_keywords { 1.0 } else { 0.0 },
            if state.has_file_keywords { 1.0 } else { 0.0 },
            if state.has_search_keywords { 1.0 } else { 0.0 },
            (state.session_history_length as f32 / 50.0).min(1.0),
            state.avg_success_rate,
        ];
        features.extend(state.skill_features.iter().map(|&b| if b { 1.0 } else { 0.0 }));
        features
    }
}

/// RL-based Skill Router
pub struct RLSkillRouter {
    policy: Arc<RwLock<PolicyNetwork>>,
    skill_names: Vec<String>,
    skill_usage_counts: Arc<RwLock<HashMap<String, u32>>>,
    recent_failures: Arc<RwLock<HashMap<String, u32>>>,
    trajectories: Arc<RwLock<Vec<TrajectoryEntry>>>,
    config: RLRouterConfig,
}

#[derive(Debug, Clone)]
pub struct RLRouterConfig {
    /// Learning rate for policy updates
    pub learning_rate: f32,
    /// Discount factor for future rewards
    pub gamma: f32,
    /// Clip range for PPO
    pub clip_epsilon: f32,
    /// Batch size for training
    pub batch_size: usize,
    /// Maximum trajectory buffer size
    pub max_trajectory_size: usize,
    /// Enable exploration (epsilon-greedy)
    pub exploration_rate: f32,
}

impl Default for RLRouterConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.001,
            gamma: 0.99,
            clip_epsilon: 0.2,
            batch_size: 32,
            max_trajectory_size: 1000,
            exploration_rate: 0.1,
        }
    }
}

impl RLSkillRouter {
    /// Create a new RL router
    pub fn new(skill_names: Vec<String>, config: RLRouterConfig) -> Self {
        let state_size = 7 + skill_names.len(); // Basic features + skill availability
        let action_size = skill_names.len();

        Self {
            policy: Arc::new(RwLock::new(PolicyNetwork::new(state_size, action_size))),
            skill_names,
            skill_usage_counts: Arc::new(RwLock::new(HashMap::new())),
            recent_failures: Arc::new(RwLock::new(HashMap::new())),
            trajectories: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    /// Create with default configuration
    pub fn with_skills(skill_names: Vec<String>) -> Self {
        Self::new(skill_names, RLRouterConfig::default())
    }

    /// Convert query and context to state vector
    pub fn query_to_state(&self, query: &str, history_length: usize, avg_success_rate: f32) -> StateVector {
        let query_lower = query.to_lowercase();

        let skill_features: Vec<bool> = self.skill_names.iter().map(|_| true).collect();

        StateVector {
            query_length: query.len() as f32,
            has_code_keywords: ["code", "编程", "debug", "function", "函数"].iter().any(|kw| query_lower.contains(kw)),
            has_data_keywords: ["data", "数据", "分析", "统计", "analytics"].iter().any(|kw| query_lower.contains(kw)),
            has_file_keywords: ["file", "文件", "read", "write", "读取", "写入"].iter().any(|kw| query_lower.contains(kw)),
            has_search_keywords: ["search", "搜索", "find", "query", "查找"].iter().any(|kw| query_lower.contains(kw)),
            session_history_length: history_length,
            avg_success_rate,
            skill_features,
        }
    }

    /// Select action (skill) based on current state
    pub async fn select_skill(&self, state: &StateVector) -> RoutingAction {
        let policy = self.policy.read().await;
        let probs = policy.forward(&self.state_to_vec(state));

        let mut rng = rand::thread_rng();

        // Epsilon-greedy exploration
        if rng.gen::<f32>() < self.config.exploration_rate {
            // Random exploration: pick a random skill
            // rand 0.10: gen_range is inclusive on both ends (..=)
            let max_idx = self.skill_names.len().saturating_sub(1);
            let idx = rng.gen_range(0..=max_idx);
            return RoutingAction::new(idx);
        }

        // Exploitation: sample from policy
        let (action, _, _) = policy.sample_action(&probs);
        action
    }

    /// Record execution result and update policy
    pub async fn record_result(&self, state: &StateVector, action: RoutingAction, reward: RewardSignal) {
        let skill_name = self.skill_names.get(action.skill_index).cloned().unwrap_or_default();

        // Update skill usage counts
        {
            let mut counts = self.skill_usage_counts.write().await;
            *counts.entry(skill_name.clone()).or_insert(0) += 1;
        }

        // Update failure tracking
        {
            let mut failures = self.recent_failures.write().await;
            if reward.success < 0.0 {
                *failures.entry(skill_name.clone()).or_insert(0) += 1;
            } else {
                failures.remove(&skill_name);
            }
        }

        // Compute log probability for trajectory
        let policy = self.policy.read().await;
        let probs = policy.forward(&self.state_to_vec(state));
        let log_prob = probs.get(action.skill_index).map(|p| p.ln()).unwrap_or(0.0);
        let value_estimate = probs.get(action.skill_index).copied().unwrap_or(0.5);

        // Add to trajectory buffer
        {
            let mut trajectories = self.trajectories.write().await;
            trajectories.push(TrajectoryEntry {
                state: state.clone(),
                action,
                reward: reward.total(),
                log_prob,
                value_estimate,
            });

            // Trim if too large
            if trajectories.len() > self.config.max_trajectory_size {
                let drain_count = trajectories.len() - self.config.max_trajectory_size;
                trajectories.drain(0..drain_count);
            }
        }

        // Periodic policy update
        let trajectory_count = {
            let trajectories = self.trajectories.read().await;
            trajectories.len()
        };

        if trajectory_count >= self.config.batch_size {
            self.train_policy().await;
        }
    }

    /// Train the policy using collected trajectories
    async fn train_policy(&self) {
        let trajectories = {
            let mut t = self.trajectories.write().await;
            let drain_count = self.config.batch_size.min(t.len());
            let batch: Vec<_> = t.drain(0..drain_count).collect();
            batch
        };

        if trajectories.is_empty() {
            return;
        }

        let policy = self.policy.write().await;
        let _old_probs: Vec<f32> = {
            let state_vec = self.state_to_vec(&trajectories[0].state);
            policy.forward(&state_vec)
        };

        // Simplified PPO update
        // In full PPO, we would compute advantages using GAE and apply clipped surrogate loss
        let mut policy = policy;
        policy.update(&trajectories, self.config.learning_rate);

        info!("RL Router: Updated policy with {} trajectories", trajectories.len());
    }

    fn state_to_vec(&self, state: &StateVector) -> Vec<f32> {
        let mut features = vec![
            state.query_length / 100.0,
            if state.has_code_keywords { 1.0 } else { 0.0 },
            if state.has_data_keywords { 1.0 } else { 0.0 },
            if state.has_file_keywords { 1.0 } else { 0.0 },
            if state.has_search_keywords { 1.0 } else { 0.0 },
            (state.session_history_length as f32 / 50.0).min(1.0),
            state.avg_success_rate,
        ];
        features.extend(state.skill_features.iter().map(|&b| if b { 1.0 } else { 0.0 }));
        features
    }

    /// Get routing statistics
    pub async fn get_stats(&self) -> RouterStats {
        let trajectories = self.trajectories.read().await;
        let usage = self.skill_usage_counts.read().await.clone();

        RouterStats {
            total_selections: usage.values().copied().sum(),
            skill_usage_counts: usage,
            trajectory_buffer_size: trajectories.len(),
            exploration_rate: self.config.exploration_rate,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterStats {
    pub total_selections: u32,
    pub skill_usage_counts: HashMap<String, u32>,
    pub trajectory_buffer_size: usize,
    pub exploration_rate: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rl_router() {
        let skills = vec!["code_skill".to_string(), "data_skill".to_string(), "file_skill".to_string()];
        let router = RLSkillRouter::with_skills(skills);

        let state = router.query_to_state("分析我的销售数据", 5, 0.8);
        let action = router.select_skill(&state).await;

        assert!(action.skill_index < 3);

        let reward = RewardSignal::compute(true, 150.0, 1, false);
        router.record_result(&state, action, reward).await;

        let stats = router.get_stats().await;
        assert_eq!(stats.total_selections, 1);
    }

    #[test]
    fn test_reward_signal() {
        let reward = RewardSignal::compute(true, 100.0, 1, false);
        assert!(reward.success > 0.0);

        let failed_reward = RewardSignal::compute(false, 200.0, 5, true);
        assert!(failed_reward.total() < 0.0);
    }
}