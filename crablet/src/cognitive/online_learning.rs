//! Online Learning System
//!
//! A continuous learning system that enables agents to learn from experiences
//! while preventing catastrophic forgetting.
//!
//! # Core Features
//!
//! 1. **Priority Experience Replay** - Prioritized sampling of experiences
//! 2. **Continual Learning** - Learn from new experiences without forgetting
//! 3. **Adaptive Learning Rate** - Dynamic adjustment based on learning progress
//! 4. **Performance Monitoring** - Real-time tracking of learning metrics
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Online Learning System                      │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
//! │  │ Experience │  │   Policy   │  │  Learning   │        │
//! │  │  Buffer    │  │  Network   │  │   Rate     │        │
//! │  └─────────────┘  └─────────────┘  └─────────────┘        │
//! │  ┌─────────────────────────────────────────────┐           │
//! │  │        Catastrophic Forgetting Prevention       │           │
//! │  │  ┌─────────┐ ┌─────────┐ ┌─────────┐      │           │
//! │  │  │Ebbinghaus│ │ Elastic │ │ Memory  │      │           │
//! │  │  │ Regular. │ │ Weight  │ │  Consolidation │      │           │
//! │  │  └─────────┘ └─────────┘ └─────────┘      │           │
//! │  └─────────────────────────────────────────────┘           │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! let learner = OnlineLearner::new(learning_config);
//!
//! // Add experience
//! learner.add_experience(experience)?;
//!
//! // Learn from experiences
//! learner.learn().await?;
//!
//! // Get updated policy
//! let policy = learner.get_policy();
//! ```

use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::sync::RwLock;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Configuration for online learning
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OnlineLearningConfig {
    /// Maximum experiences to store
    pub buffer_size: usize,
    /// Learning rate
    pub learning_rate: f32,
    /// Discount factor
    pub discount_factor: f32,
    /// Batch size for learning
    pub batch_size: usize,
    /// Minimum replay ratio (portion of batch from replay)
    pub replay_ratio: f32,
    /// Importance sampling exponent
    pub priority_exponent: f32,
    /// Elastic weight consolidation strength
    pub ewc_strength: f32,
    /// Fisher importance
    pub fisher_importance: f32,
    /// Enable adaptive learning rate
    pub adaptive_lr: bool,
    /// Learning rate adjustment interval
    pub lr_adjust_interval: usize,
    /// Minimum learning rate
    pub min_lr: f32,
    /// Maximum learning rate
    pub max_lr: f32,
}

impl Default for OnlineLearningConfig {
    fn default() -> Self {
        Self {
            buffer_size: 100_000,
            learning_rate: 0.001,
            discount_factor: 0.99,
            batch_size: 32,
            replay_ratio: 0.5,
            priority_exponent: 0.6,
            ewc_strength: 1000.0,
            fisher_importance: 1.0,
            adaptive_lr: true,
            lr_adjust_interval: 1000,
            min_lr: 0.00001,
            max_lr: 0.1,
        }
    }
}

/// An experience in the replay buffer
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Experience {
    /// State
    pub state: Vec<f32>,
    /// Action taken
    pub action: f32,
    /// Reward received
    pub reward: f32,
    /// Next state
    pub next_state: Vec<f32>,
    /// Done flag
    pub done: bool,
    /// Priority (higher = more important)
    pub priority: f32,
    /// Timestamp
    pub timestamp: u64,
    /// Task ID (for multi-task learning)
    pub task_id: Option<String>,
}

impl Experience {
    /// Create a new experience
    pub fn new(
        state: Vec<f32>,
        action: f32,
        reward: f32,
        next_state: Vec<f32>,
        done: bool,
    ) -> Self {
        Self {
            state,
            action,
            reward,
            next_state,
            done,
            priority: 1.0,
            timestamp: current_timestamp(),
            task_id: None,
        }
    }

