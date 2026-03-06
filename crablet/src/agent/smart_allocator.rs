use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
use serde::Serialize;
use crate::agent::capability::{AgentCapability, CapabilityRouter};
use crate::agent::swarm::types::{AgentId, TaskNode};

#[async_trait]
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Vec<f32>;
}

pub struct KeywordEmbedder;

#[async_trait]
impl Embedder for KeywordEmbedder {
    async fn embed(&self, text: &str) -> Vec<f32> {
        let lower = text.to_lowercase();
        let groups = [
            vec!["code", "coding", "rust", "python", "bug", "refactor", "api", "程序", "代码"],
            vec!["research", "search", "investigate", "compare", "调研", "检索"],
            vec!["review", "qa", "test", "lint", "审核", "测试"],
            vec!["security", "auth", "vulnerability", "安全", "漏洞"],
            vec!["plan", "planning", "roadmap", "拆解", "规划", "架构"],
            vec!["analysis", "reason", "metrics", "分析", "推理", "评估"],
        ];
        let mut vec = Vec::with_capacity(groups.len());
        for words in groups {
            let count = words.iter().filter(|w| lower.contains(*w)).count() as f32;
            vec.push(count);
        }
        let norm = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            vec.into_iter().map(|v| v / norm).collect()
        } else {
            vec![0.0; 6]
        }
    }
}

#[derive(Clone)]
pub struct PerformanceMetrics {
    pub success_rate: f64,
    pub avg_latency_ms: u64,
    pub total_tasks: u64,
}

#[derive(Clone)]
pub struct RoleProfile {
    pub role: String,
    pub expertise_embedding: Vec<f32>,
    pub preferred_task_types: Vec<String>,
    pub historical_performance: PerformanceMetrics,
}

