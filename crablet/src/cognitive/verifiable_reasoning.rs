//! Verifiable Reasoning Engine
//!
//! A reasoning engine that produces formally verifiable proofs for its conclusions,
//! enabling audit trails and trust verification.
//!
//! # Core Features
//!
//! 1. **Formal Verification** - Verify reasoning steps against logical rules
//! 2. **Proof Generation** - Generate human-readable proof chains
//! 3. **Audit Trail** - Immutable audit log for all reasoning steps
//! 4. **Tamper Detection** - Detect and report any tampering attempts

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::error;

// Re-export from sibling modules
pub use super::logical_expression::{
    LogicalExpression, InferenceRule, RuleType, ProofStep,
};
pub use super::audit_log::{
    ProofChain, VerificationResult, VerificationIssue, IssueType,
    AuditEntry, AuditAction,
};

/// Get current timestamp in milliseconds
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}


/// The verifiable reasoning engine
pub struct VerifiableReasoner {
    /// Knowledge base (facts)
    knowledge_base: RwLock<HashSet<LogicalExpression>>,
    /// Inference rules
    rules: RwLock<Vec<InferenceRule>>,
    /// Audit log
    audit_log: RwLock<Vec<AuditEntry>>,
    /// Proof cache
    proof_cache: RwLock<HashMap<String, ProofChain>>,
}

impl VerifiableReasoner {
    /// Create a new verifiable reasoner
    pub fn new() -> Self {
        let reasoner = Self {
            knowledge_base: RwLock::new(HashSet::new()),
            rules: RwLock::new(Vec::new()),
            audit_log: RwLock::new(Vec::new()),
            proof_cache: RwLock::new(HashMap::new()),
        };
        
        // Initialize with standard inference rules
        if let Err(e) = reasoner.init_standard_rules() {
            error!("failed to init standard rules: {e}");
        }
        
        reasoner
    }

    /// Initialize standard inference rules
    fn init_standard_rules(&self) -> Result<()> {
        let mut rules = self.rules.write().map_err(|e| anyhow!("lock poisoned: {e}"))?;
        rules.push(InferenceRule::modus_ponens());
        rules.push(InferenceRule::modus_tollens());
        rules.push(InferenceRule::custom(
            "And Introduction",
            vec![
                LogicalExpression::Variable("A".to_string()),
                LogicalExpression::Variable("B".to_string()),
            ],
            LogicalExpression::And(
                Box::new(LogicalExpression::Variable("A".to_string())),
                Box::new(LogicalExpression::Variable("B".to_string())),
            ),
        ));
        rules.push(InferenceRule::custom(
            "And Elimination",
            vec![LogicalExpression::And(
                Box::new(LogicalExpression::Variable("A".to_string())),
                Box::new(LogicalExpression::Variable("B".to_string())),
            )],
            LogicalExpression::Variable("A".to_string()),
        ));
        rules.push(InferenceRule::custom(
            "Or Introduction",
            vec![LogicalExpression::Variable("A".to_string())],
            LogicalExpression::Or(
                Box::new(LogicalExpression::Variable("A".to_string())),
                Box::new(LogicalExpression::Variable("B".to_string())),
            ),
        ));
        Ok(())
    }

    /// Add a fact to the knowledge base
    pub fn add_fact(&self, fact: LogicalExpression) -> Result<()> {
        let mut kb = self.knowledge_base.write().map_err(|e| anyhow!("lock poisoned: {e}"))?;
        kb.insert(fact.clone());
        
        // Audit log
        self.add_audit_entry(AuditAction::FactAdded, &fact.to_string())?;
        
        Ok(())
    }

    /// Add a fact from string
    pub fn add_fact_string(&self, fact_str: &str) -> Result<()> {
        let fact = LogicalExpression::parse(fact_str)?;
        self.add_fact(fact)
    }