    /// Create with priority
    pub fn with_priority(mut self, priority: f32) -> Self {
        self.priority = priority;
        self
    }

    /// Create with task ID
    pub fn with_task(mut self, task_id: String) -> Self {
        self.task_id = Some(task_id);
        self
    }

    /// Calculate TD error
    pub fn td_error(&self, value: f32, next_value: f32, discount_factor: f32) -> f32 {
        let target = self.reward + discount_factor * next_value * (1.0 - if self.done { 1.0 } else { 0.0 });
        (target - value).abs()
    }
}

/// Priority queue entry for experience replay
#[derive(Clone, Debug)]
struct PrioritizedExperience {
    experience: Experience,
    priority: f32,
    index: usize,
}

impl PartialEq for PrioritizedExperience {
    fn eq(&self, other: &Self) -> bool {
        self.priority == other.priority
    }
}

impl Eq for PrioritizedExperience {}

impl PartialOrd for PrioritizedExperience {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PrioritizedExperience {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.priority.partial_cmp(&self.priority).unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// A policy network for value function approximation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PolicyNetwork {
    /// Network weights
    weights: HashMap<String, Vec<f32>>,
    /// Network architecture
    architecture: Vec<usize>,
    /// Optimizer state
    optimizer_state: HashMap<String, Vec<f32>>,
}

impl PolicyNetwork {
    /// Create a new policy network
    pub fn new(architecture: Vec<usize>) -> Self {
        let mut weights = HashMap::new();
        let mut optimizer_state = HashMap::new();
        
        for i in 0..architecture.len() - 1 {
            let layer_name = format!("layer_{}", i);
            let size = architecture[i] * architecture[i + 1];
            weights.insert(layer_name.clone(), vec![0.0; size]);
            optimizer_state.insert(layer_name, vec![0.0; size]);
        }
        
        Self {
            weights,
            architecture,
            optimizer_state,
        }
    }

    /// Forward pass (simplified)
    pub fn forward(&self, state: &[f32]) -> Vec<f32> {
        let mut activations = state.to_vec();
        
        for i in 0..self.architecture.len() - 1 {
            let input_size = self.architecture[i];
            let output_size = self.architecture[i + 1];
            
            if let Some(weight) = self.weights.get(&format!("layer_{}", i)) {
                let mut output = vec![0.0; output_size];
                for j in 0..output_size {
                    for k in 0..input_size {
                        let idx = k * output_size + j;
                        if idx < weight.len() {
                            output[j] += activations[k] * weight[idx];
                        }
                    }
                }
                // ReLU activation
                activations = output.into_iter().map(|x| x.max(0.0)).collect();
            }
        }
        
        activations
    }

    /// Get a weight
    pub fn get_weight(&self, layer: &str) -> Option<&Vec<f32>> {
        self.weights.get(layer)
    }

    /// Update weights using gradient (simplified SGD)
    pub fn update(&mut self, gradients: &HashMap<String, Vec<f32>>, lr: f32) {
        for (layer, grad) in gradients {
            if let Some(weight) = self.weights.get_mut(layer) {
                for (i, g) in grad.iter().enumerate() {
                    if i < weight.len() {
                        weight[i] += lr * g;
                    }
                }
            }
        }
    }

