//! RL-Based Optimizer - Reinforcement Learning for Strategy Optimization
//!
//! Uses policy gradient methods to learn optimal strategy selection:
//! - PPO (Proximal Policy Optimization) for stable learning
//! - Experience replay buffer
//! - Advantage estimation

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use rand::Rng;

use crate::cognitive::llm::LlmClient;
use crate::types::Message;

// ============================================================================
// Experience & Learning Types
// ============================================================================

/// An experience tuple for replay buffer
#[derive(Debug, Clone)]
pub struct Experience {
    /// Current state (task description or embedding)
    pub state: String,
    /// Action taken (strategy name)
    pub action: String,
    /// Reward received
    pub reward: f64,
    /// Next state
    pub next_state: String,
    /// Whether the episode ended
    pub done: bool,
}

/// A learning episode
#[derive(Debug, Clone)]
pub struct Episode {
    pub experiences: Vec<Experience>,
    pub total_reward: f64,
    pub strategy_used: String,
    pub success: bool,
}

// ============================================================================
// Policy Network (Simplified)
// ============================================================================

/// A simple policy network for strategy selection
#[derive(Debug, Clone)]
pub struct PolicyNetwork {
    /// Strategy scores (before softmax)
    strategy_scores: Vec<f64>,
    /// Learning rate
    learning_rate: f64,
}

impl PolicyNetwork {
    pub fn new(num_strategies: usize, learning_rate: f64) -> Self {
        Self {
            // Initialize with small random values
            strategy_scores: (0..num_strategies)
                .map(|_| rand::thread_rng().gen_range(-0.1..0.1))
                .collect(),
            learning_rate,
        }
    }

    /// Select action using softmax probability
    pub fn select_action(&self) -> usize {
        let scores = &self.strategy_scores;

        // Compute softmax
        let max_score = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exp_scores: Vec<f64> = scores
            .iter()
            .map(|s| (*s - max_score).exp())
            .collect();
        let sum: f64 = exp_scores.iter().sum();
        let probs: Vec<f64> = exp_scores.iter().map(|e| e / sum).collect();

        // Sample from distribution
        let mut rng = rand::thread_rng();
        let r: f64 = rng.gen_range(0.0..1.0);
        let mut cumulative = 0.0;
        for (i, p) in probs.iter().enumerate() {
            cumulative += p;
            if r <= cumulative {
                return i;
            }
        }
        probs.len() - 1
    }

    /// Get probability distribution
    pub fn get_probs(&self) -> Vec<f64> {
        let scores = &self.strategy_scores;
        let max_score = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let exp_scores: Vec<f64> = scores
            .iter()
            .map(|s| (*s - max_score).exp())
            .collect();
        let sum: f64 = exp_scores.iter().sum();
        exp_scores.iter().map(|e| e / sum).collect()
    }

    /// Update policy using REINFORCE with baseline
    pub fn update(&mut self, action: usize, advantage: f64) {
        let probs = self.get_probs();
        let prob_action = probs[action].max(1e-8); // Avoid log(0)

        // Gradient: advantage * grad log pi(a)
        let gradient = advantage / prob_action;

        // Update only the selected action (simplified)
        self.strategy_scores[action] += self.learning_rate * gradient;

        // Normalize to prevent explosion
        let norm = (self.strategy_scores.iter().map(|s| s * s).sum::<f64>()).sqrt();
        if norm > 2.0 {
            for score in &mut self.strategy_scores {
                *score *= 2.0 / norm;
            }
        }
    }

    /// Update using PPO-style clipped objective
    pub fn update_ppo(&mut self, action: usize, old_log_prob: f64, advantage: f64, clip_epsilon: f64) {
        let probs = self.get_probs();
        let new_log_prob = probs[action].ln().max(-20.0); // Clip for stability

        // Ratio
        let ratio = (new_log_prob - old_log_prob).exp();

        // Clipped objective
        let surr1 = ratio * advantage;
        let surr2 = ratio.clamp(1.0 - clip_epsilon, 1.0 + clip_epsilon) * advantage;
        let clipped_advantage = surr1.min(surr2);

        // Gradient update
        let gradient = clipped_advantage;
        self.strategy_scores[action] += self.learning_rate * gradient;

        // Normalize
        let norm = (self.strategy_scores.iter().map(|s| s * s).sum::<f64>()).sqrt();
        if norm > 2.0 {
            for score in &mut self.strategy_scores {
                *score *= 2.0 / norm;
            }
        }
    }
}

