//! Adaptive RL-based Agent Optimization System
//!
//! 基于强化学习的 Agent 自适应优化系统:
//! - 在线学习选择最佳策略
//! - Multi-Armed Bandit 探索-利用平衡
//! - 策略梯度优化

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use rand::Rng;

/// 策略类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Strategy {
    System1,     // 快速响应
    System2,     // 分析思考
    System3,     // 深度推理
    Swarm,       // 多 Agent 协作
    ToolFirst,   // 优先使用工具
    ToolLast,    // 优先自主思考
}

/// 策略评估结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyEvaluation {
    pub strategy: Strategy,
    pub reward: f32,         // 奖励值 (-1 到 1)
    pub latency_ms: u64,     // 延迟
    pub success: bool,       // 是否成功
    pub quality: f32,        // 质量评分
}

/// 策略性能记录
#[derive(Debug, Clone)]
pub struct StrategyRecord {
    pub strategy: Strategy,
    pub total_trials: u32,
    pub successful_trials: u32,
    pub total_reward: f32,
    pub avg_latency_ms: f32,
    pub last_updated: std::time::Instant,
}

impl StrategyRecord {
    pub fn success_rate(&self) -> f32 {
        if self.total_trials == 0 { 0.0 }
        else { self.successful_trials as f32 / self.total_trials as f32 }
    }
    
    pub fn avg_reward(&self) -> f32 {
        if self.total_trials == 0 { 0.0 }
        else { self.total_reward / self.total_trials as f32 }
    }
    
    pub fn ucb_score(&self, total_trials: u32, exploration_param: f32) -> f32 {
        if self.total_trials == 0 {
            return f32::MAX;  // 未尝试的策略优先
        }
        
        let exploitation = self.avg_reward();
        let exploration = exploration_param * 
            ((total_trials as f32).ln() / self.total_trials as f32).sqrt();
        
        exploitation + exploration
    }
}

/// 多臂老虎机选择器
pub struct MultiArmedBandit {
    strategies: HashMap<Strategy, StrategyRecord>,
    exploration_param: f32,  // UCB 探索参数
    epsilon: f32,           // ε-greedy 概率
    history: VecDeque<StrategyEvaluation>,
    max_history: usize,
}

impl MultiArmedBandit {
    pub fn new(exploration_param: f32, epsilon: f32) -> Self {
        let mut strategies = HashMap::new();
        
        for strategy in [
            Strategy::System1, Strategy::System2, Strategy::System3,
            Strategy::Swarm, Strategy::ToolFirst, Strategy::ToolLast
        ] {
            strategies.insert(strategy, StrategyRecord {
                strategy,
                total_trials: 0,
                successful_trials: 0,
                total_reward: 0.0,
                avg_latency_ms: 0.0,
                last_updated: std::time::Instant::now(),
            });
        }
        
        Self {
            strategies,
            exploration_param,
            epsilon,
            history: VecDeque::new(),
            max_history: 1000,
        }
    }
    
    /// 选择策略 (UCB + ε-greedy)
    pub fn select(&self, task_complexity: f32) -> Strategy {
        let mut rng = rand::thread_rng();
        
        // ε-greedy 探索
        if rng.gen::<f32>() < self.epsilon {
            let strategies: Vec<Strategy> = self.strategies.keys().cloned().collect();
            // rand 0.10: gen_range is inclusive on both ends (..=)
            let max_idx = strategies.len().saturating_sub(1);
            return strategies[rng.gen_range(0..=max_idx)];
        }
        
        // 根据任务复杂度调整选择
        let total_trials: u32 = self.strategies.values().map(|r| r.total_trials).sum();
        
        // 基于复杂度的策略偏好
        let preferred = match task_complexity as u8 {
            0..=3 => Strategy::System1,
            4..=6 => Strategy::System2,
            7..=10 => Strategy::System3,
            _ => Strategy::Swarm,
        };
        
        // UCB 选择，但给首选策略一定加成
        let mut best_strategy = preferred;
        let mut best_score = f32::MIN;
        
        for (strategy, record) in &self.strategies {
            let mut score = record.ucb_score(total_trials, self.exploration_param);
            
            // 对首选策略加成分数
            if *strategy == preferred {
                score += 0.1;
            }
            
            if score > best_score {
                best_score = score;
                best_strategy = *strategy;
            }
        }
        
        best_strategy
    }
    