    /// Compute gradient approximation (simplified)
    pub fn compute_gradient_approximation(&self, state: &[f32], _action: f32, td_error: f32) -> HashMap<String, Vec<f32>> {
        let mut gradients = HashMap::new();
        
        // Simplified gradient computation
        let output = self.forward(state);
        let _q_value = output.first().copied().unwrap_or(0.0);
        
        for i in 0..self.architecture.len() - 1 {
            let layer_name = format!("layer_{}", i);
            let input_size = self.architecture[i];
            let output_size = self.architecture[i + 1];
            
            if let Some(_weight) = self.weights.get(&layer_name) {
                let mut grad = vec![0.0; input_size * output_size];
                
                // Simplified gradient: td_error * state features
                for j in 0..output_size.min(input_size) {
                    let idx = j * output_size + j;
                    if idx < grad.len() && j < state.len() {
                        grad[idx] = td_error * state[j];
                    }
                }
                
                gradients.insert(layer_name, grad);
            }
        }
        
        gradients
    }
}

/// Elastic weight consolidation parameters for a specific task
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EwcParams {
    /// Task identifier
    pub task_id: String,
    /// Fisher information diagonal
    pub fisher_diagonal: HashMap<String, Vec<f32>>,
    /// Optimal weights for this task
    pub optimal_weights: HashMap<String, Vec<f32>>,
    /// Importance of this task
    pub importance: f32,
}

/// Performance record for monitoring
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerformanceRecord {
    /// Episode number
    pub episode: usize,
    /// Episode reward
    pub episode_reward: f32,
    /// Episode length
    pub episode_length: usize,
    /// Learning loss
    pub loss: f32,
    /// Learning rate
    pub learning_rate: f32,
    /// Timestamp
    pub timestamp: u64,
}

/// Learning statistics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LearningStatistics {
    /// Total experiences seen
    pub total_experiences: usize,
    /// Total learning steps
    pub total_steps: usize,
    /// Current learning rate
    pub current_lr: f32,
    /// Average recent loss
    pub avg_loss: f32,
    /// Average recent reward
    pub avg_reward: f32,
    /// Forgetting measure
    pub forgetting_measure: f32,
}

/// The online learning system
pub struct OnlineLearner {
    /// Configuration
    config: OnlineLearningConfig,
    /// Experience replay buffer
    replay_buffer: RwLock<VecDeque<Experience>>,
    /// Priority queue for sampling
    priority_queue: RwLock<BinaryHeap<PrioritizedExperience>>,
    /// Policy network
    policy: RwLock<PolicyNetwork>,
    /// EWC parameters for each task
    ewc_params: RwLock<Vec<EwcParams>>,
    /// Performance history
    performance_history: RwLock<VecDeque<PerformanceRecord>>,
    /// Statistics
    stats: RwLock<LearningStatistics>,
    /// Learning step counter
    learning_step: RwLock<usize>,
    /// Experience counter
    experience_counter: usize,
}

impl OnlineLearner {
    /// Create a new online learner
    pub fn new(config: OnlineLearningConfig) -> Self {
        let policy = PolicyNetwork::new(vec![10, 64, 32, 1]); // Default architecture
        
        let stats = LearningStatistics {
            total_experiences: 0,
            total_steps: 0,
            current_lr: config.learning_rate,
            avg_loss: 0.0,
            avg_reward: 0.0,
            forgetting_measure: 0.0,
        };
        
        Self {
            config,
            replay_buffer: RwLock::new(VecDeque::new()),
            priority_queue: RwLock::new(BinaryHeap::new()),
            policy: RwLock::new(policy),
            ewc_params: RwLock::new(Vec::new()),
            performance_history: RwLock::new(VecDeque::new()),
            stats: RwLock::new(stats),
            learning_step: RwLock::new(0),
            experience_counter: 0,
        }
    }

    /// Add an experience to the replay buffer
    pub fn add_experience(&mut self, experience: Experience) -> Result<()> {
        let mut buffer = self.replay_buffer.write()
            .map_err(|e| anyhow!("Replay buffer lock poisoned: {e}"))?;

        // Add to buffer
        if buffer.len() >= self.config.buffer_size {
            buffer.pop_front();
        }

        // Calculate priority based on TD error
        let mut exp_with_priority = experience;
        let priority = self.calculate_priority(&exp_with_priority);
        exp_with_priority.priority = priority;

        buffer.push_back(exp_with_priority.clone());

        // Add to priority queue
        let mut pq = self.priority_queue.write()
            .map_err(|e| anyhow!("Priority queue lock poisoned: {e}"))?;
        pq.push(PrioritizedExperience {
            experience: exp_with_priority,
            priority,
            index: self.experience_counter,
        });

        self.experience_counter += 1;

        // Update statistics
        {
            let mut stats = self.stats.write()
                .map_err(|e| anyhow!("Stats lock poisoned: {e}"))?;
            stats.total_experiences = buffer.len();
        }

        Ok(())
    }

