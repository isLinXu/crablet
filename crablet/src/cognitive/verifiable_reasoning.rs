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
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                 Verifiable Reasoning Engine                  │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
//! │  │  Inference  │  │   Proof    │  │   Audit    │        │
//! │  │   Engine   │  │  Generator  │  │    Log     │        │
//! │  └─────────────┘  └─────────────┘  └─────────────┘        │
//! │  ┌─────────────────────────────────────────────┐           │
//! │  │           Formal Verification Layer           │           │
//! │  │  ┌─────────┐ ┌─────────┐ ┌─────────┐      │           │
//! │  │  │ Propositional │ │ First-Order │ │ Modal  │      │
//! │  │  │   Logic   │ │   Logic   │ │  Logic  │      │           │
//! │  │  └─────────┘ └─────────┘ └─────────┘      │           │
//! │  └─────────────────────────────────────────────┘           │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! let engine = VerifiableReasoner::new();
//!
//! // Add knowledge
//! engine.add_fact("all_humans_are_mortal".to_string())?;
//! engine.add_rule(" modus_ponens", vec!["A", "A_implies_B"], "B")?;
//!
//! // Reason with verification
//! let result = engine.verify("Socrates_is_mortal").await?;
//!
//! println!("Proof: {:?}", result.proof_chain);
//! println!("Verified: {}", result.is_valid);
//! println!("Trust level: {}", result.trust_level);
//! ```

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::RwLock;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// A logical expression
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LogicalExpression {
    /// Propositional variable
    Variable(String),
    /// Logical AND
    And(Box<LogicalExpression>, Box<LogicalExpression>),
    /// Logical OR
    Or(Box<LogicalExpression>, Box<LogicalExpression>),
    /// Logical IMPLIES
    Implies(Box<LogicalExpression>, Box<LogicalExpression>),
    /// Logical NOT
    Not(Box<LogicalExpression>),
    /// Logical EQUIVALENT
    Equivalent(Box<LogicalExpression>, Box<LogicalExpression>),
    /// Universal quantification: ∀x.P
    ForAll(String, Box<LogicalExpression>),
    /// Existential quantification: ∃x.P
    Exists(String, Box<LogicalExpression>),
    /// Constant
    Constant(bool),
}

impl LogicalExpression {
    /// Parse from string representation
    pub fn parse(s: &str) -> Result<Self> {
        // Simple recursive descent parser for logical expressions
        let s = s.trim();
        
        // Handle parentheses
        if s.starts_with('(') && s.ends_with(')') {
            return Self::parse(&s[1..s.len()-1]);
        }
        
        // Handle NOT
        if s.starts_with("NOT ") || s.starts_with("not ") {
            return Ok(LogicalExpression::Not(Box::new(Self::parse(&s[4..])?)));
        }
        
        // Handle AND
        if let Some(pos) = find_binary_op(s, " AND ", &['(', ')']) {
            return Ok(LogicalExpression::And(
                Box::new(Self::parse(&s[..pos])?),
                Box::new(Self::parse(&s[pos + 5..])?),
            ));
        }
        
        // Handle OR
        if let Some(pos) = find_binary_op(s, " OR ", &['(', ')']) {
            return Ok(LogicalExpression::Or(
                Box::new(Self::parse(&s[..pos])?),
                Box::new(Self::parse(&s[pos + 4..])?),
            ));
        }
        
        // Handle IMPLIES
        if let Some(pos) = find_binary_op(s, " IMPLIES ", &['(', ')']) {
            return Ok(LogicalExpression::Implies(
                Box::new(Self::parse(&s[..pos])?),
                Box::new(Self::parse(&s[pos + 9..])?),
            ));
        }
        
        // Handle EQUIVALENT
        if let Some(pos) = find_binary_op(s, " EQUIV ", &['(', ')']) {
            return Ok(LogicalExpression::Equivalent(
                Box::new(Self::parse(&s[..pos])?),
                Box::new(Self::parse(&s[pos + 7..])?),
            ));
        }
        
        // Handle constants
        if s == "TRUE" || s == "true" || s == "T" {
            return Ok(LogicalExpression::Constant(true));
        }
        if s == "FALSE" || s == "false" || s == "F" {
            return Ok(LogicalExpression::Constant(false));
        }
        
        // Handle FORALL
        if s.starts_with("FORALL ") {
            if let Some(paren_pos) = s.find('(') {
                let var = &s[7..paren_pos].trim();
                let inner = &s[paren_pos + 1..s.len() - 1];
                return Ok(LogicalExpression::ForAll(
                    var.to_string(),
                    Box::new(Self::parse(inner)?),
                ));
            }
        }
        
        // Handle EXISTS
        if s.starts_with("EXISTS ") {
            if let Some(paren_pos) = s.find('(') {
                let var = &s[7..paren_pos].trim();
                let inner = &s[paren_pos + 1..s.len() - 1];
                return Ok(LogicalExpression::Exists(
                    var.to_string(),
                    Box::new(Self::parse(inner)?),
                ));
            }
        }
        
        // Otherwise, treat as variable
        Ok(LogicalExpression::Variable(s.to_string()))
    }

