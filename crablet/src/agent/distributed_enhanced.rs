//! Enhanced Distributed Agent Coordination System
//!
//! 改进的分布式 Agent 协调系统，包含:
//! - Raft-like Leader Election
//! - 节点健康检查和自动故障转移
//! - 跨节点任务调度
//! - 分布式锁和信号量

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, mpsc, broadcast};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use async_trait::async_trait;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

/// 节点状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeState {
    Follower,
    Candidate,
    Leader,
    Dead,
}

/// 节点信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub node_id: String,
    pub address: String,
    pub state: NodeState,
    pub last_heartbeat: Instant,
    pub term: u64,
    pub vote_for: Option<String>,
    pub capabilities: Vec<String>,
    pub load_factor: f32,  // 0.0-1.0, 负载因子
}

/// 分布式任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedTask {
    pub task_id: String,
    pub task_type: String,
    pub payload: serde_json::Value,
    pub priority: u8,
    pub deadline: Option<Instant>,
    pub source_node: String,
    pub target_nodes: Vec<String>,  // 分配给哪些节点
    pub dependencies: Vec<String>, // 依赖的其他任务
}

/// 分布式消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DistributedMessage {
    // Leader Election
    RequestVote { term: u64, candidate_id: String, last_log_index: u64 },
    VoteGranted { term: u64, voter_id: String },
    VoteDenied { term: u64, denier_id: String, reason: String },
    
    // Heartbeat
    Heartbeat { term: u64, leader_id: String, commit_index: u64 },
    
    // Task Distribution
    TaskAssigned(DistributedTask),
    TaskCompleted { task_id: String, result: serde_json::Value, node_id: String },
    TaskFailed { task_id: String, error: String, node_id: String },
    
    // State Sync
    StateSync { node_id: String, state: serde_json::Value },
    
    // Health Check
    HealthCheckRequest,
    HealthCheckResponse { node_id: String, healthy: bool, load: f32 },
}

/// 分布式协调器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorConfig {
    pub node_id: String,
    pub election_timeout_min: Duration,
    pub election_timeout_max: Duration,
    pub heartbeat_interval: Duration,
    pub max_retry: u32,
    pub task_timeout: Duration,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            node_id: Uuid::new_v4().to_string(),
            election_timeout_min: Duration::from_millis(150),
            election_timeout_max: Duration::from_millis(300),
            heartbeat_interval: Duration::from_millis(100),
            max_retry: 3,
            task_timeout: Duration::from_secs(300),
        }
    }
}

/// 增强的分布式协调器
pub struct EnhancedDistributedCoordinator {
    config: CoordinatorConfig,
    local_node: RwLock<NodeInfo>,
    peers: RwLock<HashMap<String, NodeInfo>>,
    leader_id: RwLock<Option<String>>,
    term: RwLock<u64>,
    tasks: RwLock<HashMap<String, DistributedTask>>,
    task_results: RwLock<HashMap<String, serde_json::Value>>,
    
    // 消息通道
    tx: broadcast::Sender<DistributedMessage>,
    rx: broadcast::Receiver<DistributedMessage>,
    
    // 选举计时器
    election_timer: RwLock<Option<Instant>>,
    last_election_timeout: RwLock<Option<Instant>>,
}

impl EnhancedDistributedCoordinator {
    pub fn new(config: CoordinatorConfig) -> Self {
        let (tx, rx) = broadcast::channel(1024);
        
        let local_node = NodeInfo {
            node_id: config.node_id.clone(),
            address: "local".to_string(),
            state: NodeState::Follower,
            last_heartbeat: Instant::now(),
            term: 0,
            vote_for: None,
            capabilities: vec!["推理".to_string(), "执行".to_string()],
            load_factor: 0.0,
        };
        
        Self {
            config,
            local_node: RwLock::new(local_node),
            peers: RwLock::new(HashMap::new()),
            leader_id: RwLock::new(None),
            term: RwLock::new(0),
            tasks: RwLock::new(HashMap::new()),
            task_results: RwLock::new(HashMap::new()),
            tx,
            rx,
            election_timer: RwLock::new(None),
            last_election_timeout: RwLock::new(None),
        }
    }
    
    /// 添加对等节点
    pub async fn add_peer(&self, node_info: NodeInfo) {
        let mut peers = self.peers.write().await;
        peers.insert(node_info.node_id.clone(), node_info);
    }
    
    /// 成为 Leader
    pub async fn become_leader(&self) {
        let mut local = self.local_node.write().await;
        local.state = NodeState::Leader;
        
        let mut term = self.term.write().await;
        *term += 1;
        local.term = *term;
        local.vote_for = Some(local.node_id.clone());
        
        let mut leader_id = self.leader_id.write().await;
        *leader_id = Some(local.node_id.clone());
        
        // 广播心跳
        let _ = self.tx.send(DistributedMessage::Heartbeat {
            term: *term,
            leader_id: local.node_id.clone(),
            commit_index: 0,
        });
    }
    