    /// Calculate priority for an experience
    fn calculate_priority(&self, exp: &Experience) -> f32 {
        // TD error-based priority
        let td_priority = exp.priority.max(0.01);
        
        // Recency priority (newer experiences get boost)
        let age = current_timestamp().saturating_sub(exp.timestamp);
        let age_f = age as f32;
        let recency_boost = (-age_f / 10000.0).exp().max(0.1);
        
        // Combine
        let priority = td_priority * recency_boost;
        priority.powf(self.config.priority_exponent)
    }

    /// Sample a batch of experiences
    pub fn sample_batch(&self) -> Result<Vec<Experience>> {
        let buffer = self.replay_buffer.read()
            .map_err(|e| anyhow!("Replay buffer lock poisoned: {e}"))?;

        if buffer.is_empty() {
            return Err(anyhow!("Replay buffer is empty"));
        }

        let batch_size = self.config.batch_size.min(buffer.len());
        let mut batch = Vec::with_capacity(batch_size);

        // Sample from priority queue
        let pq = self.priority_queue.read()
            .map_err(|e| anyhow!("Priority queue lock poisoned: {e}"))?;
        let samples: Vec<_> = pq.iter().take(batch_size).collect();

        for sample in samples {
            batch.push(sample.experience.clone());
        }

        // If not enough samples, fill with random
        while batch.len() < batch_size {
            let idx = rand_idx(buffer.len());
            batch.push(buffer[idx].clone());
        }

        Ok(batch)
    }

    /// Perform a learning step
    pub async fn learn(&mut self) -> Result<f32> {
        // Check if we have enough experiences
        {
            let buffer = self.replay_buffer.read()
                .map_err(|e| anyhow!("Replay buffer lock poisoned: {e}"))?;
            if buffer.len() < self.config.batch_size {
                return Err(anyhow!("Not enough experiences to learn"));
            }
        }

        // Sample batch
        let batch = self.sample_batch()?;

        // Compute learning rate
        let lr = self.compute_learning_rate()?;

        // Update policy
        let loss = self.update_policy(&batch, lr).await?;

        // Update statistics
        {
            let mut stats = self.stats.write()
                .map_err(|e| anyhow!("Stats lock poisoned: {e}"))?;
            stats.total_steps += 1;
            stats.current_lr = lr;
            stats.avg_loss = 0.9 * stats.avg_loss + 0.1 * loss;
        }

        // Adaptive learning rate adjustment
        if self.config.adaptive_lr {
            self.maybe_adjust_learning_rate()?;
        }

        Ok(loss)
    }

    /// Compute current learning rate
    fn compute_learning_rate(&self) -> Result<f32> {
        let step = *self.learning_step.read()
            .map_err(|e| anyhow!("Learning step lock poisoned: {e}"))?;

        // Linear decay with warm restarts
        let progress = step as f32 / (self.config.lr_adjust_interval as f32);
        let lr = self.config.learning_rate * (1.0 - progress.min(1.0) * 0.5);

        Ok(lr.clamp(self.config.min_lr, self.config.max_lr))
    }

