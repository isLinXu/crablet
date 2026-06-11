//! Audit Log and Verification Types
//!
//! Types for proof chains, verification results, and audit logging
//! in the verifiable reasoning engine.

use serde::{Deserialize, Serialize};

use super::logical_expression::LogicalExpression;

/// A complete proof chain
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProofChain {
    /// Unique proof ID
    pub id: String,
    /// Steps in the proof
    pub steps: Vec<super::logical_expression::ProofStep>,
    /// Final conclusion
    pub conclusion: Option<LogicalExpression>,
    /// Whether the proof is valid
    pub is_valid: bool,
    /// Trust level (0.0 - 1.0)
    pub trust_level: f32,
    /// Proof generation time (ms)
    pub generation_time_ms: u64,
}

impl ProofChain {
    /// Create a new proof chain
    pub fn new(id: String) -> Self {
        Self {
            id,
            steps: Vec::new(),
            conclusion: None,
            is_valid: false,
            trust_level: 0.0,
            generation_time_ms: 0,
        }
    }

    /// Add a step to the proof
    pub fn add_step(&mut self, step: super::logical_expression::ProofStep) {
        self.steps.push(step);
    }

    /// Verify the proof chain
    pub fn verify(&self) -> bool {
        if self.steps.is_empty() {
            return false;
        }

        // Check that each step has valid antecedents
        for step in &self.steps {
            for ant_idx in &step.antecedents {
                if *ant_idx >= step.step_number {
                    return false; // Antecedent must come before this step
                }
            }
        }

        true
    }

    /// Convert to human-readable format
    pub fn to_display_string(&self) -> String {
        let mut output = format!("Proof ID: {}\n", self.id);
        output.push_str(&format!(
            "Valid: {} | Trust Level: {:.2}\n\n",
            self.is_valid, self.trust_level
        ));

        for step in &self.steps {
            output.push_str(&format!(
                "[Step {}] {}\n  Rule: {}\n  From: {:?}\n  Justification: {}\n\n",
                step.step_number,
                step.expression,
                step.rule_name,
                step.antecedents,
                step.justification
            ));
        }

        if let Some(conclusion) = &self.conclusion {
            output.push_str(&format!("Conclusion: {}\n", conclusion));
        }

        output
    }
}

/// Verification result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether the reasoning is valid
    pub is_valid: bool,
    /// Proof chain if available
    pub proof_chain: Option<ProofChain>,
    /// Trust level (0.0 - 1.0)
    pub trust_level: f32,
    /// Issues found (empty if valid)
    pub issues: Vec<VerificationIssue>,
    /// Verification timestamp
    pub timestamp: u64,
}

/// An issue found during verification
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationIssue {
    /// Issue type
    pub issue_type: IssueType,
    /// Description
    pub description: String,
    /// Severity (0.0 - 1.0)
    pub severity: f32,
    /// Related step number if applicable
    pub step_number: Option<usize>,
}

/// Type of verification issue
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum IssueType {
    /// Logical inconsistency detected
    Inconsistency,
    /// Unsupported assumption
    UnsupportedAssumption,
    /// Invalid inference
    InvalidInference,
    /// Circular reasoning
    CircularReasoning,
    /// Missing justification
    MissingJustification,
    /// Tampering detected
    TamperingDetected,
}

/// An audit log entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Entry ID
    pub id: String,
    /// Action type
    pub action: AuditAction,
    /// Details
    pub details: String,
    /// Timestamp
    pub timestamp: u64,
    /// Hash of previous entry (for chain integrity)
    pub prev_hash: String,
    /// Hash of this entry
    pub hash: String,
}

/// Type of audit action
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AuditAction {
    /// Fact added
    FactAdded,
    /// Rule added
    RuleAdded,
    /// Reasoning performed
    ReasoningPerformed,
    /// Query executed
    QueryExecuted,
    /// Verification performed
    VerificationPerformed,
    /// Audit log accessed
    AuditLogAccessed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proof_chain_new() {
        let chain = ProofChain::new("test-proof".to_string());
        assert_eq!(chain.id, "test-proof");
        assert!(!chain.is_valid);
        assert_eq!(chain.trust_level, 0.0);
    }

    #[test]
    fn test_proof_chain_verify_empty() {
        let chain = ProofChain::new("test-proof".to_string());
        assert!(!chain.verify());
    }

    #[test]
    fn test_verification_result_default() {
        let result = VerificationResult {
            is_valid: false,
            proof_chain: None,
            trust_level: 0.0,
            issues: Vec::new(),
            timestamp: 0,
        };
        assert!(!result.is_valid);
        assert!(result.issues.is_empty());
    }
}