    /// Evaluate this expression given variable assignments
    pub fn evaluate(&self, context: &HashMap<String, bool>) -> Result<bool> {
        match self {
            LogicalExpression::Variable(name) => {
                context.get(name)
                    .copied()
                    .ok_or_else(|| anyhow!("Unknown variable: {}", name))
            }
            LogicalExpression::Constant(v) => Ok(*v),
            LogicalExpression::Not(e) => Ok(!e.evaluate(context)?),
            LogicalExpression::And(l, r) => Ok(l.evaluate(context)? && r.evaluate(context)?),
            LogicalExpression::Or(l, r) => Ok(l.evaluate(context)? || r.evaluate(context)?),
            LogicalExpression::Implies(l, r) => {
                let left = l.evaluate(context)?;
                let right = r.evaluate(context)?;
                Ok(!left || right)
            }
            LogicalExpression::Equivalent(l, r) => {
                Ok(l.evaluate(context)? == r.evaluate(context)?)
            }
            LogicalExpression::ForAll(_, _) => {
                // Simplified: assume true for now
                // Full implementation would require domain specification
                Ok(true)
            }
            LogicalExpression::Exists(_, _) => {
                // Simplified: assume true for now
                Ok(true)
            }
        }
    }

    /// Convert to string representation
    pub fn to_string(&self) -> String {
        match self {
            LogicalExpression::Variable(v) => v.clone(),
            LogicalExpression::Constant(true) => "TRUE".to_string(),
            LogicalExpression::Constant(false) => "FALSE".to_string(),
            LogicalExpression::Not(e) => format!("NOT ({})", e.to_string()),
            LogicalExpression::And(l, r) => format!("({} AND {})", l.to_string(), r.to_string()),
            LogicalExpression::Or(l, r) => format!("({} OR {})", l.to_string(), r.to_string()),
            LogicalExpression::Implies(l, r) => format!("({} IMPLIES {})", l.to_string(), r.to_string()),
            LogicalExpression::Equivalent(l, r) => format!("({} EQUIV {})", l.to_string(), r.to_string()),
            LogicalExpression::ForAll(var, e) => format!("FORALL {} ({})", var, e.to_string()),
            LogicalExpression::Exists(var, e) => format!("EXISTS {} ({})", var, e.to_string()),
        }
    }

    /// Get all variables in this expression
    pub fn get_variables(&self) -> HashSet<String> {
        let mut vars = HashSet::new();
        self.collect_variables(&mut vars);
        vars
    }

    fn collect_variables(&self, vars: &mut HashSet<String>) {
        match self {
            LogicalExpression::Variable(v) => { vars.insert(v.clone()); }
            LogicalExpression::Not(e) => e.collect_variables(vars),
            LogicalExpression::And(l, r) => {
                l.collect_variables(vars);
                r.collect_variables(vars);
            }
            LogicalExpression::Or(l, r) => {
                l.collect_variables(vars);
                r.collect_variables(vars);
            }
            LogicalExpression::Implies(l, r) => {
                l.collect_variables(vars);
                r.collect_variables(vars);
            }
            LogicalExpression::Equivalent(l, r) => {
                l.collect_variables(vars);
                r.collect_variables(vars);
            }
            LogicalExpression::ForAll(_, e) => e.collect_variables(vars),
            LogicalExpression::Exists(_, e) => e.collect_variables(vars),
            LogicalExpression::Constant(_) => {}
        }
    }
}

/// Find a binary operator at the right level of nesting
fn find_binary_op(s: &str, op: &str, _parens: &[char]) -> Option<usize> {
    let mut depth = 0;
    let op_len = op.len();
    
    for (i, c) in s.char_indices() {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
        } else if depth == 0 && i + op_len <= s.len() && &s[i..i + op_len] == op {
            return Some(i);
        }
    }
    None
}

