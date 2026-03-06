use dashmap::DashMap;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
#[allow(unused_imports)]
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapability {
    pub agent_role: String,
    pub domain_scores: HashMap<String, f64>,  // Domain -> Score (0.0 - 1.0)
    pub success_rate: f64,                    // Moving average
    pub total_tasks: u64,
    pub success_count: u64,
    pub avg_latency_ms: u64,
    pub current_load: i64,                    // Current active tasks
}

impl AgentCapability {
    pub fn new(role: &str) -> Self {
        Self {
            agent_role: role.to_string(),
            domain_scores: HashMap::new(),
            success_rate: 1.0, // Optimistic start
            total_tasks: 0,
            success_count: 0,
            avg_latency_ms: 0,
            current_load: 0,
        }
    }

    pub fn update_metrics(&mut self, success: bool, latency_ms: u64) {
        self.total_tasks += 1;
        if success {
            self.success_count += 1;
        }
        self.success_rate = self.success_count as f64 / self.total_tasks as f64;
        
        // Simple moving average for latency
        if self.avg_latency_ms == 0 {
            self.avg_latency_ms = latency_ms;
        } else {
            self.avg_latency_ms = (self.avg_latency_ms * 9 + latency_ms) / 10;
        }
    }
}

pub struct CapabilityRouter {
    capabilities: Arc<DashMap<String, AgentCapability>>, // role -> Capability
}

impl Default for CapabilityRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl CapabilityRouter {
    pub fn new() -> Self {
        let caps = DashMap::new();
        // Initialize default roles
        caps.insert("researcher".to_string(), AgentCapability::new("researcher"));
        caps.insert("coder".to_string(), AgentCapability::new("coder"));
        caps.insert("analyst".to_string(), AgentCapability::new("analyst"));
        caps.insert("reviewer".to_string(), AgentCapability::new("reviewer"));
        caps.insert("planner".to_string(), AgentCapability::new("planner"));
        caps.insert("security".to_string(), AgentCapability::new("security"));
        
        Self {
            capabilities: Arc::new(caps),
        }
    }

    // Multi-Armed Bandit (UCB1) inspired selection
    // Since we select by Role (and we assume one agent pool per role for now, 
    // or we select *which role* fits a task description? 
    // Usually the task comes with a required role or we need to route to the best instance of that role.
    // Assuming we have multiple agents per role or we are selecting the role itself?
    // The prompt says "Dynamic Team Assembly... CapabilityRouter select_agent".
    // If we have multiple agents for "coder", we pick one. 
    // But currently SwarmOrchestrator creates agents on the fly via factory.
    // So this might be selecting the *Role* for a vague task, or selecting a specific *Profile/Persona* if we had them.
    // For Crablet v3 currently, we have Roles.
    // Let's assume we use this to track metrics PER ROLE to decide if we should fallback or use a different role?
    // OR, better: We might want to select between "Senior Coder" vs "Junior Coder" if we had them.
    // For now, let's implement it as a Metrics Tracker & Router for Roles.
    
    pub fn select_best_role(&self, _task_description: &str, candidates: &[String]) -> Option<String> {
        if self.total_ops() == 0 {
            return candidates.first().cloned();
        }
        
        let mut best_score = -1.0;
        let mut best_role = None;

        for role in candidates {
            if let Some(cap) = self.capabilities.get(role) {
                let exploration_bonus = self.get_exploration_bonus(role);
                
                let load_penalty = cap.current_load as f64 * 0.1;
                
                let score = cap.success_rate + exploration_bonus - load_penalty;
                
                if score > best_score {
                    best_score = score;
                    best_role = Some(role.clone());
                }
            }
        }
        
        best_role
    }
    
    pub fn update_load(&self, role: &str, delta: i64) {
        if let Some(mut cap) = self.capabilities.get_mut(role) {
            cap.current_load += delta;
        }
    }
    
    pub fn record_result(&self, role: &str, success: bool, latency_ms: u64) {
        if let Some(mut cap) = self.capabilities.get_mut(role) {
            cap.update_metrics(success, latency_ms);
        }
    }
    
    pub fn get_stats(&self) -> HashMap<String, AgentCapability> {
        self.capabilities.iter().map(|r| (r.key().clone(), r.value().clone())).collect()
    }

    pub fn get_capability(&self, role: &str) -> Option<AgentCapability> {
        self.capabilities.get(role).map(|c| c.clone())
    }

    pub fn get_exploration_bonus(&self, role: &str) -> f64 {
        let total_ops = self.total_ops();
        if let Some(cap) = self.capabilities.get(role) {
            if cap.total_tasks == 0 {
                return 2.5;
            }
            if total_ops <= 1 {
                return 0.0;
            }
            return (2.0 * (total_ops as f64).ln() / cap.total_tasks as f64).sqrt();
        }
        0.0
    }

    fn total_ops(&self) -> u64 {
        self.capabilities.iter().map(|c| c.total_tasks).sum()
    }
}
