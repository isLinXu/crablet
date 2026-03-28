//! Collaborative Problem Solving - ThoughtTeam
//!
//! Multiple agents form a "thought team" to collaboratively solve problems
//! like humans do. Implements various coordination protocols.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::agent::swarm::AgentId;

// ============================================================================
// Team Roles & Structure
// ============================================================================

/// Role within a thought team
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TeamRole {
    Lead,
    Critic,
    Specialist,
    Synthesizer,
}

impl TeamRole {
    pub fn name(&self) -> &'static str {
        match self {
            TeamRole::Lead => "Lead",
            TeamRole::Critic => "Critic",
            TeamRole::Specialist => "Specialist",
            TeamRole::Synthesizer => "Synthesizer",
        }
    }

    pub fn prompt_suffix(&self) -> &'static str {
        match self {
            TeamRole::Lead => "You are the team lead. Coordinate the discussion and drive towards a solution.",
            TeamRole::Critic => "You are the critic. Challenge assumptions and identify potential flaws.",
            TeamRole::Specialist => "You are a specialist. Provide deep domain expertise.",
            TeamRole::Synthesizer => "You are the synthesizer. Combine insights into coherent solutions.",
        }
    }
}

/// A member of the thought team
#[derive(Debug, Clone)]
pub struct ThoughtTeamMember {
    pub id: AgentId,
    pub role: TeamRole,
    pub specialization: String,
    pub contribution_count: u32,
    pub last_active: DateTime<Utc>,
}

impl ThoughtTeamMember {
    pub fn new(id: AgentId, role: TeamRole, specialization: impl Into<String>) -> Self {
        Self {
            id,
            role,
            specialization: specialization.into(),
            contribution_count: 0,
            last_active: Utc::now(),
        }
    }

    pub fn record_contribution(&mut self) {
        self.contribution_count += 1;
        self.last_active = Utc::now();
    }
}

/// Thought team member with full context
#[derive(Debug, Clone)]
pub struct TeamMemberContext {
    pub member: ThoughtTeamMember,
    pub contributions: Vec<Contribution>,
}

impl TeamMemberContext {
    pub fn new(member: ThoughtTeamMember) -> Self {
        Self {
            member,
            contributions: Vec::new(),
        }
    }
}

/// A contribution from a team member
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contribution {
    pub id: String,
    pub member_id: AgentId,
    pub content: String,
    pub contribution_type: ContributionType,
    pub timestamp: DateTime<Utc>,
    pub importance: f32,
}

impl Contribution {
    pub fn new(member_id: AgentId, content: String, contribution_type: ContributionType) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static ID_COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = format!("contrib_{}", ID_COUNTER.fetch_add(1, Ordering::Relaxed));

        Self {
            id,
            member_id,
            content,
            contribution_type,
            timestamp: Utc::now(),
            importance: 0.5,
        }
    }
}

/// Type of contribution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContributionType {
    Analysis,
    Proposal,
    Criticism,
    Build,
    Synthesis,
    Question,
}

// ============================================================================
// Coordination Protocols
// ============================================================================

/// How the team coordinates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoordinationProtocol {
    Brainstorm,
    Review,
    PairProgramming,
    Debate,
}

impl CoordinationProtocol {
    pub fn name(&self) -> &'static str {
        match self {
            CoordinationProtocol::Brainstorm => "Brainstorm",
            CoordinationProtocol::Review => "Review",
            CoordinationProtocol::PairProgramming => "Pair Programming",
            CoordinationProtocol::Debate => "Debate",
        }
    }

    pub fn max_rounds(&self) -> u32 {
        match self {
            CoordinationProtocol::Brainstorm => 5,
            CoordinationProtocol::Review => 3,
            CoordinationProtocol::PairProgramming => 4,
            CoordinationProtocol::Debate => 6,
        }
    }
}

// ============================================================================
// Shared Workspace
// ============================================================================

/// Shared workspace for team collaboration
#[derive(Debug, Clone, Default)]
pub struct SharedWorkspace {
    pub shared_context: Arc<RwLock<HashMap<String, String>>>,
    pub message_pool: Arc<RwLock<Vec<Contribution>>>,
    pub knowledge_base: Arc<RwLock<KnowledgeBase>>,
    pub current_round: u32,
    pub total_contributions: Arc<AtomicU32>,
}

