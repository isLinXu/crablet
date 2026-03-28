//! Multi-dimensional Agent Role System
//!
//! Agent 不是单一角色，而是多维度的能力向量。
//! 每个 Agent 有一个能力向量，表示在不同维度的能力水平。
//!
//! # Architecture
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Multi-Dimensional Agent                        │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                  │
//! │  ┌─────────────────────────────────────────────────────────┐    │
//! │  │              Capability Vector                            │    │
//! │  ├─────────────────────────────────────────────────────────┤    │
//! │  │  [coding: 0.85] [reasoning: 0.92] [creativity: 0.78]   │    │
//! │  │  [research: 0.90] [analysis: 0.88] [communication: 0.95]│    │
//! │  └─────────────────────────────────────────────────────────┘    │
//! │                                                                  │
//! │  Role Composition = weighted sum of capabilities                 │
//! │                                                                  │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::agent::swarm::AgentId;

// ============================================================================
// Capability Dimensions
// ============================================================================

/// Capability dimensions for agents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapabilityDim {
    Coding,
    Reasoning,
    Creativity,
    Research,
    Analysis,
    Communication,
    Planning,
    Execution,
    Review,
    Leadership,
}

impl CapabilityDim {
    pub fn name(&self) -> &'static str {
        match self {
            CapabilityDim::Coding => "coding",
            CapabilityDim::Reasoning => "reasoning",
            CapabilityDim::Creativity => "creativity",
            CapabilityDim::Research => "research",
            CapabilityDim::Analysis => "analysis",
            CapabilityDim::Communication => "communication",
            CapabilityDim::Planning => "planning",
            CapabilityDim::Execution => "execution",
            CapabilityDim::Review => "review",
            CapabilityDim::Leadership => "leadership",
        }
    }

    pub fn all() -> Vec<CapabilityDim> {
        vec![
            CapabilityDim::Coding,
            CapabilityDim::Reasoning,
            CapabilityDim::Creativity,
            CapabilityDim::Research,
            CapabilityDim::Analysis,
            CapabilityDim::Communication,
            CapabilityDim::Planning,
            CapabilityDim::Execution,
            CapabilityDim::Review,
            CapabilityDim::Leadership,
        ]
    }
}

/// A vector of capabilities with scores
pub type CapabilityVector = HashMap<CapabilityDim, f64>;

/// Normalized capability vector (all values sum to 1.0)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedVector {
    pub values: Vec<(CapabilityDim, f64)>,
}

impl NormalizedVector {
    pub fn from_raw(raw: &CapabilityVector) -> Self {
        let total: f64 = raw.values().sum();
        if total == 0.0 {
            return Self {
                values: CapabilityDim::all()
                    .into_iter()
                    .map(|d| (d, 0.1))
                    .collect(),
            };
        }

        let values = raw
            .iter()
            .map(|(dim, score)| (*dim, *score / total))
            .collect();

        Self { values }
    }

    /// Dot product with another vector
    pub fn dot(&self, other: &NormalizedVector) -> f64 {
        let self_map: HashMap<_, _> = self.values.iter().cloned().collect();
        let other_map: HashMap<_, _> = other.values.iter().cloned().collect();

        self_map
            .iter()
            .filter_map(|(dim, v1)| other_map.get(dim).map(|v2| v1 * v2))
            .sum()
    }

    /// Euclidean distance
    pub fn distance(&self, other: &NormalizedVector) -> f64 {
        let self_map: HashMap<_, _> = self.values.iter().cloned().collect();
        let other_map: HashMap<_, _> = other.values.iter().cloned().collect();

        CapabilityDim::all()
            .iter()
            .map(|dim| {
                let v1 = self_map.get(dim).copied().unwrap_or(0.0);
                let v2 = other_map.get(dim).copied().unwrap_or(0.0);
                (v1 - v2).powi(2)
            })
            .sum::<f64>()
            .sqrt()
    }
}

// ============================================================================
// Role Definitions
// ============================================================================

/// A role defined as a weighted combination of capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDefinition {
    pub name: String,
    pub description: String,
    pub capability_weights: CapabilityVector,
}