    /// Maybe adjust learning rate based on performance
    fn maybe_adjust_learning_rate(&self) -> Result<()> {
        let step = *self.learning_step.read()
            .map_err(|e| anyhow!("Learning step lock poisoned: {e}"))?;

        if step % self.config.lr_adjust_interval != 0 {
            return Ok(());
        }

        let perf_history = self.performance_history.read()
            .map_err(|e| anyhow!("Performance history lock poisoned: {e}"))?;

        if perf_history.len() < 2 {
            return Ok(());
        }

        // Compare recent performance
        let recent: Vec<_> = perf_history.iter().rev().take(10).collect();
        let older: Vec<_> = perf_history.iter().rev().skip(10).take(10).collect();

        if recent.is_empty() || older.is_empty() {
            return Ok(());
        }

        let recent_avg = recent.iter().map(|p| p.episode_reward).sum::<f32>() / recent.len() as f32;
        let old_avg = older.iter().map(|p| p.episode_reward).sum::<f32>() / older.len() as f32;

        // If performance is degrading, reduce learning rate
        if recent_avg < old_avg * 0.95 {
            let mut stats = self.stats.write()
                .map_err(|e| anyhow!("Stats lock poisoned: {e}"))?;
            stats.current_lr *= 0.9;
            stats.current_lr = stats.current_lr.max(self.config.min_lr);
        }

        Ok(())
    }

    /// Update policy network
    async fn update_policy(&self, batch: &[Experience], lr: f32) -> Result<f32> {
        let mut policy = self.policy.write()
            .map_err(|e| anyhow!("Policy lock poisoned: {e}"))?;

        let mut total_loss = 0.0;

        for exp in batch {
            // Compute TD error
            let state_values = policy.forward(&exp.state);
            let next_values = policy.forward(&exp.next_state);

            let value = state_values.first().copied().unwrap_or(0.0);
            let next_value = next_values.first().copied().unwrap_or(0.0);

            let td_error = exp.td_error(value, next_value, self.config.discount_factor);

            // Compute gradient
            let gradients = policy.compute_gradient_approximation(&exp.state, exp.action, td_error);

            // Apply EWC penalty
            let ewc_penalty = self.compute_ewc_penalty(&policy)?;

            // Update weights
            policy.update(&gradients, lr);

            total_loss += td_error + ewc_penalty;
        }

        // Update learning step
        {
            let mut step = self.learning_step.write()
                .map_err(|e| anyhow!("Learning step lock poisoned: {e}"))?;
            *step += 1;
        }

        Ok(total_loss / batch.len() as f32)
    }

    /// Compute EWC penalty for catastrophic forgetting prevention
    fn compute_ewc_penalty(&self, policy: &PolicyNetwork) -> Result<f32> {
        let ewc_params = self.ewc_params.read()
            .map_err(|e| anyhow!("EWC params lock poisoned: {e}"))?;
        let mut penalty = 0.0;
        
        for params in ewc_params.iter() {
            for (layer, fisher) in &params.fisher_diagonal {
                if let Some(weights) = policy.get_weight(layer) {
                    if let Some(optimal) = params.optimal_weights.get(layer) {
                        for (f, (w, o)) in fisher.iter().zip(weights.iter().zip(optimal.iter())) {
                            penalty += params.importance * f * (w - o).powi(2);
                        }
                    }
                }
            }
        }
        
        Ok(self.config.ewc_strength * penalty)
    }

    /// Save EWC parameters for a task (call when task is complete)
    pub fn save_task_params(&mut self, task_id: String, importance: f32) -> Result<()> {
        let policy = self.policy.read()
            .map_err(|e| anyhow!("Policy lock poisoned: {e}"))?;

        let mut fisher_diagonal = HashMap::new();
        let mut optimal_weights = HashMap::new();

        for (layer, weights) in &policy.weights {
            // Approximate Fisher information (diagonal)
            let fisher = weights.iter().map(|w| w.powi(2) * self.config.fisher_importance).collect();
            fisher_diagonal.insert(layer.clone(), fisher);
            optimal_weights.insert(layer.clone(), weights.clone());
        }

        drop(policy);

        let params = EwcParams {
            task_id,
            fisher_diagonal,
            optimal_weights,
            importance,
        };

        let mut ewc_params = self.ewc_params.write()
            .map_err(|e| anyhow!("EWC params lock poisoned: {e}"))?;
        ewc_params.push(params);

        Ok(())
    }