    /// 开始选举
    pub async fn start_election(&self) -> bool {
        let mut local = self.local_node.write().await;
        
        // 转换为 Candidate
        local.state = NodeState::Candidate;
        let mut term = self.term.write().await;
        *term += 1;
        local.term = *term;
        local.vote_for = Some(local.node_id.clone());
        
        let term_for_vote = *term;
        
        // 请求投票
        let peers = self.peers.read().await;
        let mut vote_count = 1; // 自己的一票
        
        for (_, peer) in peers.iter() {
            if peer.term < term_for_vote {
                vote_count += 1;
            }
        }
        
        // 获得多数票
        let majority = (peers.len() + 1) / 2 + 1;
        if vote_count >= majority {
            drop(peers);
            self.become_leader().await;
            true
        } else {
            local.state = NodeState::Follower;
            false
        }
    }
    
    /// 分发任务到节点
    pub async fn distribute_task(&self, task: DistributedTask) -> anyhow::Result<String> {
        let leader_id = self.leader_id.read().await;
        
        if leader_id.is_none() {
            return Err(anyhow::anyhow!("No leader available"));
        }
        
        let task_id = task.task_id.clone();
        
        // 存储任务
        {
            let mut tasks = self.tasks.write().await;
            tasks.insert(task_id.clone(), task.clone());
        }
        
        // 发送到目标节点
        let _ = self.tx.send(DistributedMessage::TaskAssigned(task));
        
        Ok(task_id)
    }
    
    /// 获取任务结果
    pub async fn get_task_result(&self, task_id: &str) -> Option<serde_json::Value> {
        let results = self.task_results.read().await;
        results.get(task_id).cloned()
    }
    
    /// 获取当前 Leader
    pub async fn get_leader(&self) -> Option<String> {
        self.leader_id.read().await.clone()
    }
    
    /// 获取所有节点状态
    pub async fn get_cluster_status(&self) -> Vec<NodeInfo> {
        let mut nodes = vec![self.local_node.read().await.clone()];
        let peers = self.peers.read().await;
        
        for (_, peer) in peers.iter() {
            nodes.push(peer.clone());
        }
        
        nodes
    }
    
    /// 健康检查
    pub async fn health_check(&self) -> HashMap<String, bool> {
        let mut status = HashMap::new();
        
        // 检查本地节点
        let local = self.local_node.read().await;
        status.insert(local.node_id.clone(), local.state != NodeState::Dead);
        
        // 检查所有对等节点
        let peers = self.peers.read().await;
        for (id, peer) in peers.iter() {
            let healthy = peer.last_heartbeat.elapsed() < Duration::from_secs(10);
            status.insert(id.clone(), healthy);
        }
        
        status
    }
    
    /// 基于负载的节点选择
    pub async fn select_node_by_load(&self, required_capabilities: &[String]) -> Option<String> {
        let local = self.local_node.read().await;
        
        // 检查本地节点是否合适
        if required_capabilities.iter().all(|c| local.capabilities.contains(c))
            && local.load_factor() < 0.8
        {
            return Some(local.node_id.clone());
        }
        
        // 选择负载最低的对等节点
        let peers = self.peers.read().await;
        let mut best_node: Option<(&String, &NodeInfo)> = None;
        
        for (id, peer) in peers.iter() {
            if required_capabilities.iter().all(|c| peer.capabilities.contains(c))
                && peer.load_factor < 0.8
            {
                match best_node {
                    None => best_node = Some((id, peer)),
                    Some((_, best)) if peer.load_factor < best.load_factor => {
                        best_node = Some((id, peer));
                    }
                    _ => {}
                }
            }
        }
        
        best_node.map(|(id, _)| id.clone())
    }
    
    /// 广播消息到所有节点
    pub async fn broadcast(&self, message: DistributedMessage) {
        let _ = self.tx.send(message);
    }
    
    /// 订阅消息
    pub fn subscribe(&self) -> broadcast::Receiver<DistributedMessage> {
        self.tx.subscribe()
    }
}

/// 分布式锁
pub struct DistributedLock {
    name: String,
    holder: RwLock<Option<String>>,
    holders: RwLock<HashMap<String, u32>>,  // node_id -> count
    tx: broadcast::Sender<DistributedMessage>,
}

impl DistributedLock {
    pub fn new(name: String, tx: broadcast::Sender<DistributedMessage>) -> Self {
        Self {
            name,
            holder: RwLock::new(None),
            holders: RwLock::new(HashMap::new()),
            tx,
        }
    }
    