impl RoleDefinition {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            capability_weights: HashMap::new(),
        }
    }

    pub fn with_capability(mut self, dim: CapabilityDim, weight: f64) -> Self {
        self.capability_weights.insert(dim, weight);
        self
    }

    /// Get normalized capability vector for this role
    pub fn normalized(&self) -> NormalizedVector {
        NormalizedVector::from_raw(&self.capability_weights)
    }

    /// Match score against an agent's capability vector
    pub fn match_score(&self, agent_vector: &NormalizedVector) -> f64 {
        self.normalized().dot(agent_vector)
    }
}

/// Predefined roles
impl RoleDefinition {
    pub fn researcher() -> Self {
        Self::new("researcher", "Expert at finding and synthesizing information")
            .with_capability(CapabilityDim::Research, 0.9)
            .with_capability(CapabilityDim::Analysis, 0.7)
            .with_capability(CapabilityDim::Reasoning, 0.6)
            .with_capability(CapabilityDim::Communication, 0.5)
    }

    pub fn coder() -> Self {
        Self::new("coder", "Expert at writing and debugging code")
            .with_capability(CapabilityDim::Coding, 0.9)
            .with_capability(CapabilityDim::Execution, 0.7)
            .with_capability(CapabilityDim::Reasoning, 0.6)
            .with_capability(CapabilityDim::Review, 0.5)
    }

    pub fn analyst() -> Self {
        Self::new("analyst", "Expert at analyzing problems and data")
            .with_capability(CapabilityDim::Analysis, 0.9)
            .with_capability(CapabilityDim::Reasoning, 0.8)
            .with_capability(CapabilityDim::Research, 0.6)
            .with_capability(CapabilityDim::Communication, 0.5)
    }

    pub fn reviewer() -> Self {
        Self::new("reviewer", "Expert at reviewing and critiquing work")
            .with_capability(CapabilityDim::Review, 0.9)
            .with_capability(CapabilityDim::Analysis, 0.7)
            .with_capability(CapabilityDim::Reasoning, 0.8)
            .with_capability(CapabilityDim::Communication, 0.6)
    }

    pub fn leader() -> Self {
        Self::new("leader", "Coordinates team efforts")
            .with_capability(CapabilityDim::Leadership, 0.9)
            .with_capability(CapabilityDim::Communication, 0.8)
            .with_capability(CapabilityDim::Planning, 0.7)
            .with_capability(CapabilityDim::Reasoning, 0.6)
    }

    pub fn creative() -> Self {
        Self::new("creative", "Expert at generating novel ideas")
            .with_capability(CapabilityDim::Creativity, 0.9)
            .with_capability(CapabilityDim::Planning, 0.6)
            .with_capability(CapabilityDim::Reasoning, 0.5)
            .with_capability(CapabilityDim::Communication, 0.7)
    }
}

// ============================================================================
// Multi-Dimensional Agent
// ============================================================================

/// A multi-dimensional agent with capability vector
#[derive(Debug, Clone)]
pub struct MultiDimensionalAgent {
    pub id: AgentId,
    pub name: String,
    /// Raw capability scores (not normalized)
    pub capabilities: CapabilityVector,
    /// Specializations as role compositions
    pub specializations: Vec<RoleDefinition>,
    /// Performance history
    history: Arc<RwLock<Vec<PerformanceRecord>>>,
    /// Last updated
    last_updated: DateTime<Utc>,
}