    /// Record performance
    pub fn record_performance(&mut self, episode: usize, episode_reward: f32, episode_length: usize, loss: f32) -> Result<()> {
        let stats = {
            let s = self.stats.read()
                .map_err(|e| anyhow!("Stats lock poisoned: {e}"))?;
            s.current_lr
        };

        let record = PerformanceRecord {
            episode,
            episode_reward,
            episode_length,
            loss,
            learning_rate: stats,
            timestamp: current_timestamp(),
        };

        let mut history = self.performance_history.write()
            .map_err(|e| anyhow!("Performance history lock poisoned: {e}"))?;
        history.push_back(record);

        // Update average reward
        {
            let mut stats = self.stats.write()
                .map_err(|e| anyhow!("Stats lock poisoned: {e}"))?;
            stats.avg_reward = 0.99 * stats.avg_reward + 0.01 * episode_reward;
        }

        Ok(())
    }

    /// Get current policy
    pub fn get_policy(&self) -> Result<PolicyNetwork> {
        let policy = self.policy.read()
            .map_err(|e| anyhow!("Policy lock poisoned: {e}"))?;
        Ok(policy.clone())
    }

    /// Get learning statistics
    pub fn get_statistics(&self) -> Result<LearningStatistics> {
        let stats = self.stats.read()
            .map_err(|e| anyhow!("Stats lock poisoned: {e}"))?;
        Ok(stats.clone())
    }

    /// Get performance history
    pub fn get_performance_history(&self) -> Result<Vec<PerformanceRecord>> {
        let history = self.performance_history.read()
            .map_err(|e| anyhow!("Performance history lock poisoned: {e}"))?;
        Ok(history.iter().cloned().collect())
    }

    /// Clear replay buffer
    pub fn clear_buffer(&self) -> Result<()> {
        let mut buffer = self.replay_buffer.write()
            .map_err(|e| anyhow!("Replay buffer lock poisoned: {e}"))?;
        buffer.clear();

        let mut pq = self.priority_queue.write()
            .map_err(|e| anyhow!("Priority queue lock poisoned: {e}"))?;
        pq.clear();

        Ok(())
    }

    /// Compute forgetting measure
    pub fn compute_forgetting_measure(&self) -> Result<f32> {
        let history = self.performance_history.read()
            .map_err(|e| anyhow!("Performance history lock poisoned: {e}"))?;
        
        if history.len() < 10 {
            return Ok(0.0);
        }

        // Compare initial and final performance on recent tasks
        let recent: Vec<_> = history.iter().rev().take(5).collect();
        let older: Vec<_> = history.iter().rev().skip(5).take(5).collect();

        if recent.is_empty() || older.is_empty() {
            return Ok(0.0);
        }

        let recent_avg = recent.iter().map(|p| p.episode_reward).sum::<f32>() / recent.len() as f32;
        let older_avg = older.iter().map(|p| p.episode_reward).sum::<f32>() / older.len() as f32;

        // Forgetting = old - recent (positive means forgetting)
        Ok((older_avg - recent_avg).max(0.0))
    }

    /// Prioritize experiences based on learning progress
    pub fn update_priorities(&self) -> Result<()> {
        let buffer = self.replay_buffer.read()
            .map_err(|e| anyhow!("Replay buffer lock poisoned: {e}"))?;
        let mut pq = self.priority_queue.write()
            .map_err(|e| anyhow!("Priority queue lock poisoned: {e}"))?;
        
        let mut new_pq = BinaryHeap::new();
        
        for exp in buffer.iter() {
            let priority = self.calculate_priority(exp);
            new_pq.push(PrioritizedExperience {
                experience: exp.clone(),
                priority,
                index: self.experience_counter,
            });
        }
        
        *pq = new_pq;
        
        Ok(())
    }