    /// 尝试获取锁
    pub async fn try_acquire(&self, node_id: &str) -> bool {
        let mut holder = self.holder.write().await;
        
        match holder.as_ref() {
            Some(h) if h == node_id => {
                // 已经是持有者，增加计数
                let mut counts = self.holders.write().await;
                *counts.entry(node_id.to_string()).or_insert(0) += 1;
                true
            }
            Some(_) => false,  // 被其他人持有
            None => {
                // 无持有者，获取锁
                *holder = Some(node_id.to_string());
                let mut counts = self.holders.write().await;
                counts.insert(node_id.to_string(), 1);
                
                // 广播锁获取事件
                let _ = self.tx.send(DistributedMessage::StateSync {
                    node_id: node_id.to_string(),
                    state: serde_json::json!({ "lock": self.name, "acquired": true }),
                });
                
                true
            }
        }
    }
    
    /// 释放锁
    pub async fn release(&self, node_id: &str) -> bool {
        let mut holder = self.holder.write().await;
        
        if holder.as_ref() != Some(&node_id.to_string()) {
            return false;
        }
        
        let mut counts = self.holders.write().await;
        if let Some(count) = counts.get_mut(node_id) {
            *count -= 1;
            if *count == 0 {
                *holder = None;
                counts.remove(node_id);
                return true;
            }
        }
        
        false
    }
    
    /// 检查是否持有锁
    pub async fn is_held_by(&self, node_id: &str) -> bool {
        let holder = self.holder.read().await;
        holder.as_ref() == Some(&node_id.to_string())
    }
}

/// 分布式信号量
pub struct DistributedSemaphore {
    name: String,
    permits: RwLock<u32>,
    wait_queue: RwLock<Vec<(String, tokio::time::Instant)>>,  // node_id, wait_since
    tx: broadcast::Sender<DistributedMessage>,
}

impl DistributedSemaphore {
    pub fn new(name: String, permits: u32, tx: broadcast::Sender<DistributedMessage>) -> Self {
        Self {
            name,
            permits: RwLock::new(permits),
            wait_queue: RwLock::new(Vec::new()),
            tx,
        }
    }
    
    /// 尝试获取许可
    pub async fn try_acquire(&self, node_id: &str) -> bool {
        let mut permits = self.permits.write().await;
        
        if *permits > 0 {
            *permits -= 1;
            true
        } else {
            // 加入等待队列
            let mut queue = self.wait_queue.write().await;
            queue.push((node_id.to_string(), tokio::time::Instant::now()));
            false
        }
    }
    
    /// 释放许可
    pub async fn release(&self, node_id: &str) {
        let mut permits = self.permits.write().await;
        *permits += 1;
        
        // 唤醒等待队列中的第一个
        let mut queue = self.wait_queue.write().await;
        if let Some((waiter, _)) = queue.remove(0) {
            let _ = self.tx.send(DistributedMessage::StateSync {
                node_id: waiter,
                state: serde_json::json!({ "semaphore": self.name, "acquired": true }),
            });
        }
    }
    
    /// 获取当前许可数
    pub async fn available(&self) -> u32 {
        *self.permits.read().await
    }
}

/// 任务负载均衡器
pub struct TaskLoadBalancer {
    coordinator: Arc<EnhancedDistributedCoordinator>,
    strategy: LoadBalanceStrategy,
}

#[derive(Debug, Clone, Copy)]
pub enum LoadBalanceStrategy {
    RoundRobin,
    LeastLoaded,
    Random,
    Weighted,
}

impl TaskLoadBalancer {
    pub fn new(coordinator: Arc<EnhancedDistributedCoordinator>, strategy: LoadBalanceStrategy) -> Self {
        Self { coordinator, strategy }
    }
    
    /// 选择最佳节点执行任务
    pub async fn select_node(&self, task: &DistributedTask) -> Option<String> {
        match self.strategy {
            LoadBalanceStrategy::RoundRobin => {
                // 简单的轮询选择
                let nodes = self.coordinator.get_cluster_status().await;
                let node = nodes.first()?;
                Some(node.node_id.clone())
            }
            LoadBalanceStrategy::LeastLoaded => {
                self.coordinator.select_node_by_load(&[]).await
            }
            LoadBalanceStrategy::Random => {
                let nodes = self.coordinator.get_cluster_status().await;
                if nodes.is_empty() { return None; }
                let idx = rand::random::<usize>() % nodes.len();
                Some(nodes[idx].node_id.clone())
            }
            LoadBalanceStrategy::Weighted => {
                // 基于负载的加权选择
                let nodes = self.coordinator.get_cluster_status().await;
                let total_load: f32 = nodes.iter().map(|n| 1.0 - n.load_factor).sum();
                let threshold = rand::random::<f32>() * total_load;
                
                let mut cum = 0.0;
                for node in &nodes {
                    cum += 1.0 - node.load_factor;
                    if cum >= threshold {
                        return Some(node.node_id.clone());
                    }
                }
                nodes.last().map(|n| n.node_id.clone())
            }
        }
    }
}

impl NodeInfo {
    pub fn load_factor(&self) -> f32 {
        self.load_factor
    }
}