// ============================================================================
// Value Network (Baseline)
// ============================================================================

/// Simple value network for baseline estimation
#[derive(Debug, Clone)]
pub struct ValueNetwork {
    /// Cached state values
    state_values: std::collections::HashMap<String, f64>,
    /// Learning rate
    learning_rate: f64,
}

impl ValueNetwork {
    pub fn new(learning_rate: f64) -> Self {
        Self {
            state_values: std::collections::HashMap::new(),
            learning_rate,
        }
    }

    /// Get value estimate for a state
    pub fn get_value(&self, state: &str) -> f64 {
        self.state_values.get(state).copied().unwrap_or(0.0)
    }

    /// Update value estimate
    pub fn update(&mut self, state: &str, reward: f64, next_value: f64, gamma: f64) {
        let target = reward + gamma * next_value;
        let current = self.get_value(state);

        let new_value = current + self.learning_rate * (target - current);
        self.state_values.insert(state.to_string(), new_value);
    }

    /// Batch update from episode
    pub fn update_from_episode(&mut self, experiences: &[Experience], gamma: f64) {
        let mut next_value = 0.0;
        for exp in experiences.iter().rev() {
            let value = self.get_value(&exp.state);
            let target = exp.reward + gamma * next_value;
            let td_error = target - value;

            let current = self.state_values.entry(exp.state.clone()).or_insert(0.0);
            *current += self.learning_rate * td_error;

            next_value = self.get_value(&exp.next_state);
        }
    }
}

// ============================================================================
// Replay Buffer
// ============================================================================

/// Experience replay buffer
pub struct ReplayBuffer {
    buffer: VecDeque<Experience>,
    capacity: usize,
}

impl ReplayBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Add an experience
    pub fn push(&mut self, experience: Experience) {
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
        }
        self.buffer.push_back(experience);
    }

    /// Sample a batch
    pub fn sample(&self, batch_size: usize) -> Vec<Experience> {
        let mut rng = rand::thread_rng();
        let len = self.buffer.len().min(batch_size);
        (0..len)
            .map(|_| {
                let idx = rng.gen_range(0..self.buffer.len());
                self.buffer[idx].clone()
            })
            .collect()
    }

    /// Get buffer size
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Clear buffer
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

// ============================================================================
// RL Optimizer
// ============================================================================

/// Configuration for RL optimizer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RLOptimizerConfig {
    /// Learning rate for policy
    pub policy_lr: f64,
    /// Learning rate for value function
    pub value_lr: f64,
    /// Discount factor (gamma)
    pub gamma: f64,
    /// PPO clip epsilon
    pub clip_epsilon: f64,
    /// Replay buffer capacity
    pub replay_capacity: usize,
    /// Batch size for training
    pub batch_size: usize,
    /// Available strategies
    pub strategies: Vec<String>,
}

impl Default for RLOptimizerConfig {
    fn default() -> Self {
        Self {
            policy_lr: 0.001,
            value_lr: 0.01,
            gamma: 0.99,
            clip_epsilon: 0.2,
            replay_capacity: 10000,
            batch_size: 32,
            strategies: vec![
                "direct".to_string(),
                "exploratory".to_string(),
                "conservative".to_string(),
                "aggressive".to_string(),
            ],
        }
    }
}

/// RL-based strategy optimizer
pub struct RLOptimizerCore {
    config: RLOptimizerConfig,
    policy: Arc<RwLock<PolicyNetwork>>,
    value: Arc<RwLock<ValueNetwork>>,
    replay: Arc<RwLock<ReplayBuffer>>,
    episode_buffer: Arc<RwLock<Vec<Experience>>>,
}