impl MultiDimensionalAgent {
    pub fn new(id: AgentId, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            capabilities: HashMap::new(),
            specializations: Vec::new(),
            history: Arc::new(RwLock::new(Vec::new())),
            last_updated: Utc::now(),
        }
    }

    /// Set capability in a dimension
    pub fn with_capability(mut self, dim: CapabilityDim, score: f64) -> Self {
        self.capabilities.insert(dim, score.clamp(0.0, 1.0));
        self
    }

    /// Add a specialization
    pub fn with_specialization(mut self, role: RoleDefinition) -> Self {
        self.specializations.push(role);
        self
    }

    /// Get normalized capability vector
    pub fn normalized_capabilities(&self) -> NormalizedVector {
        NormalizedVector::from_raw(&self.capabilities)
    }

    /// Get best matching role for this agent
    pub fn best_role(&self) -> Option<&RoleDefinition> {
        let agent_vec = self.normalized_capabilities();
        self.specializations
            .iter()
            .max_by(|a, b| {
                a.match_score(&agent_vec)
                    .partial_cmp(&b.match_score(&agent_vec))
                    .unwrap()
            })
    }

    /// Score for a specific task type
    pub fn task_score(&self, task_requirements: &NormalizedVector) -> f64 {
        let agent_vec = self.normalized_capabilities();
        agent_vec.dot(task_requirements)
    }

    /// Update capability based on performance (synchronous)
    pub fn learn(&mut self, dim: CapabilityDim, feedback: f64) {
        // Simple exponential moving average update
        let current = self.capabilities.get(&dim).copied().unwrap_or(0.5);
        let alpha = 0.1; // Learning rate
        let new_score = current + alpha * (feedback - current);
        self.capabilities.insert(dim, new_score.clamp(0.0, 1.0));
        self.last_updated = Utc::now();
    }

    /// Record performance for learning
    pub async fn record_performance(&self, task_type: &str, success: bool, quality: f64) {
        let mut history = self.history.write().await;
        history.push(PerformanceRecord {
            task_type: task_type.to_string(),
            success,
            quality,
            timestamp: Utc::now(),
        });

        // Keep only last 100 records
        if history.len() > 100 {
            history.remove(0);
        }
    }

    /// Get capability vector as map
    pub fn as_map(&self) -> HashMap<String, f64> {
        self.capabilities
            .iter()
            .map(|(k, v)| (k.name().to_string(), *v))
            .collect()
    }
}

/// Record of past performance
#[derive(Debug, Clone)]
pub struct PerformanceRecord {
    pub task_type: String,
    pub success: bool,
    pub quality: f64,
    pub timestamp: DateTime<Utc>,
}

// ============================================================================
// Role Assignment
// ============================================================================

/// Task requirements as capability vector
#[derive(Debug, Clone)]
pub struct TaskRequirements {
    pub name: String,
    pub capability_weights: CapabilityVector,
    pub priority: f64,
}

impl TaskRequirements {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            capability_weights: HashMap::new(),
            priority: 1.0,
        }
    }

    pub fn with_weight(mut self, dim: CapabilityDim, weight: f64) -> Self {
        self.capability_weights.insert(dim, weight);
        self
    }

    pub fn normalized(&self) -> NormalizedVector {
        NormalizedVector::from_raw(&self.capability_weights)
    }
}

/// Role assignment result
#[derive(Debug, Clone)]
pub struct RoleAssignment {
    pub agent_id: AgentId,
    pub role: RoleDefinition,
    pub match_score: f64,
    pub reasoning: String,
}

// ============================================================================
// Multi-Role Manager
// ============================================================================

/// Manages multi-dimensional agents and role assignments
pub struct MultiRoleManager {
    agents: HashMap<AgentId, Arc<MultiDimensionalAgent>>,
    roles: Vec<RoleDefinition>,
}

impl MultiRoleManager {
    pub fn new() -> Self {
        let mut roles = Vec::new();
        roles.push(RoleDefinition::researcher());
        roles.push(RoleDefinition::coder());
        roles.push(RoleDefinition::analyst());
        roles.push(RoleDefinition::reviewer());
        roles.push(RoleDefinition::leader());
        roles.push(RoleDefinition::creative());

        Self {
            agents: HashMap::new(),
            roles,
        }
    }

    /// Register an agent
    pub fn register(&mut self, agent: Arc<MultiDimensionalAgent>) {
        self.agents.insert(agent.id.clone(), agent);
    }

    /// Get an agent
    pub fn get(&self, id: &AgentId) -> Option<&Arc<MultiDimensionalAgent>> {
        self.agents.get(id)
    }

    /// Find best agent for a task
    pub fn find_best_agent(&self, task: &TaskRequirements) -> Option<RoleAssignment> {
        let task_vec = task.normalized();

        let candidates: Vec<_> = self
            .agents
            .values()
            .map(|agent| {
                let score = agent.task_score(&task_vec);
                let best_role = agent
                    .specializations
                    .iter()
                    .max_by(|a, b| {
                        a.match_score(&task_vec)
                            .partial_cmp(&b.match_score(&task_vec))
                            .unwrap()
                    })
                    .cloned();

                (agent.as_ref().clone(), score, best_role)
            })
            .collect();

        candidates
            .iter()
            .max_by(|(_, score_a, _), (_, score_b, _)| {
                score_a.partial_cmp(score_b).unwrap()
            })
            .and_then(|(agent, score, best_role)| {
                best_role.as_ref().map(|role| RoleAssignment {
                    agent_id: agent.id.clone(),
                    role: role.clone(),
                    match_score: *score,
                    reasoning: format!(
                        "{} matched with {} (score: {:.2})",
                        agent.name, role.name, score
                    ),
                })
            })
    }