    /// 更新策略性能
    pub fn update(&mut self, evaluation: StrategyEvaluation) {
        // 添加到历史
        if self.history.len() >= self.max_history {
            self.history.pop_front();
        }
        self.history.push_back(evaluation.clone());
        
        // 更新策略记录
        let record = self.strategies.get_mut(&evaluation.strategy).unwrap();
        record.total_trials += 1;
        
        if evaluation.success {
            record.successful_trials += 1;
        }
        
        // 计算奖励: 结合成功率、质量、延迟
        let reward = evaluation.reward 
            + evaluation.quality * 0.3 
            - (evaluation.latency_ms as f32 / 1000.0) * 0.2;
        
        record.total_reward += reward;
        
        // 更新平均延迟
        let new_avg = (record.avg_latency_ms * (record.total_trials - 1) as f32 
            + evaluation.latency_ms as f32) / record.total_trials as f32;
        record.avg_latency_ms = new_avg;
        
        record.last_updated = std::time::Instant::now();
    }
    
    /// 获取最佳策略
    pub fn best_strategy(&self) -> Option<Strategy> {
        self.strategies.values()
            .max_by(|a, b| a.avg_reward().partial_cmp(&b.avg_reward()).unwrap())
            .map(|r| r.strategy)
    }
    
    /// 获取所有策略性能
    pub fn get_all_performance(&self) -> Vec<(Strategy, f32, f32, f32)> {
        self.strategies.values()
            .map(|r| (r.strategy, r.success_rate(), r.avg_reward(), r.avg_latency_ms))
            .collect()
    }
    
    /// 衰减 ε (随着学习深入减少探索)
    pub fn decay_epsilon(&mut self, factor: f32) {
        self.epsilon = (self.epsilon * factor).max(0.05);
    }
}

/// 策略梯度优化器
pub struct PolicyGradientOptimizer {
    learning_rate: f32,
    discount_factor: f32,
    baseline: f32,
    value_estimates: HashMap<String, f32>,  // 任务特征 -> 价值估计
}

impl PolicyGradientOptimizer {
    pub fn new(learning_rate: f32, discount_factor: f32) -> Self {
        Self {
            learning_rate,
            discount_factor,
            baseline: 0.0,
            value_estimates: HashMap::new(),
        }
    }
    
    /// 计算优势 (Advantage)
    pub fn compute_advantage(&mut self, task_features: &str, reward: f32) -> f32 {
        // 更新基线 (指数移动平均)
        self.baseline = 0.9 * self.baseline + 0.1 * reward;
        
        // 更新价值估计
        let estimate = self.value_estimates.entry(task_features.to_string())
            .or_insert(reward);
        *estimate = *estimate + self.learning_rate * (reward - *estimate);
        
        // 优势 = 奖励 - 基线
        reward - self.baseline
    }
    
    /// 调整学习率 (基于性能自适应)
    pub fn adaptive_learning_rate(&self, recent_performance: f32) -> f32 {
        // 性能好时降低学习率，差时增加
        if recent_performance > 0.8 {
            self.learning_rate * 0.9
        } else if recent_performance < 0.3 {
            self.learning_rate * 1.2
        } else {
            self.learning_rate
        }
    }
}

/// 自适应 Agent 优化器
pub struct AdaptiveAgentOptimizer {
    bandit: Arc<RwLock<MultiArmedBandit>>,
    policy_gradient: Arc<RwLock<PolicyGradientOptimizer>>,
    task_history: VecDeque<TaskContext>,
    max_history: usize,
}