impl SharedWorkspace {
    pub fn new() -> Self {
        Self {
            shared_context: Arc::new(RwLock::new(HashMap::new())),
            message_pool: Arc::new(RwLock::new(Vec::new())),
            knowledge_base: Arc::new(RwLock::new(KnowledgeBase::new())),
            current_round: 0,
            total_contributions: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Add a contribution to the workspace
    pub async fn add_contribution(&self, contribution: Contribution) {
        let mut pool = self.message_pool.write().await;
        pool.push(contribution.clone());
        drop(pool);

        if contribution.contribution_type == ContributionType::Synthesis
            || contribution.importance > 0.7
        {
            let mut kb = self.knowledge_base.write().await;
            kb.add_insight(&contribution.content, contribution.importance);
        }

        self.total_contributions.fetch_add(1, Ordering::SeqCst);
    }

    /// Get recent contributions
    pub async fn get_recent_contributions(&self, limit: usize) -> Vec<Contribution> {
        let pool = self.message_pool.read().await;
        pool.iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    /// Advance to next round
    pub async fn advance_round(&mut self) {
        self.current_round += 1;
    }

    /// Set shared context value
    pub async fn set_context(&self, key: &str, value: &str) {
        let mut ctx = self.shared_context.write().await;
        ctx.insert(key.to_string(), value.to_string());
    }

    /// Get shared context value
    pub async fn get_context(&self, key: &str) -> Option<String> {
        let ctx = self.shared_context.read().await;
        ctx.get(key).cloned()
    }
}

/// Simple knowledge base for shared insights
#[derive(Debug, Clone, Default)]
pub struct KnowledgeBase {
    insights: Vec<Insight>,
}

impl KnowledgeBase {
    pub fn new() -> Self {
        Self { insights: Vec::new() }
    }

    pub fn add_insight(&mut self, content: &str, importance: f32) {
        self.insights.push(Insight {
            content: content.to_string(),
            importance,
            created_at: Utc::now(),
        });
    }

    pub fn get_top_insights(&self, limit: usize) -> Vec<&Insight> {
        let mut sorted = self.insights.iter().collect::<Vec<_>>();
        sorted.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap());
        sorted.into_iter().take(limit).collect()
    }
}

#[derive(Debug, Clone)]
pub struct Insight {
    pub content: String,
    pub importance: f32,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// Thought Team
// ============================================================================

/// Configuration for thought team
#[derive(Debug, Clone)]
pub struct ThoughtTeamConfig {
    pub protocol: CoordinationProtocol,
    pub max_round: Option<u32>,
    pub timeout_per_round_ms: u64,
    pub consensus_threshold: f32,
    pub enable_specialists: bool,
}

impl Default for ThoughtTeamConfig {
    fn default() -> Self {
        Self {
            protocol: CoordinationProtocol::Brainstorm,
            max_round: None,
            timeout_per_round_ms: 30000,
            consensus_threshold: 0.7,
            enable_specialists: true,
        }
    }
}

/// A collaborative problem solving team
pub struct ThoughtTeam {
    pub id: String,
    pub config: ThoughtTeamConfig,
    pub members: Vec<ThoughtTeamMember>,
    pub member_roles: HashMap<TeamRole, usize>,
    pub workspace: SharedWorkspace,
    pub status: TeamStatus,
    pub created_at: DateTime<Utc>,
}

impl ThoughtTeam {
    /// Create a new thought team with default members
    pub fn new(id: impl Into<String>, config: ThoughtTeamConfig) -> Self {
        let team_id = id.into();
        let members = vec![
            ThoughtTeamMember::new(
                AgentId(format!("{}_lead", team_id)),
                TeamRole::Lead,
                "Coordination",
            ),
            ThoughtTeamMember::new(
                AgentId(format!("{}_critic", team_id)),
                TeamRole::Critic,
                "Critical Analysis",
            ),
            ThoughtTeamMember::new(
                AgentId(format!("{}_synthesizer", team_id)),
                TeamRole::Synthesizer,
                "Synthesis",
            ),
        ];

        let mut member_roles = HashMap::new();
        member_roles.insert(TeamRole::Lead, 0);
        member_roles.insert(TeamRole::Critic, 1);
        member_roles.insert(TeamRole::Synthesizer, 2);

        Self {
            id: team_id,
            config,
            members,
            member_roles,
            workspace: SharedWorkspace::new(),
            status: TeamStatus::Initialized,
            created_at: Utc::now(),
        }
    }

    /// Add a specialist member
    pub fn add_specialist(&mut self, domain: &str) {
        let idx = self.members.len();
        let id = format!("{}_specialist_{}", self.id, idx);
        let member = ThoughtTeamMember::new(
            AgentId(id),
            TeamRole::Specialist,
            domain,
        );
        self.member_roles.insert(TeamRole::Specialist, idx);
        self.members.push(member);
    }

    /// Get member index by role
    fn member_index_by_role(&self, role: TeamRole) -> Option<usize> {
        self.member_roles.get(&role).copied()
    }

    /// Execute a round of collaboration
    pub async fn execute_round(&mut self, _prompt: &str) -> RoundResult {
        self.status = TeamStatus::InProgress;

        let round = self.workspace.current_round + 1;
        let mut round_results = Vec::new();

        // Determine participants based on protocol
        let participant_indices = self.get_participant_indices(round);

        for &idx in &participant_indices {
            let member = &self.members[idx];
            let contribution_type = match member.role {
                TeamRole::Lead => ContributionType::Analysis,
                TeamRole::Critic => ContributionType::Criticism,
                TeamRole::Specialist => ContributionType::Proposal,
                TeamRole::Synthesizer => ContributionType::Synthesis,
            };

            let content = format!(
                "[{}] {} contributes: Analysis of the task based on {} perspective",
                member.role.name(),
                member.id.0,
                member.specialization
            );

            let contribution = Contribution::new(
                member.id.clone(),
                content,
                contribution_type,
            );

            round_results.push(contribution.clone());
            self.workspace.add_contribution(contribution).await;
            self.members[idx].record_contribution();
        }

        self.workspace.advance_round().await;

        RoundResult {
            round,
            contributions: round_results,
            status: self.evaluate_round_status(),
        }
    }

    /// Get participant indices for this round based on protocol
    fn get_participant_indices(&self, round: u32) -> Vec<usize> {
        match self.config.protocol {
            CoordinationProtocol::Brainstorm => {
                (0..self.members.len()).collect()
            }
            CoordinationProtocol::Review => {
                let mut indices = Vec::new();
                if let Some(idx) = self.member_index_by_role(TeamRole::Lead) {
                    indices.push(idx);
                }
                if let Some(idx) = self.member_index_by_role(TeamRole::Critic) {
                    indices.push(idx);
                }
                indices
            }
            CoordinationProtocol::PairProgramming => {
                let mut indices = Vec::new();
                if let Some(idx) = self.member_index_by_role(TeamRole::Lead) {
                    indices.push(idx);
                }
                if let Some(idx) = self.member_index_by_role(TeamRole::Specialist) {
                    indices.push(idx);
                }
                if indices.is_empty() {
                    indices.push(0);
                }
                indices
            }
            CoordinationProtocol::Debate => {
                if round <= 2 {
                    // Early rounds: exclude synthesizer
                    self.members.iter()
                        .enumerate()
                        .filter(|(_, m)| m.role != TeamRole::Synthesizer)
                        .map(|(i, _)| i)
                        .collect()
                } else {
                    (0..self.members.len()).collect()
                }
            }
        }
    }

    /// Evaluate round status
    fn evaluate_round_status(&self) -> RoundStatus {
        let max_round = self.config.max_round.unwrap_or(u32::MAX);

        if self.workspace.current_round >= max_round {
            return RoundStatus::Completed;
        }

        let kb = self.workspace.knowledge_base.blocking_read();
        let top_insights = kb.get_top_insights(1);
        if let Some(insight) = top_insights.first() {
            if insight.importance >= self.config.consensus_threshold {
                return RoundStatus::ConsensusReached;
            }
        }

        RoundStatus::Continue
    }

    /// Run the full collaboration session
    pub async fn solve(&mut self, problem: &str) -> SolutionResult {
        self.status = TeamStatus::InProgress;

        let mut all_rounds = Vec::new();
        let max_round = self.config.max_round.unwrap_or(u32::MAX);

        while self.workspace.current_round < max_round {
            let round_result = self.execute_round(problem).await;
            all_rounds.push(round_result.clone());

            match round_result.status {
                RoundStatus::ConsensusReached | RoundStatus::Completed => {
                    break;
                }
                RoundStatus::Continue => {
                    continue;
                }
            }
        }

        let solution = self.synthesize_solution().await;
        self.status = TeamStatus::Completed;

        SolutionResult {
            problem: problem.to_string(),
            team_id: self.id.clone(),
            rounds: all_rounds,
            solution,
            total_contributions: self.workspace.total_contributions.load(Ordering::SeqCst),
        }
    }

    /// Synthesize a final solution
    async fn synthesize_solution(&self) -> String {
        let kb = self.workspace.knowledge_base.read().await;
        let insights = kb.get_top_insights(5);

        format!(
            "Solution synthesized from {} key insights and {} total contributions.",
            insights.len(),
            self.workspace.total_contributions.load(Ordering::SeqCst)
        )
    }

    /// Get team statistics
    pub fn stats(&self) -> TeamStats {
        let kb = self.workspace.knowledge_base.blocking_read();
        TeamStats {
            team_id: self.id.clone(),
            member_count: self.members.len(),
            total_contributions: self.workspace.total_contributions.load(Ordering::SeqCst),
            current_round: self.workspace.current_round,
            status: self.status,
            top_insights: kb.get_top_insights(3).len(),
        }
    }
}

/// Team execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamStatus {
    Initialized,
    InProgress,
    Completed,
    Failed,
}

/// Result of a single round
#[derive(Debug, Clone)]
pub struct RoundResult {
    pub round: u32,
    pub contributions: Vec<Contribution>,
    pub status: RoundStatus,
}

/// Round status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundStatus {
    Continue,
    ConsensusReached,
    Completed,
}

/// Final solution result
#[derive(Debug, Clone)]
pub struct SolutionResult {
    pub problem: String,
    pub team_id: String,
    pub rounds: Vec<RoundResult>,
    pub solution: String,
    pub total_contributions: u32,
}

/// Team statistics
#[derive(Debug, Clone)]
pub struct TeamStats {
    pub team_id: String,
    pub member_count: usize,
    pub total_contributions: u32,
    pub current_round: u32,
    pub status: TeamStatus,
    pub top_insights: usize,
}

// ============================================================================
// Global Thought Team Manager
// ============================================================================

/// Global manager for thought teams
pub struct ThoughtTeamManager {
    teams: HashMap<String, Arc<RwLock<ThoughtTeam>>>,
}

impl ThoughtTeamManager {
    pub fn new() -> Self {
        Self {
            teams: HashMap::new(),
        }
    }