impl RLOptimizerCore {
    pub fn new(config: RLOptimizerConfig) -> Self {
        let policy = PolicyNetwork::new(config.strategies.len(), config.policy_lr);
        let value = ValueNetwork::new(config.value_lr);

        Self {
            config,
            policy: Arc::new(RwLock::new(policy)),
            value: Arc::new(RwLock::new(value)),
            replay: Arc::new(RwLock::new(ReplayBuffer::new(config.replay_capacity))),
            episode_buffer: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_default() -> Self {
        Self::new(RLOptimizerConfig::default())
    }

    /// Get available strategies
    pub fn strategies(&self) -> &[String] {
        &self.config.strategies
    }

    /// Select a strategy based on current state
    pub async fn select_strategy(&self, state: &str) -> (String, usize) {
        let policy = self.policy.read().await;
        let action_idx = policy.select_action();
        let strategy = self.config.strategies[action_idx].clone();
        (strategy, action_idx)
    }

    /// Record an experience
    pub async fn record_experience(
        &self,
        state: String,
        action: String,
        reward: f64,
        next_state: String,
        done: bool,
    ) {
        let experience = Experience {
            state,
            action,
            reward,
            next_state,
            done,
        };

        // Add to episode buffer
        let mut buffer = self.episode_buffer.write().await;
        buffer.push(experience);
    }

    /// End current episode and learn
    pub async fn end_episode(&self, success: bool) -> LearningResult {
        let mut buffer = self.episode_buffer.write().await;

        // Mark last experience as terminal
        if let Some(last) = buffer.last_mut() {
            last.done = true;
        }

        // Calculate total reward
        let total_reward: f64 = buffer.iter().map(|e| e.reward).sum();

        // Store experiences in replay buffer
        let mut replay = self.replay.write().await;
        for exp in buffer.drain(..) {
            replay.push(exp);
        }

        drop(replay);

        // Perform learning if enough samples
        let samples = self.replay.read().await.len();
        if samples >= self.config.batch_size {
            self.learn().await
        } else {
            LearningResult {
                updated: false,
                total_reward,
                policy_change: 0.0,
                samples_collected: samples,
                message: "Collecting more samples".to_string(),
            }
        }
    }

    /// Perform a learning update
    async fn learn(&self) -> LearningResult {
        let batch = {
            let replay = self.replay.read().await;
            replay.sample(self.config.batch_size)
        };

        if batch.is_empty() {
            return LearningResult {
                updated: false,
                total_reward: 0.0,
                policy_change: 0.0,
                samples_collected: 0,
                message: "No samples to learn from".to_string(),
            };
        }

        // Calculate advantages using value function baseline
        let mut total_policy_change = 0.0;

        {
            let mut policy = self.policy.write().await;

            for exp in &batch {
                // Get baseline value
                let baseline = {
                    let value = self.value.read().await;
                    value.get_value(&exp.state)
                };

                // Get next state value
                let next_value = {
                    let value = self.value.read().await;
                    value.get_value(&exp.next_state)
                };

                // Calculate TD target and advantage
                let td_target = exp.reward + self.config.gamma * next_value;
                let advantage = td_target - baseline;

                // Find action index
                let action_idx = self.config
                    .strategies
                    .iter()
                    .position(|s| s == &exp.action)
                    .unwrap_or(0);

                // Get old log prob for PPO
                let old_probs = policy.get_probs();
                let old_log_prob = old_probs[action_idx].ln().max(-20.0);

                // Update policy with PPO
                let probs_before = policy.strategy_scores.clone();
                policy.update_ppo(action_idx, old_log_prob, advantage, self.config.clip_epsilon);

                // Calculate policy change magnitude
                let change: f64 = policy.strategy_scores
                    .iter()
                    .zip(probs_before.iter())
                    .map(|(new, old)| (new - old).abs())
                    .sum();
                total_policy_change += change;
            }
        }

        // Update value network
        {
            let mut value = self.value.write().await;
            value.update_from_episode(&batch, self.config.gamma);
        }

        LearningResult {
            updated: true,
            total_reward: batch.iter().map(|e| e.reward).sum(),
            policy_change: total_policy_change / batch.len() as f64,
            samples_collected: self.replay.read().await.len(),
            message: "Learning completed".to_string(),
        }
    }

    /// Get current policy distribution
    pub async fn get_policy_distribution(&self) -> Vec<(String, f64)> {
        let policy = self.policy.read().await;
        let probs = policy.get_probs();

        self.config
            .strategies
            .iter()
            .zip(probs.iter())
            .map(|(s, p)| (s.clone(), *p))
            .collect()
    }

    /// Get optimizer statistics
    pub async fn get_stats(&self) -> RLOptimizerStats {
        let replay_len = self.replay.read().await.len();
        let episode_len = self.episode_buffer.read().await.len();

        RLOptimizerStats {
            replay_size: replay_len,
            current_episode_size: episode_len,
            strategy_distribution: self.get_policy_distribution().await,
        }
    }
}

/// Result of a learning update
#[derive(Debug, Clone)]
pub struct LearningResult {
    pub updated: bool,
    pub total_reward: f64,
    pub policy_change: f64,
    pub samples_collected: usize,
    pub message: String,
}

/// Statistics about the optimizer
#[derive(Debug, Clone)]
pub struct RLOptimizerStats {
    pub replay_size: usize,
    pub current_episode_size: usize,
    pub strategy_distribution: Vec<(String, f64)>,
}

// ============================================================================
// Integration with Meta Cognitive
// ============================================================================

/// Strategy recommendation based on RL learned policy
pub struct RLStrategyRecommender {
    optimizer: Arc<RLOptimizerCore>,
    llm: Arc<Box<dyn LlmClient>>,
}

impl RLStrategyRecommender {
    pub fn new(optimizer: Arc<RLOptimizerCore>, llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self { optimizer, llm }
    }

    /// Recommend strategy based on task
    pub async fn recommend(&self, task: &str) -> StrategyRecommendation {
        // Use RL optimizer to select strategy
        let (strategy, action_idx) = self.optimizer.select_strategy(task).await;

        // Get policy distribution for alternatives
        let distribution = self.optimizer.get_policy_distribution().await;
        let confidence = distribution
            .get(action_idx)
            .map(|(_, p)| *p)
            .unwrap_or(0.0);

        let alternatives: Vec<(String, f64)> = distribution
            .into_iter()
            .filter(|(s, _)| s != &strategy)
            .take(3)
            .collect();

        // Generate reasoning using LLM
        let reasoning = self.generate_reasoning(task, &strategy, confidence).await;

        StrategyRecommendation {
            recommended_strategy: strategy,
            confidence,
            alternatives,
            reasoning,
        }
    }

    /// Generate reasoning for the recommendation
    async fn generate_reasoning(&self, task: &str, strategy: &str, confidence: f64) -> String {
        let prompt = format!(
            "Task: {}\nSelected Strategy: {}\nConfidence: {:.2}\nExplain why this strategy is suitable for the task in 1-2 sentences.",
            task, strategy, confidence
        );

        let messages = vec![
            Message::system("You are a strategy advisor. Give brief explanations."),
            Message::user(prompt),
        ];

        self.llm.chat_complete(&messages).await.unwrap_or_else(|_| "Strategy selected based on learned policy.".to_string())
    }

    /// Record outcome for learning
    pub async fn record_outcome(&self, task: &str, strategy: &str, outcome: StrategyOutcome) {
        let reward = outcome.to_reward();

        self.optimizer
            .record_experience(
                task.to_string(),
                strategy.to_string(),
                reward,
                task.to_string(), // For simplicity, state doesn't change much
                true,
            )
            .await;
    }

    /// End the learning episode
    pub async fn finish_episode(&self, success: bool) -> LearningResult {
        self.optimizer.end_episode(success).await
    }
}

/// Outcome of a strategy execution
#[derive(Debug, Clone)]
pub enum StrategyOutcome {
    /// Task completed successfully
    Success,
    /// Task completed with some issues
    PartialSuccess { quality: f64 },
    /// Task failed
    Failure,
    /// Task timed out
    Timeout,
}

impl StrategyOutcome {
    pub fn to_reward(&self) -> f64 {
        match self {
            StrategyOutcome::Success => 1.0,
            StrategyOutcome::PartialSuccess { quality } => *quality,
            StrategyOutcome::Failure => -0.5,
            StrategyOutcome::Timeout => -0.3,
        }
    }
}

/// Recommendation result
#[derive(Debug, Clone)]
pub struct StrategyRecommendation {
    pub recommended_strategy: String,
    pub confidence: f64,
    pub alternatives: Vec<(String, f64)>,
    pub reasoning: String,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_network() {
        let mut policy = PolicyNetwork::new(4, 0.01);

        // Select a few actions
        let actions: Vec<usize> = (0..10).map(|_| policy.select_action()).collect();
        println!("Selected actions: {:?}", actions);

        // Check probabilities sum to 1
        let probs = policy.get_probs();
        let sum: f64 = probs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);

        // Update policy
        policy.update(0, 1.0);
        let new_probs = policy.get_probs();
        assert_eq!(new_probs.len(), 4);
    }

    #[test]
    fn test_value_network() {
        let mut value = ValueNetwork::new(0.1);

        // Initial value
        assert_eq!(value.get_value("state1"), 0.0);

        // Update
        value.update("state1", 1.0, 0.5, 0.99);
        let new_val = value.get_value("state1");
        assert!(new_val > 0.0);

        // Update from episode
        let experiences = vec![
            Experience {
                state: "s1".to_string(),
                action: "a1".to_string(),
                reward: 1.0,
                next_state: "s2".to_string(),
                done: false,
            },
            Experience {
                state: "s2".to_string(),
                action: "a2".to_string(),
                reward: 0.5,
                next_state: "s3".to_string(),
                done: false,
            },
            Experience {
                state: "s3".to_string(),
                action: "a3".to_string(),
                reward: 0.0,
                next_state: "s3".to_string(),
                done: true,
            },
        ];
        value.update_from_episode(&experiences, 0.99);
    }

    #[test]
    fn test_replay_buffer() {
        let mut buffer = ReplayBuffer::new(3);

        // Add experiences
        buffer.push(Experience {
            state: "s1".to_string(),
            action: "a1".to_string(),
            reward: 1.0,
            next_state: "s2".to_string(),
            done: false,
        });

        assert_eq!(buffer.len(), 1);

        // Add more than capacity
        for i in 0..5 {
            buffer.push(Experience {
                state: format!("s{}", i),
                action: format!("a{}", i),
                reward: 1.0,
                next_state: format!("s{}", i + 1),
                done: false,
            });
        }

        // Should be at capacity
        assert_eq!(buffer.len(), 3);

        // Sample
        let batch = buffer.sample(2);
        assert!(batch.len() <= 2);
    }

    #[tokio::test]
    async fn test_rl_optimizer() {
        let optimizer = Arc::new(RLOptimizerCore::with_default());

        // Select strategies
        let (s1, _) = optimizer.select_strategy("test task").await;
        println!("Selected: {}", s1);

        // Record experiences
        optimizer
            .record_experience(
                "task1".to_string(),
                s1.clone(),
                1.0,
                "task1".to_string(),
                false,
            )
            .await;

        optimizer
            .record_experience(
                "task2".to_string(),
                "exploratory".to_string(),
                0.5,
                "task2".to_string(),
                false,
            )
            .await;

        // End episode
        let result = optimizer.end_episode(true).await;
        println!("Learning result: {:?}", result);

        // Get stats
        let stats = optimizer.get_stats().await;
        println!("Stats: {:?}", stats);
    }

    #[test]
    fn test_strategy_outcome_reward() {
        assert_eq!(StrategyOutcome::Success.to_reward(), 1.0);
        assert_eq!(StrategyOutcome::PartialSuccess { quality: 0.7 }.to_reward(), 0.7);
        assert_eq!(StrategyOutcome::Failure.to_reward(), -0.5);
        assert_eq!(StrategyOutcome::Timeout.to_reward(), -0.3);
    }
}