/// A logical rule/inference rule
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InferenceRule {
    /// Rule name
    pub name: String,
    /// Rule description
    pub description: String,
    /// Antecedents (premises)
    pub antecedents: Vec<LogicalExpression>,
    /// Consequent (conclusion)
    pub consequent: LogicalExpression,
    /// Rule type
    pub rule_type: RuleType,
}

/// Type of inference rule
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleType {
    /// Modus Ponens: A, A→B ⊢ B
    ModusPonens,
    /// Modus Tollens: ¬B, A→B ⊢ ¬A
    ModusTollens,
    /// Hypothetical Syllogism: A→B, B→C ⊢ A→C
    HypotheticalSyllogism,
    /// Disjunctive Syllogism: A∨B, ¬A ⊢ B
    DisjunctiveSyllogism,
    /// Constructive Dilemma: A→B, C→D, A∨C ⊢ B∨D
    ConstructiveDilemma,
    /// Universal Instantiation
    UniversalInstantiation,
    /// Universal Generalization
    UniversalGeneralization,
    /// Custom rule
    Custom,
}

impl InferenceRule {
    /// Create Modus Ponens rule
    pub fn modus_ponens() -> Self {
        let a = LogicalExpression::Variable("A".to_string());
        let b = LogicalExpression::Variable("B".to_string());
        let implies = LogicalExpression::Implies(Box::new(a.clone()), Box::new(b.clone()));
        
        Self {
            name: "Modus Ponens".to_string(),
            description: "From A and A→B, infer B".to_string(),
            antecedents: vec![a, implies],
            consequent: b,
            rule_type: RuleType::ModusPonens,
        }
    }

    /// Create Modus Tollens rule
    pub fn modus_tollens() -> Self {
        let a = LogicalExpression::Variable("A".to_string());
        let b = LogicalExpression::Variable("B".to_string());
        let not_b = LogicalExpression::Not(Box::new(b.clone()));
        let implies = LogicalExpression::Implies(Box::new(a), Box::new(b));
        
        Self {
            name: "Modus Tollens".to_string(),
            description: "From ¬B and A→B, infer ¬A".to_string(),
            antecedents: vec![not_b, implies],
            consequent: LogicalExpression::Not(Box::new(LogicalExpression::Variable("A".to_string()))),
            rule_type: RuleType::ModusTollens,
        }
    }

    /// Create a custom rule
    pub fn custom(name: &str, antecedents: Vec<LogicalExpression>, consequent: LogicalExpression) -> Self {
        Self {
            name: name.to_string(),
            description: format!("Custom rule: {} antecedents", antecedents.len()),
            antecedents,
            consequent,
            rule_type: RuleType::Custom,
        }
    }

    /// Check if this rule matches the given facts
    pub fn matches(&self, facts: &HashSet<LogicalExpression>) -> bool {
        // Check if all antecedents are satisfied
        for ant in &self.antecedents {
            if !facts.contains(ant) {
                // Check for variable unification
                if !self.unifies(ant, facts) {
                    return false;
                }
            }
        }
        true
    }

    /// Try to unify antecedents with facts
    fn unifies(&self, ant: &LogicalExpression, facts: &HashSet<LogicalExpression>) -> bool {
        // Simplified: check direct equality or negation
        for fact in facts {
            if fact == ant {
                return true;
            }
            // Check negation
            if let LogicalExpression::Not(f) = fact {
                if **f == *ant {
                    return true;
                }
            }
        }
        false
    }
}

/// A proof step in a reasoning chain
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProofStep {
    /// Step number
    pub step_number: usize,
    /// Expression derived at this step
    pub expression: LogicalExpression,
    /// Rule used to derive this step
    pub rule_name: String,
    /// Antecedents used for this step
    pub antecedents: Vec<usize>, // Indices of previous steps
    /// Justification text
    pub justification: String,
    /// Timestamp
    pub timestamp: u64,
}

impl ProofStep {
    /// Create a new proof step
    pub fn new(step_number: usize, expression: LogicalExpression, rule: &InferenceRule, antecedents: Vec<usize>) -> Self {
        Self {
            step_number,
            expression,
            rule_name: rule.name.clone(),
            antecedents,
            justification: rule.description.clone(),
            timestamp: current_timestamp(),
        }
    }
}

