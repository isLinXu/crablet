//! Logical Expression types and Inference Rules
//!
//! Core types for the verifiable reasoning engine:
//! - LogicalExpression: Propositional and first-order logic expressions
//! - InferenceRule: Rules for logical deduction
//! - ProofStep: Individual steps in a proof chain

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
            return Self::parse(&s[1..s.len() - 1]);
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
            LogicalExpression::Variable(name) => context
                .get(name)
                .copied()
                .ok_or_else(|| anyhow!("Unknown variable: {}", name)),
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
            LogicalExpression::Implies(l, r) => {
                format!("({} IMPLIES {})", l.to_string(), r.to_string())
            }
            LogicalExpression::Equivalent(l, r) => {
                format!("({} EQUIV {})", l.to_string(), r.to_string())
            }
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
            LogicalExpression::Variable(v) => {
                vars.insert(v.clone());
            }
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
            consequent: LogicalExpression::Not(Box::new(LogicalExpression::Variable(
                "A".to_string(),
            ))),
            rule_type: RuleType::ModusTollens,
        }
    }

    /// Create a custom rule
    pub fn custom(
        name: &str,
        antecedents: Vec<LogicalExpression>,
        consequent: LogicalExpression,
    ) -> Self {
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
    pub fn new(
        step_number: usize,
        expression: LogicalExpression,
        rule: &InferenceRule,
        antecedents: Vec<usize>,
    ) -> Self {
        Self {
            step_number,
            expression,
            rule_name: rule.name.clone(),
            antecedents,
            justification: rule.description.clone(),
            timestamp: {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0)
        },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_variable() {
        let expr = LogicalExpression::parse("rain").unwrap();
        assert_eq!(expr, LogicalExpression::Variable("rain".to_string()));
    }

    #[test]
    fn test_parse_and() {
        let expr = LogicalExpression::parse("rain AND cold").unwrap();
        match expr {
            LogicalExpression::And(_, _) => {}
            _ => panic!("Expected And expression"),
        }
    }

    #[test]
    fn test_parse_implies() {
        let expr = LogicalExpression::parse("rain IMPLIES wet").unwrap();
        match expr {
            LogicalExpression::Implies(_, _) => {}
            _ => panic!("Expected Implies expression"),
        }
    }

    #[test]
    fn test_evaluate_and() {
        let expr = LogicalExpression::parse("a AND b").unwrap();
        let ctx = HashMap::from([("a".to_string(), true), ("b".to_string(), true)]);
        assert!(expr.evaluate(&ctx).unwrap());
    }

    #[test]
    fn test_evaluate_or() {
        let expr = LogicalExpression::parse("a OR b").unwrap();
        let ctx = HashMap::from([("a".to_string(), true), ("b".to_string(), false)]);
        assert!(expr.evaluate(&ctx).unwrap());
    }

    #[test]
    fn test_get_variables() {
        let expr = LogicalExpression::parse("a AND b").unwrap();
        let vars = expr.get_variables();
        assert!(vars.contains("a"));
        assert!(vars.contains("b"));
    }

    #[test]
    fn test_modus_ponens() {
        let rule = InferenceRule::modus_ponens();
        assert_eq!(rule.rule_type, RuleType::ModusPonens);
    }
}
