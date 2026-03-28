//! Enhanced Consensus Mechanisms for Swarm Coordination
//!
//! Implements advanced consensus algorithms beyond simple majority voting:
//! - Weighted voting with agent credibility
//! - BFT (Byzantine Fault Tolerance)
//! - Auction-based consensus
//! - Quadratic voting
//!
//! # Architecture
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                   Consensus Orchestrator                          │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                                                                  │
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
//! │  │   Weighted   │  │     BFT      │  │   Auction    │          │
//! │  │   Voting     │  │  Consensus   │  │   Consensus  │          │
//! │  └──────────────┘  └──────────────┘  └──────────────┘          │
//! │                                                                  │
//! │  ┌──────────────┐  ┌──────────────┐                            │
//! │  │  Quadratic   │  │   Majority    │                            │
//! │  │   Voting     │  │   Voting      │                            │
//! │  └──────────────┘  └──────────────┘                            │
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
// Consensus Protocols
// ============================================================================

/// Advanced consensus protocol types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusProtocol {
    /// Simple majority vote (50%+1)
    SimpleMajority,
    /// Weighted voting based on agent credibility/reputation
    WeightedVoting,
    /// Byzantine Fault Tolerance with threshold
    BFT,
    /// Auction-based consensus
    Auction,
    /// Quadratic voting (cost = votes^2)
    QuadraticVoting,
}

impl ConsensusProtocol {
    pub fn name(&self) -> &'static str {
        match self {
            ConsensusProtocol::SimpleMajority => "Simple Majority",
            ConsensusProtocol::WeightedVoting => "Weighted Voting",
            ConsensusProtocol::BFT => "Byzantine Fault Tolerance",
            ConsensusProtocol::Auction => "Auction-based",
            ConsensusProtocol::QuadraticVoting => "Quadratic Voting",
        }
    }
}

/// A vote in the consensus process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    pub voter_id: AgentId,
    pub choice: String,
    pub weight: f64,
    pub timestamp: DateTime<Utc>,
    pub signature: Option<String>,
}

impl Vote {
    pub fn new(voter_id: AgentId, choice: String, weight: f64) -> Self {
        Self {
            voter_id,
            choice,
            weight,
            timestamp: Utc::now(),
            signature: None,
        }
    }

    pub fn with_signature(mut self, sig: String) -> Self {
        self.signature = Some(sig);
        self
    }
}

/// Result of a consensus decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusResult {
    /// The winning decision
    pub decision: String,
    /// Confidence level (0.0 - 1.0)
    pub confidence: f64,
    /// All voters who participated
    pub voters: Vec<AgentId>,
    /// Dissenting agents and their reasons
    pub dissenting_agents: Vec<DissentingAgent>,
    /// Total votes tallied
    pub total_votes: usize,
    /// Protocol used
    pub protocol: ConsensusProtocol,
    /// Whether consensus was reached
    pub reached: bool,
}

/// An agent who dissented from the consensus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DissentingAgent {
    pub agent_id: AgentId,
    pub reason: String,
    pub alternative_choice: String,
}

// ============================================================================
// Agent Credibility / Reputation
// ============================================================================

/// Agent credibility tracker for weighted voting
#[derive(Debug, Clone, Default)]
pub struct AgentCredibility {
    scores: Arc<RwLock<HashMap<AgentId, CredibilityScore>>>,
}