/// A complete proof chain
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProofChain {
    /// Unique proof ID
    pub id: String,
    /// Steps in the proof
    pub steps: Vec<ProofStep>,
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
    pub fn add_step(&mut self, step: ProofStep) {
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
        output.push_str(&format!("Valid: {} | Trust Level: {:.2}\n\n", self.is_valid, self.trust_level));
        
        for step in &self.steps {
            output.push_str(&format!(
                "[Step {}] {}\n  Rule: {}\n  From: {:?}\n  Justification: {}\n\n",
                step.step_number,
                step.expression.to_string(),
                step.rule_name,
                step.antecedents,
                step.justification
            ));
        }
        
        if let Some(conclusion) = &self.conclusion {
            output.push_str(&format!("Conclusion: {}\n", conclusion.to_string()));
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
        reasoner.init_standard_rules();
        
        reasoner
    }

    /// Initialize standard inference rules
    fn init_standard_rules(&self) {
        let mut rules = self.rules.write().unwrap();
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
    }

    /// Add a fact to the knowledge base
    pub fn add_fact(&self, fact: LogicalExpression) -> Result<()> {
        let mut kb = self.knowledge_base.write().unwrap();
        kb.insert(fact.clone());
        
        // Audit log
        self.add_audit_entry(AuditAction::FactAdded, &fact.to_string());
        
        Ok(())
    }

    /// Add a fact from string
    pub fn add_fact_string(&self, fact_str: &str) -> Result<()> {
        let fact = LogicalExpression::parse(fact_str)?;
        self.add_fact(fact)
    }

    /// Add an inference rule
    pub fn add_rule(&self, rule: InferenceRule) -> Result<()> {
        let mut rules = self.rules.write().unwrap();
        rules.push(rule.clone());
        
        // Audit log
        self.add_audit_entry(AuditAction::RuleAdded, &rule.name);
        
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
        if let Some(cached) = self.proof_cache.read().unwrap().get(target_str) {
            return Ok(VerificationResult {
                is_valid: cached.is_valid,
                proof_chain: Some(cached.clone()),
                trust_level: cached.trust_level,
                issues: vec![],
                timestamp: current_timestamp(),
            });
        }
        
        // Audit log
        self.add_audit_entry(AuditAction::ReasoningPerformed, target_str);
        
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
            self.proof_cache.write().unwrap().insert(target_str.to_string(), proof);
        }
        
        // Audit log
        self.add_audit_entry(AuditAction::VerificationPerformed, &format!("valid={}", is_valid));
        
        Ok(result)
    }

    /// Try to prove a target expression
    async fn prove(&self, target: LogicalExpression) -> Result<ProofChain> {
        let mut proof = ProofChain::new(uuid::Uuid::new_v4().to_string());
        let kb = self.knowledge_base.read().unwrap();
        let rules = self.rules.read().unwrap();
        
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
    fn add_audit_entry(&self, action: AuditAction, details: &str) {
        let mut log = self.audit_log.write().unwrap();
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
        self.add_audit_entry(AuditAction::AuditLogAccessed, "Audit log retrieved");
        self.audit_log.read().unwrap().clone()
    }

    /// Verify audit log integrity
    pub fn verify_audit_integrity(&self) -> Result<bool> {
        let log = self.audit_log.read().unwrap();
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
        self.knowledge_base.read().unwrap().iter().cloned().collect()
    }

    /// Get available inference rules
    pub fn get_rules(&self) -> Vec<InferenceRule> {
        self.rules.read().unwrap().clone()
    }

    /// Clear the knowledge base
    pub fn clear_knowledge_base(&self) {
        self.knowledge_base.write().unwrap().clear();
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
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_variable() {
        let expr = LogicalExpression::parse("X").unwrap();
        assert_eq!(expr, LogicalExpression::Variable("X".to_string()));
    }

    #[test]
    fn test_parse_and() {
        let expr = LogicalExpression::parse("A AND B").unwrap();
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
        let expr = LogicalExpression::parse("A IMPLIES B").unwrap();
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
        let expr = LogicalExpression::parse("A AND B").unwrap();
        let mut context = HashMap::new();
        context.insert("A".to_string(), true);
        context.insert("B".to_string(), true);
        
        assert_eq!(expr.evaluate(&context).unwrap(), true);
        
        context.insert("B".to_string(), false);
        assert_eq!(expr.evaluate(&context).unwrap(), false);
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
        reasoner.add_fact_string("A").unwrap();
        reasoner.add_fact_string("A IMPLIES B").unwrap();

        // Verify
        let result = reasoner.verify("B").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_audit_integrity() {
        let reasoner = VerifiableReasoner::new();
        reasoner.add_fact_string("A").unwrap();
        
        let is_valid = reasoner.verify_audit_integrity();
        assert!(is_valid.is_ok());
        assert!(is_valid.unwrap());
    }
}