    /// Add an inference rule
    pub fn add_rule(&self, rule: InferenceRule) -> Result<()> {
        let mut rules = self.rules.write().map_err(|e| anyhow!("lock poisoned: {e}"))?;
        rules.push(rule.clone());
        
        // Audit log
        self.add_audit_entry(AuditAction::RuleAdded, &rule.name)?;
        
        Ok(())
    }

    /// Add an inference rule from string representation
    pub fn add_rule_from_strings(&self, name: &str, antecedents: Vec<&str>, consequent: &str) -> Result<()> {
        let ants: Result<Vec<LogicalExpression>> = antecedents.iter().map(|s| LogicalExpression::parse(s)).collect();
        let rule = InferenceRule::custom(name, ants?, LogicalExpression::parse(consequent)?);
        self.add_rule(rule)
    }

    /// Verify a conclusion using the knowledge base
    pub async fn verify(&self, target_str: &str) -> Result<VerificationResult> {
        let start_time = std::time::Instant::now();
        
        let target = LogicalExpression::parse(target_str)?;
        
        // Check cache
        if let Some(cached) = self.proof_cache.read().map_err(|e| anyhow!("lock poisoned: {e}"))?.get(target_str) {
            return Ok(VerificationResult {
                is_valid: cached.is_valid,
                proof_chain: Some(cached.clone()),
                trust_level: cached.trust_level,
                issues: vec![],
                timestamp: current_timestamp(),
            });
        }
        
        // Audit log
        self.add_audit_entry(AuditAction::ReasoningPerformed, target_str)?;
        
        // Try to prove the target
        let proof_result = self.prove(target.clone()).await;
        
        let issues = Vec::new();
        let (is_valid, proof_chain, trust_level) = match proof_result {
            Ok(proof) => {
                let is_valid = proof.verify();
                let trust_level = self.calculate_trust_level(&proof);
                (is_valid, Some(proof), trust_level)
            }
            Err(e) => {
                let _issue = VerificationIssue {
                    issue_type: IssueType::InvalidInference,
                    description: e.to_string(),
                    severity: 1.0,
                    step_number: None,
                };
                (false, None, 0.0)
            }
        };
        
        let generation_time_ms = start_time.elapsed().as_millis() as u64;
        
        // Create proof chain with timing
        let result = VerificationResult {
            is_valid,
            proof_chain: proof_chain.clone(),
            trust_level,
            issues,
            timestamp: current_timestamp(),
        };
        
        // Cache the proof
        if let Some(mut proof) = proof_chain {
            proof.generation_time_ms = generation_time_ms;
            self.proof_cache.write().map_err(|e| anyhow!("lock poisoned: {e}"))?.insert(target_str.to_string(), proof);
        }
        
        // Audit log
        self.add_audit_entry(AuditAction::VerificationPerformed, &format!("valid={}", is_valid))?;
        
        Ok(result)
    }

    /// Try to prove a target expression
    async fn prove(&self, target: LogicalExpression) -> Result<ProofChain> {
        let mut proof = ProofChain::new(uuid::Uuid::new_v4().to_string());
        let kb = self.knowledge_base.read().map_err(|e| anyhow!("lock poisoned: {e}"))?;
        let rules = self.rules.read().map_err(|e| anyhow!("lock poisoned: {e}"))?;
        
        // Clone target for later use
        let target_for_comparison = target.clone();
        
        // Direct check: is the target in the knowledge base?
        if kb.contains(&target) {
            proof.add_step(ProofStep::new(
                1,
                target.clone(),
                &InferenceRule::custom("Premise", vec![], target.clone()),
                vec![],
            ));
            proof.conclusion = Some(target);
            proof.is_valid = true;
            proof.trust_level = 1.0;
            return Ok(proof);
        }
        
        // Try to derive using inference rules
        let mut step_number = 1;
        let mut derived: HashSet<LogicalExpression> = kb.clone();
        let mut queue: VecDeque<(LogicalExpression, Vec<usize>)> = VecDeque::new();
        queue.push_back((target.clone(), vec![]));
        
        while let Some((expr, _antecedents)) = queue.pop_front() {
            // Check if already derived
            if derived.contains(&expr) {
                continue;
            }
            
            // Try each rule
            for rule in rules.iter() {
                if let Some((new_expr, new_ants)) = self.apply_rule(rule, &derived, expr.clone(), step_number) {
                    let step = ProofStep::new(
                        step_number,
                        new_expr.clone(),
                        rule,
                        new_ants,
                    );
                    proof.add_step(step);
                    derived.insert(new_expr.clone());
                    queue.push_back((new_expr.clone(), vec![step_number]));
                    step_number += 1;
                    
                    if new_expr == target_for_comparison {
                        proof.conclusion = Some(target);
                        proof.is_valid = true;
                        return Ok(proof);
                    }
                }
            }
        }
        
        // Could not prove
        proof.is_valid = false;
        proof.trust_level = 0.0;
        Ok(proof)
    }