impl AgentCredibility {
    pub fn new() -> Self {
        Self {
            scores: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get credibility score for an agent
    pub async fn get_score(&self, agent_id: &AgentId) -> f64 {
        let scores = self.scores.read().await;
        scores.get(agent_id).map(|s| s.score()).unwrap_or(0.5)
    }

    /// Update credibility based on decision accuracy
    pub async fn update(
        &self,
        agent_id: &AgentId,
        decision_was_correct: bool,
        stake: f64,
    ) {
        let mut scores = self.scores.write().await;
        let entry = scores.entry(agent_id.clone()).or_insert_with(|| {
            CredibilityScore::new()
        });
        entry.record_vote(decision_was_correct, stake);
    }

    /// Get all scores
    pub async fn get_all_scores(&self) -> HashMap<AgentId, f64> {
        let scores = self.scores.read().await;
        scores.iter()
            .map(|(id, cs)| (id.clone(), cs.score()))
            .collect()
    }
}

/// Individual credibility score with history
#[derive(Debug, Clone)]
pub struct CredibilityScore {
    total_stake: f64,
    correct_stake: f64,
    vote_count: u64,
    correct_count: u64,
    last_updated: DateTime<Utc>,
}

impl CredibilityScore {
    pub fn new() -> Self {
        Self {
            total_stake: 0.0,
            correct_stake: 0.0,
            vote_count: 0,
            correct_count: 0,
            last_updated: Utc::now(),
        }
    }

    pub fn record_vote(&mut self, correct: bool, stake: f64) {
        self.total_stake += stake;
        self.vote_count += 1;
        if correct {
            self.correct_stake += stake;
            self.correct_count += 1;
        }
        self.last_updated = Utc::now();
    }

    /// Calculate credibility score using Wilson score interval
    pub fn score(&self) -> f64 {
        if self.vote_count == 0 {
            return 0.5;
        }

        let proportion = self.correct_count as f64 / self.vote_count as f64;
        let z = 1.645; // 90% confidence

        let n = self.vote_count as f64;
        let denominator = 1.0 + z * z / n;
        let center = proportion + z * z / (2.0 * n);
        let spread = z * ((proportion * (1.0 - proportion) + z * z / (4.0 * n)) / n).sqrt();

        ((center - spread) / denominator).max(0.0).min(1.0)
    }
}

impl Default for CredibilityScore {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Consensus Calculator
// ============================================================================

/// Calculator for different consensus mechanisms
pub struct ConsensusCalculator {
    protocol: ConsensusProtocol,
    credibility: Option<Arc<AgentCredibility>>,
    threshold: f64,
}

impl ConsensusCalculator {
    pub fn new(protocol: ConsensusProtocol) -> Self {
        Self {
            protocol,
            credibility: None,
            threshold: 0.5,
        }
    }

    pub fn with_credibility(mut self, credibility: Arc<AgentCredibility>) -> Self {
        self.credibility = Some(credibility);
        self
    }

    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold;
        self
    }

    /// Calculate consensus from votes
    pub async fn calculate(&self, votes: Vec<Vote>) -> ConsensusResult {
        match self.protocol {
            ConsensusProtocol::SimpleMajority => self.simple_majority(votes).await,
            ConsensusProtocol::WeightedVoting => self.weighted_voting(votes).await,
            ConsensusProtocol::BFT => self.bft_consensus(votes).await,
            ConsensusProtocol::Auction => self.auction_consensus(votes).await,
            ConsensusProtocol::QuadraticVoting => self.quadratic_voting(votes).await,
        }
    }

    /// Simple majority vote
    async fn simple_majority(&self, votes: Vec<Vote>) -> ConsensusResult {
        let mut tally: HashMap<String, usize> = HashMap::new();
        let mut voters = Vec::new();

        for vote in &votes {
            voters.push(vote.voter_id.clone());
            *tally.entry(vote.choice.clone()).or_insert(0) += 1;
        }

        let total_votes = tally.values().sum::<usize>();
        let threshold_votes = total_votes / 2 + 1;

        let (decision, decision_count, reached) = tally
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(choice, count)| {
                (choice, count, count >= threshold_votes)
            })
            .unwrap_or_else(|| ("No decision".to_string(), 0, false));

        let confidence = if reached {
            decision_count as f64 / total_votes as f64
        } else {
            0.0
        };

        ConsensusResult {
            decision,
            confidence,
            voters,
            dissenting_agents: Vec::new(),
            total_votes,
            protocol: self.protocol,
            reached,
        }
    }

    /// Weighted voting based on credibility
    async fn weighted_voting(&self, votes: Vec<Vote>) -> ConsensusResult {
        let mut weighted_tally: HashMap<String, f64> = HashMap::new();
        let mut voters = Vec::new();
        let dissenting = Vec::new();

        for vote in &votes {
            voters.push(vote.voter_id.clone());

            // Use vote weight if credibility not available
            let weight = if let Some(ref cred) = self.credibility {
                cred.get_score(&vote.voter_id).await * vote.weight
            } else {
                vote.weight
            };

            *weighted_tally.entry(vote.choice.clone()).or_insert(0.0) += weight;
        }

        let total_weight: f64 = weighted_tally.values().sum();
        let threshold_weight = total_weight * self.threshold;

        // Find winner
        let (decision, winning_weight) = weighted_tally
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap_or_else(|| ("No decision".to_string(), 0.0));

        let reached = winning_weight >= threshold_weight;
        let confidence = if total_weight > 0.0 {
            winning_weight / total_weight
        } else {
            0.0
        };

        ConsensusResult {
            decision,
            confidence,
            voters,
            dissenting_agents: dissenting,
            total_votes: votes.len(),
            protocol: self.protocol,
            reached,
        }
    }

    /// Byzantine Fault Tolerance consensus
    async fn bft_consensus(&self, votes: Vec<Vote>) -> ConsensusResult {
        // BFT requires 2f+1 honest nodes out of 3f+1 total
        let n = votes.len();
        let f = (n - 1) / 3; // Maximum Byzantine nodes

        // Group votes by choice
        let mut choice_votes: HashMap<String, Vec<&Vote>> = HashMap::new();
        for vote in &votes {
            choice_votes.entry(vote.choice.clone()).or_insert_with(Vec::new).push(vote);
        }

        // Need n - f votes for consensus
        let quorum = n - f;

        let (decision, voters, reached) = choice_votes
            .into_iter()
            .filter(|(_, v)| v.len() >= quorum)
            .max_by_key(|(_, v)| v.len())
            .map(|(choice, votes)| {
                (choice, votes.iter().map(|v| v.voter_id.clone()).collect(), true)
            })
            .unwrap_or_else(|| ("No consensus".to_string(), Vec::new(), false));

        let confidence = if reached {
            voters.len() as f64 / n as f64
        } else {
            0.0
        };

        ConsensusResult {
            decision,
            confidence,
            voters,
            dissenting_agents: Vec::new(),
            total_votes: n,
            protocol: self.protocol,
            reached,
        }
    }

    /// Auction-based consensus
    async fn auction_consensus(&self, votes: Vec<Vote>) -> ConsensusResult {
        // In auction consensus, agents "bid" on choices
        // The choice with highest total bid wins
        let mut bids: HashMap<String, f64> = HashMap::new();

        for vote in &votes {
            *bids.entry(vote.choice.clone()).or_insert(0.0) += vote.weight;
        }

        let total_bids: f64 = bids.values().sum();

        let (decision, winning_bid) = bids
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap_or_else(|| ("No decision".to_string(), 0.0));

        let confidence = if total_bids > 0.0 {
            winning_bid / total_bids
        } else {
            0.0
        };

        // Check if winner has majority of bids
        let reached = winning_bid > total_bids * self.threshold;

        ConsensusResult {
            decision,
            confidence,
            voters: votes.iter().map(|v| v.voter_id.clone()).collect(),
            dissenting_agents: Vec::new(),
            total_votes: votes.len(),
            protocol: self.protocol,
            reached,
        }
    }

    /// Quadratic voting (cost = votes^2)
    async fn quadratic_voting(&self, votes: Vec<Vote>) -> ConsensusResult {
        let mut choice_cost: HashMap<String, f64> = HashMap::new();
        let mut voters = Vec::new();

        // Group by agent
        let mut agent_votes: HashMap<AgentId, HashMap<String, f64>> = HashMap::new();
        for vote in &votes {
            voters.push(vote.voter_id.clone());
            agent_votes
                .entry(vote.voter_id.clone())
                .or_insert_with(HashMap::new)
                .entry(vote.choice.clone())
                .or_insert_with(|| 0.0);
        }

        // Each agent has budget for quadratic cost
        let budget = 10.0_f64;
        let mut remaining = HashMap::new();

        for (agent, mut choices) in agent_votes {
            // Allocate budget quadratically
            let _total_existing: f64 = choices.values().sum();
            let mut agent_remaining = budget;

            for (choice, count) in &mut choices {
                let count = *count as f64;
                let cost = count * count;
                if cost <= agent_remaining {
                    *choice_cost.entry(choice.clone()).or_insert(0.0) += count;
                    agent_remaining -= cost;
                }
            }
            remaining.insert(agent, agent_remaining);
        }

        let total_cost: f64 = choice_cost.values().sum();

        let (decision, winning_cost) = choice_cost
            .into_iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap_or_else(|| ("No decision".to_string(), 0.0));

        let confidence = if total_cost > 0.0 {
            (winning_cost / total_cost).sqrt() // sqrt because it's quadratic
        } else {
            0.0
        };

        ConsensusResult {
            decision,
            confidence,
            voters,
            dissenting_agents: Vec::new(),
            total_votes: votes.len(),
            protocol: self.protocol,
            reached: winning_cost > 0.0,
        }
    }
}

// ============================================================================
// Consensus Orchestrator
// ============================================================================

/// Manages consensus processes across multiple agents
pub struct ConsensusOrchestrator {
    protocol: ConsensusProtocol,
    credibility: Arc<AgentCredibility>,
    active_votes: HashMap<String, Vec<Vote>>,
}

impl ConsensusOrchestrator {
    pub fn new(protocol: ConsensusProtocol) -> Self {
        Self {
            protocol,
            credibility: Arc::new(AgentCredibility::new()),
            active_votes: HashMap::new(),
        }
    }