/// 任务上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    pub task_type: String,
    pub complexity: f32,
    pub domain: String,
    pub context_length: usize,
    pub has_tools: bool,
}

impl AdaptiveAgentOptimizer {
    pub fn new() -> Self {
        Self {
            bandit: Arc::new(RwLock::new(MultiArmedBandit::new(1.5, 0.2))),
            policy_gradient: Arc::new(RwLock::new(PolicyGradientOptimizer::new(0.1, 0.95))),
            task_history: VecDeque::new(),
            max_history: 100,
        }
    }
    
    /// 为任务选择最佳策略
    pub async fn select_strategy(&self, context: &TaskContext) -> Strategy {
        let bandit = self.bandit.read().await;
        bandit.select(context.complexity)
    }
    
    /// 记录任务执行结果并更新
    pub async fn record_result(
        &self, 
        context: &TaskContext, 
        strategy: Strategy,
        evaluation: StrategyEvaluation,
    ) {
        // 更新多臂老虎机
        {
            let mut bandit = self.bandit.write().await;
            bandit.update(evaluation.clone());
            
            // 逐渐降低探索
            bandit.decay_epsilon(0.99);
        }
        
        // 更新策略梯度
        {
            let mut pg = self.policy_gradient.write().await;
            let features = format!("{}:{}:{}", context.task_type, context.complexity, context.domain);
            let advantage = pg.compute_advantage(&features, evaluation.reward);
            
            tracing::debug!(
                "Task {} advantage: {:.3}, reward: {:.3}",
                context.task_type, advantage, evaluation.reward
            );
        }
        
        // 记录任务历史
        if self.task_history.len() >= self.max_history {
            self.task_history.pop_front();
        }
        self.task_history.push_back(context.clone());
    }
    
    /// 获取性能报告
    pub async fn get_performance_report(&self) -> PerformanceReport {
        let bandit = self.bandit.read().await;
        let performance = bandit.get_all_performance();
        
        let mut report = PerformanceReport {
            total_tasks: self.task_history.len(),
            best_strategy: bandit.best_strategy(),
            strategies: HashMap::new(),
            recent_performance: vec![],
        };
        
        for (strategy, success_rate, avg_reward, avg_latency) in performance {
            report.strategies.insert(
                format!("{:?}", strategy),
                StrategyStats {
                    success_rate,
                    avg_reward,
                    avg_latency_ms: avg_latency,
                }
            );
        }
        
        // 最近 10 个任务的性能趋势
        for eval in self.task_history.iter().take(10) {
            report.recent_performance.push(eval.complexity);
        }
        
        report
    }
    
    /// 重置学习状态
    pub async fn reset(&self) {
        let mut bandit = self.bandit.write().await;
        *bandit = MultiArmedBandit::new(1.5, 0.2);
        
        let mut pg = self.policy_gradient.write().await;
        *pg = PolicyGradientOptimizer::new(0.1, 0.95);
        
        self.task_history.clear();
    }
}

impl Default for AdaptiveAgentOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

/// 性能报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub total_tasks: usize,
    pub best_strategy: Option<Strategy>,
    pub strategies: HashMap<String, StrategyStats>,
    pub recent_performance: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyStats {
    pub success_rate: f32,
    pub avg_reward: f32,
    pub avg_latency_ms: f32,
}

/// 全局优化器实例
use std::sync::OnceLock;
static GLOBAL_OPTIMIZER: OnceLock<AdaptiveAgentOptimizer> = OnceLock::new();

pub fn get_global_optimizer() -> &'static AdaptiveAgentOptimizer {
    GLOBAL_OPTIMIZER.get_or_init(|| AdaptiveAgentOptimizer::new())
}

pub fn init_global_optimizer() {
    // 初始化全局优化器
    tracing::info!("Adaptive Agent Optimizer initialized");
}