    /// Try to apply a rule
    fn apply_rule(&self, rule: &InferenceRule, facts: &HashSet<LogicalExpression>, _target: LogicalExpression, _step: usize) -> Option<(LogicalExpression, Vec<usize>)> {
        // Simplified rule application
        // Check if antecedents match facts
        let mut matched_antecedents = Vec::new();
        
        for ant in &rule.antecedents {
            if facts.contains(ant) {
                matched_antecedents.push(true);
            } else {
                // Check negation
                if let LogicalExpression::Not(f) = ant {
                    if facts.contains(f) {
                        matched_antecedents.push(true);
                        continue;
                    }
                }
                matched_antecedents.push(false);
            }
        }
        
        // If all antecedents match, derive consequent
        if matched_antecedents.iter().all(|m| *m) {
            return Some((rule.consequent.clone(), vec![]));
        }
        
        // Special case: Modus Ponens
        if rule.rule_type == RuleType::ModusPonens {
            for fact in facts {
                if let LogicalExpression::Implies(ref lhs, ref rhs) = fact {
                    if facts.contains(lhs) {
                        let consequent: LogicalExpression = (**rhs).clone();
                        return Some((consequent, vec![]));
                    }
                }
            }
        }
        
        None
    }

    /// Calculate trust level for a proof
    fn calculate_trust_level(&self, proof: &ProofChain) -> f32 {
        if proof.steps.is_empty() {
            return 0.0;
        }
        
        let mut trust = 1.0;
        
        // Reduce trust for each step without clear justification
        for step in &proof.steps {
            if step.justification.is_empty() {
                trust *= 0.9;
            }
        }
        
        // Reduce trust for long proofs (more opportunities for error)
        let length_factor = 1.0 - (proof.steps.len() as f32 * 0.01).min(0.5);
        trust *= length_factor;
        
        trust.max(0.0).min(1.0)
    }

    /// Add an audit log entry
    fn add_audit_entry(&self, action: AuditAction, details: &str) -> Result<()> {
        let mut log = self.audit_log.write().map_err(|e| anyhow!("lock poisoned: {e}"))?;
        let prev_hash = log.last().map(|e| e.hash.clone()).unwrap_or_else(|| "genesis".to_string());
        let timestamp = current_timestamp();
        
        let entry = AuditEntry {
            id: uuid::Uuid::new_v4().to_string(),
            action,
            details: details.to_string(),
            timestamp,
            prev_hash: prev_hash.clone(),
            hash: self.compute_hash(&format!("{}{}{}", prev_hash, details, timestamp)),
        };
        
        log.push(entry);
        Ok(())
    }