    /// Create a new team
    pub async fn create_team(
        &mut self,
        team_id: &str,
        config: ThoughtTeamConfig,
    ) -> Arc<RwLock<ThoughtTeam>> {
        let team = ThoughtTeam::new(team_id, config);
        let arc = Arc::new(RwLock::new(team));
        self.teams.insert(team_id.to_string(), arc.clone());
        arc
    }

    /// Get a team by ID
    pub fn get_team(&self, team_id: &str) -> Option<Arc<RwLock<ThoughtTeam>>> {
        self.teams.get(team_id).cloned()
    }

    /// List all teams
    pub fn list_teams(&self) -> Vec<String> {
        self.teams.keys().cloned().collect()
    }
}

impl Default for ThoughtTeamManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_team_roles() {
        assert_eq!(TeamRole::Lead.name(), "Lead");
        assert_eq!(TeamRole::Critic.prompt_suffix(), "You are the critic. Challenge assumptions and identify potential flaws.");
    }

    #[test]
    fn test_protocol_rounds() {
        assert_eq!(CoordinationProtocol::Brainstorm.max_rounds(), 5);
        assert_eq!(CoordinationProtocol::Debate.max_rounds(), 6);
    }

    #[tokio::test]
    async fn test_thought_team_creation() {
        let config = ThoughtTeamConfig::default();
        let team = ThoughtTeam::new("test_team", config);

        assert_eq!(team.members.len(), 3);
    }

    #[tokio::test]
    async fn test_workspace_contributions() {
        let workspace = SharedWorkspace::new();
        let contrib = Contribution::new(
            AgentId("test".to_string()),
            "Test contribution".to_string(),
            ContributionType::Analysis,
        );

        workspace.add_contribution(contrib).await;
        assert_eq!(workspace.total_contributions.load(Ordering::SeqCst), 1);

        let recent = workspace.get_recent_contributions(10).await;
        assert_eq!(recent.len(), 1);
    }
}