    /// Assign multiple agents to a task based on role composition
    pub fn assign_team(&self, task: &TaskRequirements, size: usize) -> Vec<RoleAssignment> {
        let task_vec = task.normalized();
        let mut assignments = Vec::new();
        let mut used_agents: HashMap<AgentId, f64> = HashMap::new();

        for _ in 0..size {
            let remaining: Vec<_> = self
                .agents
                .iter()
                .filter(|(id, _)| !used_agents.contains_key(*id))
                .map(|(id, agent)| (id.clone(), agent.clone()))
                .collect();

            if remaining.is_empty() {
                break;
            }

            if let Some((agent_id, agent)) = remaining
                .iter()
                .max_by(|(_, a), (_, b)| {
                    let score_a = a.task_score(&task_vec);
                    let score_b = b.task_score(&task_vec);
                    score_a.partial_cmp(&score_b).unwrap()
                })
            {
                let score = agent.task_score(&task_vec);
                let best_role = agent
                    .specializations
                    .iter()
                    .max_by(|a, b| {
                        a.match_score(&task_vec)
                            .partial_cmp(&b.match_score(&task_vec))
                            .unwrap()
                    })
                    .cloned();

                if let Some(role) = best_role {
                    assignments.push(RoleAssignment {
                        agent_id: agent_id.clone(),
                        role: role.clone(),
                        match_score: score,
                        reasoning: format!("Team member: {} as {}", agent.name, role.name),
                    });
                    used_agents.insert(agent_id.clone(), score);
                }
            }
        }

        assignments
    }

    /// Add a custom role
    pub fn add_role(&mut self, role: RoleDefinition) {
        self.roles.push(role);
    }
}

impl Default for MultiRoleManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Global Manager
// ============================================================================

static MULTI_ROLE_MANAGER: std::sync::OnceLock<Arc<RwLock<MultiRoleManager>>> =
    std::sync::OnceLock::new();

/// Get global multi-role manager
pub fn global_manager() -> &'static Arc<RwLock<MultiRoleManager>> {
    MULTI_ROLE_MANAGER.get_or_init(|| Arc::new(RwLock::new(MultiRoleManager::new())))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalized_vector() {
        let mut raw = HashMap::new();
        raw.insert(CapabilityDim::Coding, 0.8);
        raw.insert(CapabilityDim::Reasoning, 0.6);
        raw.insert(CapabilityDim::Creativity, 0.2);

        let norm = NormalizedVector::from_raw(&raw);
        let sum: f64 = norm.values.iter().map(|(_, v)| v).sum();

        // Should sum to approximately 1.0
        assert!((sum - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_dot_product() {
        let mut raw1 = HashMap::new();
        raw1.insert(CapabilityDim::Coding, 0.8);
        raw1.insert(CapabilityDim::Reasoning, 0.6);

        let mut raw2 = HashMap::new();
        raw2.insert(CapabilityDim::Coding, 0.9);
        raw2.insert(CapabilityDim::Reasoning, 0.7);

        let norm1 = NormalizedVector::from_raw(&raw1);
        let norm2 = NormalizedVector::from_raw(&raw2);

        let dot = norm1.dot(&norm2);
        assert!(dot > 0.0);
    }

    #[test]
    fn test_role_definition() {
        let researcher = RoleDefinition::researcher();
        assert_eq!(researcher.name, "researcher");
        assert!(researcher.capability_weights.contains_key(&CapabilityDim::Research));
    }

    #[test]
    fn test_agent_learning() {
        let mut agent = MultiDimensionalAgent::new(AgentId("test".into()), "Test Agent")
            .with_capability(CapabilityDim::Coding, 0.5);

        // Learn from positive feedback
        agent.learn(CapabilityDim::Coding, 0.8);

        let score = agent.capabilities.get(&CapabilityDim::Coding).unwrap();
        assert!(*score > 0.5);
    }
}