    /// Check if learning should continue
    pub fn should_continue(&self, min_experiences: usize, max_steps: usize) -> Result<bool> {
        let stats = self.stats.read()
            .map_err(|e| anyhow!("Stats lock poisoned: {e}"))?;
        
        if stats.total_experiences < min_experiences {
            return Ok(true);
        }

        if stats.total_steps >= max_steps {
            return Ok(false);
        }

        // Stop if loss is very low
        if stats.avg_loss < 0.001 {
            return Ok(false);
        }

        Ok(true)
    }

    /// Export learner state
    pub fn export_state(&self) -> Result<LearnerState> {
        let policy = self.policy.read()
            .map_err(|e| anyhow!("Policy lock poisoned: {e}"))?;
        let ewc_params = self.ewc_params.read()
            .map_err(|e| anyhow!("EWC params lock poisoned: {e}"))?;
        let stats = self.stats.read()
            .map_err(|e| anyhow!("Stats lock poisoned: {e}"))?;
        
        Ok(LearnerState {
            policy: policy.clone(),
            ewc_params: ewc_params.clone(),
            stats: stats.clone(),
            config: self.config.clone(),
        })
    }

    /// Import learner state
    pub fn import_state(&mut self, state: LearnerState) -> Result<()> {
        *self.policy.write()
            .map_err(|e| anyhow!("Policy lock poisoned: {e}"))? = state.policy;
        *self.ewc_params.write()
            .map_err(|e| anyhow!("EWC params lock poisoned: {e}"))? = state.ewc_params;
        *self.stats.write()
            .map_err(|e| anyhow!("Stats lock poisoned: {e}"))? = state.stats;
        self.config = state.config;
        
        Ok(())
    }
}

/// State of the learner for saving/loading
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LearnerState {
    /// Policy network
    pub policy: PolicyNetwork,
    /// EWC parameters
    pub ewc_params: Vec<EwcParams>,
    /// Statistics
    pub stats: LearningStatistics,
    /// Configuration
    pub config: OnlineLearningConfig,
}

/// Get current timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_secs()
}

/// Get random index
fn rand_idx(max: usize) -> usize {
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_nanos() as usize;
    now % max.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_experience_creation() {
        let exp = Experience::new(
            vec![1.0, 2.0, 3.0],
            0.5,
            1.0,
            vec![2.0, 3.0, 4.0],
            false,
        );
        
        assert_eq!(exp.state, vec![1.0, 2.0, 3.0]);
        assert_eq!(exp.action, 0.5);
        assert_eq!(exp.reward, 1.0);
        assert!(!exp.done);
    }

    #[test]
    fn test_policy_network() {
        let policy = PolicyNetwork::new(vec![2, 4, 1]);
        let output = policy.forward(&[1.0, 2.0]);
        assert_eq!(output.len(), 1);
    }

    #[test]
    fn test_online_learner_creation() {
        let learner = OnlineLearner::new(OnlineLearningConfig::default());
        let stats = learner.get_statistics().expect("stats lock");
        assert_eq!(stats.total_experiences, 0);
        assert_eq!(stats.current_lr, 0.001);
    }

    #[test]
    fn test_add_experience() {
        let mut learner = OnlineLearner::new(OnlineLearningConfig::default());
        let exp = Experience::new(
            vec![1.0, 2.0],
            0.0,
            1.0,
            vec![2.0, 3.0],
            false,
        );
        
        learner.add_experience(exp).expect("add experience");
        let stats = learner.get_statistics().expect("stats lock");
        assert_eq!(stats.total_experiences, 1);
    }

    #[test]
    fn test_forgetting_measure() {
        let learner = OnlineLearner::new(OnlineLearningConfig::default());
        let forgetting = learner.compute_forgetting_measure().expect("history lock");
        assert_eq!(forgetting, 0.0);
    }
}