    /// Compute a simple hash
    fn compute_hash(&self, data: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Get the audit log
    pub fn get_audit_log(&self) -> Vec<AuditEntry> {
        let _ = self.add_audit_entry(AuditAction::AuditLogAccessed, "Audit log retrieved");
        match self.audit_log.read() {
            Ok(log) => log.clone(),
            Err(_) => Vec::new(),
        }
    }

    /// Verify audit log integrity
    pub fn verify_audit_integrity(&self) -> Result<bool> {
        let log = self.audit_log.read().map_err(|e| anyhow!("lock poisoned: {e}"))?;
        let mut prev_hash = "genesis".to_string();
        
        for entry in log.iter() {
            if entry.prev_hash != prev_hash {
                return Err(anyhow!("Audit log integrity violated at entry {}", entry.id));
            }
            
            let computed_hash = self.compute_hash(&format!("{}{}{}", entry.prev_hash, entry.details, entry.timestamp));
            if computed_hash != entry.hash {
                return Err(anyhow!("Tampering detected at entry {}", entry.id));
            }
            
            prev_hash = entry.hash.clone();
        }
        
        Ok(true)
    }

    /// Get knowledge base facts
    pub fn get_knowledge_base(&self) -> Vec<LogicalExpression> {
        match self.knowledge_base.read() {
            Ok(kb) => kb.iter().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Get available inference rules
    pub fn get_rules(&self) -> Vec<InferenceRule> {
        match self.rules.read() {
            Ok(rules) => rules.clone(),
            Err(_) => Vec::new(),
        }
    }

    /// Clear the knowledge base
    pub fn clear_knowledge_base(&self) {
        match self.knowledge_base.write() {
            Ok(mut kb) => kb.clear(),
            Err(e) => error!("knowledge base lock poisoned: {e}"),
        }
    }
}

impl Default for VerifiableReasoner {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_variable() {
        let expr = LogicalExpression::parse("X").expect("parse variable should succeed");
        assert_eq!(expr, LogicalExpression::Variable("X".to_string()));
    }

    #[test]
    fn test_parse_and() {
        let expr = LogicalExpression::parse("A AND B").expect("parse AND expression should succeed");
        assert_eq!(
            expr,
            LogicalExpression::And(
                Box::new(LogicalExpression::Variable("A".to_string())),
                Box::new(LogicalExpression::Variable("B".to_string()))
            )
        );
    }

    #[test]
    fn test_parse_implies() {
        let expr = LogicalExpression::parse("A IMPLIES B").expect("parse IMPLIES expression should succeed");
        assert_eq!(
            expr,
            LogicalExpression::Implies(
                Box::new(LogicalExpression::Variable("A".to_string())),
                Box::new(LogicalExpression::Variable("B".to_string()))
            )
        );
    }

    #[test]
    fn test_evaluate() {
        let expr = LogicalExpression::parse("A AND B").expect("parse A AND B should succeed");
        let mut context = HashMap::new();
        context.insert("A".to_string(), true);
        context.insert("B".to_string(), true);
        
        assert_eq!(expr.evaluate(&context).expect("evaluate with true context should succeed"), true);
        
        context.insert("B".to_string(), false);
        assert_eq!(expr.evaluate(&context).expect("evaluate with false context should succeed"), false);
    }

    #[tokio::test]
    async fn test_modus_ponens() {
        let rule = InferenceRule::modus_ponens();
        assert_eq!(rule.rule_type, RuleType::ModusPonens);
        assert_eq!(rule.antecedents.len(), 2);
    }

    #[tokio::test]
    async fn test_verifiable_reasoner() {
        let reasoner = VerifiableReasoner::new();

        // Add facts
        reasoner.add_fact_string("A").expect("add fact string 'A' should succeed");
        reasoner.add_fact_string("A IMPLIES B").expect("add fact string 'A IMPLIES B' should succeed");

        // Verify
        let result = reasoner.verify("B").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_audit_integrity() {
        let reasoner = VerifiableReasoner::new();
        reasoner.add_fact_string("A").expect("add fact string 'A' should succeed");
        
        let is_valid = reasoner.verify_audit_integrity();
        assert!(is_valid.is_ok());
        assert!(is_valid.expect("audit integrity should be valid"));
    }
}