pub struct SmartTaskAllocator {
    capability_router: Arc<CapabilityRouter>,
    task_embedder: Arc<Box<dyn Embedder>>,
    role_profiles: HashMap<String, RoleProfile>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CandidateScore {
    pub agent_id: String,
    pub role: String,
    pub expertise_match: f64,
    pub ucb_bonus: f64,
    pub load_penalty: f64,
    pub performance_bonus: f64,
    pub preferred_bonus: f64,
    pub final_score: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AllocationDecision {
    pub task_id: String,
    pub selected_agent_id: String,
    pub selected_role: String,
    pub candidates: Vec<CandidateScore>,
}

impl SmartTaskAllocator {
    pub fn new(capability_router: Arc<CapabilityRouter>) -> Self {
        Self {
            capability_router,
            task_embedder: Arc::new(Box::new(KeywordEmbedder)),
            role_profiles: default_role_profiles(),
        }
    }

    pub async fn allocate(&self, task: &TaskNode, available_agents: &[AgentId]) -> AgentId {
        self.allocate_with_decision(task, available_agents).await.selected_agent()
    }

    pub async fn allocate_with_decision(&self, task: &TaskNode, available_agents: &[AgentId]) -> AllocationDecision {
        if available_agents.is_empty() {
            return AllocationDecision {
                task_id: task.id.clone(),
                selected_agent_id: task.agent_role.clone(),
                selected_role: task.agent_role.clone(),
                candidates: vec![],
            };
        }
        let task_embedding = self.task_embedder.embed(&task.prompt).await;
        let mut scored: Vec<CandidateScore> = Vec::with_capacity(available_agents.len());
        for agent in available_agents {
            let role = self.get_agent_role(agent).unwrap_or_else(|| task.agent_role.clone());
            let profile = self.role_profiles.get(&role).cloned().unwrap_or_else(|| fallback_profile(&role));
            let expertise_match = cosine_similarity(&task_embedding, &profile.expertise_embedding);
            let ucb_bonus = self.capability_router.get_exploration_bonus(&role);
            let cap = self.capability_router.get_capability(&role).unwrap_or_else(|| AgentCapability::new(&role));
            let load_penalty = cap.current_load as f64 * 0.1;
            let perf_from_router = performance_bonus(&cap);
            let perf_from_profile = performance_bonus_from_profile(&profile);
            let preferred_bonus = preferred_task_bonus(&task.prompt, &profile.preferred_task_types);
            let perf_bonus = perf_from_router.max(perf_from_profile);
            let final_score = expertise_match + ucb_bonus + perf_bonus + preferred_bonus - load_penalty;
            scored.push(CandidateScore {
                agent_id: agent.0.clone(),
                role,
                expertise_match,
                ucb_bonus,
                load_penalty,
                performance_bonus: perf_bonus,
                preferred_bonus,
                final_score,
            });
        }
        scored.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap_or(std::cmp::Ordering::Equal));
        if let Some(best) = scored.first() {
            AllocationDecision {
                task_id: task.id.clone(),
                selected_agent_id: best.agent_id.clone(),
                selected_role: best.role.clone(),
                candidates: scored,
            }
        } else {
            AllocationDecision {
                task_id: task.id.clone(),
                selected_agent_id: available_agents[0].0.clone(),
                selected_role: self.get_agent_role(&available_agents[0]).unwrap_or_else(|| task.agent_role.clone()),
                candidates: vec![],
            }
        }
    }

    pub fn suggest_candidate_roles(&self, task: &TaskNode) -> Vec<String> {
        let mut roles = Vec::new();
        if !task.agent_role.trim().is_empty() {
            roles.push(task.agent_role.clone());
        }
        let lower = task.prompt.to_lowercase();
        for (role, profile) in &self.role_profiles {
            if profile.preferred_task_types.iter().any(|kw| lower.contains(kw)) {
                roles.push(role.clone());
            }
        }
        roles.extend(self.role_profiles.keys().cloned());
        dedup_vec(roles)
    }

    pub fn get_agent_role(&self, agent: &AgentId) -> Option<String> {
        let raw = agent.0.trim();
        if raw.is_empty() {
            return None;
        }
        if let Some((left, _)) = raw.split_once(':') {
            return Some(left.to_string());
        }
        Some(raw.to_string())
    }
}

impl AllocationDecision {
    pub fn selected_agent(&self) -> AgentId {
        AgentId::from_name(&self.selected_agent_id)
    }
}

fn default_role_profiles() -> HashMap<String, RoleProfile> {
    let mut map = HashMap::new();
    map.insert("coder".to_string(), RoleProfile {
        role: "coder".to_string(),
        expertise_embedding: vec![1.0, 0.1, 0.4, 0.2, 0.2, 0.4],
        preferred_task_types: vec!["code".to_string(), "bug".to_string(), "refactor".to_string(), "api".to_string(), "代码".to_string()],
        historical_performance: PerformanceMetrics { success_rate: 0.92, avg_latency_ms: 1800, total_tasks: 1 },
    });
    map.insert("researcher".to_string(), RoleProfile {
        role: "researcher".to_string(),
        expertise_embedding: vec![0.2, 1.0, 0.3, 0.2, 0.4, 0.5],
        preferred_task_types: vec!["research".to_string(), "search".to_string(), "compare".to_string(), "调研".to_string()],
        historical_performance: PerformanceMetrics { success_rate: 0.9, avg_latency_ms: 1600, total_tasks: 1 },
    });
    map.insert("analyst".to_string(), RoleProfile {
        role: "analyst".to_string(),
        expertise_embedding: vec![0.3, 0.4, 0.5, 0.3, 0.4, 1.0],
        preferred_task_types: vec!["analysis".to_string(), "metrics".to_string(), "reason".to_string(), "分析".to_string()],
        historical_performance: PerformanceMetrics { success_rate: 0.91, avg_latency_ms: 1700, total_tasks: 1 },
    });
    map.insert("reviewer".to_string(), RoleProfile {
        role: "reviewer".to_string(),
        expertise_embedding: vec![0.4, 0.2, 1.0, 0.3, 0.3, 0.6],
        preferred_task_types: vec!["review".to_string(), "test".to_string(), "lint".to_string(), "审核".to_string()],
        historical_performance: PerformanceMetrics { success_rate: 0.93, avg_latency_ms: 1500, total_tasks: 1 },
    });
    map.insert("planner".to_string(), RoleProfile {
        role: "planner".to_string(),
        expertise_embedding: vec![0.2, 0.4, 0.2, 0.2, 1.0, 0.5],
        preferred_task_types: vec!["plan".to_string(), "roadmap".to_string(), "architecture".to_string(), "规划".to_string()],
        historical_performance: PerformanceMetrics { success_rate: 0.89, avg_latency_ms: 1400, total_tasks: 1 },
    });
    map.insert("security".to_string(), RoleProfile {
        role: "security".to_string(),
        expertise_embedding: vec![0.3, 0.2, 0.6, 1.0, 0.2, 0.5],
        preferred_task_types: vec!["security".to_string(), "auth".to_string(), "vulnerability".to_string(), "安全".to_string()],
        historical_performance: PerformanceMetrics { success_rate: 0.94, avg_latency_ms: 1900, total_tasks: 1 },
    });
    map
}

fn fallback_profile(role: &str) -> RoleProfile {
    RoleProfile {
        role: role.to_string(),
        expertise_embedding: vec![0.2, 0.2, 0.2, 0.2, 0.2, 0.2],
        preferred_task_types: vec![role.to_string()],
        historical_performance: PerformanceMetrics {
            success_rate: 0.88,
            avg_latency_ms: 2000,
            total_tasks: 1,
        },
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let mut dot = 0.0f64;
    let mut na = 0.0f64;
    let mut nb = 0.0f64;
    for i in 0..n {
        let av = a[i] as f64;
        let bv = b[i] as f64;
        dot += av * bv;
        na += av * av;
        nb += bv * bv;
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

fn preferred_task_bonus(prompt: &str, preferred: &[String]) -> f64 {
    let lower = prompt.to_lowercase();
    if preferred.iter().any(|k| lower.contains(k)) {
        0.2
    } else {
        0.0
    }
}

fn performance_bonus(cap: &AgentCapability) -> f64 {
    let success = cap.success_rate;
    let latency = (cap.avg_latency_ms as f64 / 4000.0).min(1.0);
    (success * 0.2) - (latency * 0.1)
}

fn performance_bonus_from_profile(profile: &RoleProfile) -> f64 {
    let success = profile.historical_performance.success_rate;
    let latency = (profile.historical_performance.avg_latency_ms as f64 / 4000.0).min(1.0);
    let trials = profile.historical_performance.total_tasks.max(1) as f64;
    let confidence = (trials.ln() / 5.0).clamp(0.0, 1.0);
    let role_signal = if profile.role.is_empty() { 0.0 } else { 0.01 };
    ((success * 0.2) - (latency * 0.1)) * (0.5 + confidence * 0.5) + role_signal
}

fn dedup_vec(items: Vec<String>) -> Vec<String> {
    let mut map = HashMap::new();
    for item in items {
        if !item.trim().is_empty() {
            map.entry(item.clone()).or_insert(item);
        }
    }
    map.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::swarm::types::{AgentId, TaskNode, TaskStatus};

    fn node(prompt: &str, role: &str) -> TaskNode {
        TaskNode {
            id: "t1".to_string(),
            agent_role: role.to_string(),
            prompt: prompt.to_string(),
            dependencies: vec![],
            status: TaskStatus::Pending,
            result: None,
            logs: vec![],
            priority: 128,
            timeout_ms: 30_000,
            max_retries: 1,
            retry_count: 0,
        }
    }

    #[tokio::test]
    async fn allocate_prefers_expertise() {
        let router = Arc::new(CapabilityRouter::new());
        let allocator = SmartTaskAllocator::new(router);
        let task = node("please debug rust code and refactor api", "planner");
        let chosen = allocator.allocate(
            &task,
            &[
                AgentId::from_name("coder"),
                AgentId::from_name("researcher"),
                AgentId::from_name("analyst"),
            ],
        ).await;
        assert_eq!(chosen.0, "coder");
    }

    #[tokio::test]
    async fn allocate_considers_ucb_exploration() {
        let router = Arc::new(CapabilityRouter::new());
        for _ in 0..20 {
            router.record_result("coder", true, 900);
        }
        let allocator = SmartTaskAllocator::new(router);
        let task = node("general task", "planner");
        let chosen = allocator.allocate(
            &task,
            &[
                AgentId::from_name("coder"),
                AgentId::from_name("security"),
            ],
        ).await;
        assert_eq!(chosen.0, "security");
    }

    #[tokio::test]
    async fn allocate_with_decision_contains_scores() {
        let router = Arc::new(CapabilityRouter::new());
        let allocator = SmartTaskAllocator::new(router);
        let task = node("security auth check", "planner");
        let decision = allocator
            .allocate_with_decision(
                &task,
                &[
                    AgentId::from_name("security"),
                    AgentId::from_name("coder"),
                ],
            )
            .await;
        assert!(!decision.candidates.is_empty());
        assert_eq!(decision.selected_role, decision.candidates[0].role);
    }
}