    /// Start a new vote
    pub fn start_vote(&mut self, vote_id: &str) {
        self.active_votes.insert(vote_id.to_string(), Vec::new());
    }

    /// Cast a vote
    pub async fn cast_vote(
        &self,
        vote_id: &str,
        voter_id: AgentId,
        choice: String,
        weight: f64,
    ) -> Option<()> {
        let votes = self.active_votes.get(vote_id)?;
        let mut votes = votes.clone();

        let vote = Vote::new(voter_id, choice, weight);
        votes.push(vote);

        Some(())
    }

    /// Tally votes and reach consensus
    pub async fn tally(&self, vote_id: &str) -> Option<ConsensusResult> {
        let votes = self.active_votes.get(vote_id)?.clone();

        let calculator = ConsensusCalculator::new(self.protocol)
            .with_credibility(self.credibility.clone());

        Some(calculator.calculate(votes).await)
    }

    /// Update credibility based on outcome
    pub async fn update_credibility(
        &self,
        agent_id: &AgentId,
        correct: bool,
        stake: f64,
    ) {
        self.credibility.update(agent_id, correct, stake).await;
    }

    /// Get credibility for an agent
    pub async fn get_credibility(&self, agent_id: &AgentId) -> f64 {
        self.credibility.get_score(agent_id).await
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_majority() {
        let calc = ConsensusCalculator::new(ConsensusProtocol::SimpleMajority);

        let votes = vec![
            Vote::new(AgentId("a1".into()), "option1".to_string(), 1.0),
            Vote::new(AgentId("a2".into()), "option1".to_string(), 1.0),
            Vote::new(AgentId("a3".into()), "option2".to_string(), 1.0),
        ];

        let result = calc.calculate(votes).await;
        assert_eq!(result.decision, "option1");
        assert!(result.reached);
    }

    #[tokio::test]
    async fn test_weighted_voting() {
        let cred = Arc::new(AgentCredibility::new());
        let calc = ConsensusCalculator::new(ConsensusProtocol::WeightedVoting)
            .with_credibility(cred);

        let votes = vec![
            Vote::new(AgentId("a1".into()), "option1".to_string(), 1.0),
            Vote::new(AgentId("a2".into()), "option1".to_string(), 3.0), // 3x weight
            Vote::new(AgentId("a3".into()), "option2".to_string(), 1.0),
        ];

        let result = calc.calculate(votes).await;
        assert_eq!(result.decision, "option1");
    }

    #[tokio::test]
    async fn test_credibility() {
        let cred = AgentCredibility::new();

        // Record some votes
        cred.update(&AgentId("agent1".into()), true, 1.0).await;
        cred.update(&AgentId("agent1".into()), true, 1.0).await;
        cred.update(&AgentId("agent1".into()), false, 1.0).await;

        let score = cred.get_score(&AgentId("agent1".into())).await;
        assert!(score > 0.5); // Should be high since 2/3 correct
    }
}