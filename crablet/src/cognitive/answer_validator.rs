//! Answer Validation and Quality Assessment
//! 
//! Provides multi-layer validation for generated answers including:
//! - Factual consistency checking
//! - Relevance scoring
//! - Completeness assessment
//! - Hallucination detection

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Validation result with detailed metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub overall_score: f32,
    pub factual_consistency: f32,
    pub relevance: f32,
    pub completeness: f32,
    pub hallucination_risk: f32,
    pub issues: Vec<ValidationIssue>,
    pub suggestions: Vec<String>,
}

/// Types of validation issues
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationIssue {
    FactualInconsistency { detail: String, severity: Severity },
    IrrelevantContent { detail: String },
    IncompleteAnswer { missing: Vec<String> },
    PotentialHallucination { claim: String },
    UnclearStatement { statement: String },
    Contradiction { statement1: String, statement2: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// Answer validator with multiple validation strategies
pub struct AnswerValidator {
    /// Threshold for accepting an answer
    threshold: f32,
    /// Enable factual consistency check
    check_factual_consistency: bool,
    /// Enable relevance check
    check_relevance: bool,
    /// Enable completeness check
    check_completeness: bool,
    /// Enable hallucination detection
    detect_hallucinations: bool,
}

impl AnswerValidator {
    /// Create a new validator with default settings
    pub fn new() -> Self {
        Self {
            threshold: 0.7,
            check_factual_consistency: true,
            check_relevance: true,
            check_completeness: true,
            detect_hallucinations: true,
        }
    }

    /// Set validation threshold
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Validate an answer against source context
    pub async fn validate(
        &self,
        answer: &str,
        query: &str,
        sources: &[SourceDocument],
    ) -> Result<ValidationResult> {
        let mut issues = Vec::new();
        let mut scores = ValidationScores::default();

        // 1. Factual Consistency Check
        if self.check_factual_consistency && !sources.is_empty() {
            scores.factual_consistency = self.check_factual_consistency(answer, sources, &mut issues);
        } else {
            scores.factual_consistency = 1.0; // Assume consistent if no sources
        }

        // 2. Relevance Check
        if self.check_relevance {
            scores.relevance = self.check_relevance(answer, query, &mut issues);
        }

        // 3. Completeness Check
        if self.check_completeness {
            scores.completeness = self.check_completeness(answer, query, &mut issues);
        }

        // 4. Hallucination Detection
        if self.detect_hallucinations {
            scores.hallucination_risk = self.detect_hallucinations(answer, sources, &mut issues);
        }

        // Calculate overall score
        let overall_score = self.calculate_overall_score(&scores);
        let is_valid = overall_score >= self.threshold;

        // Generate suggestions
        let suggestions = self.generate_suggestions(&issues, &scores);

        Ok(ValidationResult {
            is_valid,
            overall_score,
            factual_consistency: scores.factual_consistency,
            relevance: scores.relevance,
            completeness: scores.completeness,
            hallucination_risk: scores.hallucination_risk,
            issues,
            suggestions,
        })
    }

    /// Check factual consistency against sources
    fn check_factual_consistency(
        &self,
        answer: &str,
        sources: &[SourceDocument],
        issues: &mut Vec<ValidationIssue>,
    ) -> f32 {
        let claims = self.extract_claims(answer);
        let mut consistent_claims = 0;
        let mut total_claims = claims.len();

        for claim in &claims {
            let mut found_support = false;
            
            for source in sources {
                if self.claim_supported_by_source(claim, &source.content) {
                    found_support = true;
                    break;
                }
            }

            if !found_support {
                issues.push(ValidationIssue::FactualInconsistency {
                    detail: format!("Claim not found in sources: {}", claim),
                    severity: Severity::Medium,
                });
            } else {
                consistent_claims += 1;
            }
        }

        if total_claims == 0 {
            return 1.0;
        }

        consistent_claims as f32 / total_claims as f32
    }

    /// Extract factual claims from answer
    fn extract_claims(&self, answer: &str) -> Vec<String> {
        let mut claims = Vec::new();
        let sentences: Vec<&str> = answer.split(['.', '!', '?']).collect();

        for sentence in sentences {
            let trimmed = sentence.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Look for factual statements (containing numbers, dates, specific names)
            let has_fact_indicators = trimmed.chars().any(|c| c.is_numeric()) ||
                trimmed.contains("is") ||
                trimmed.contains("are") ||
                trimmed.contains("was") ||
                trimmed.contains("were") ||
                trimmed.contains("has") ||
                trimmed.contains("have");

            if has_fact_indicators && trimmed.len() > 10 {
                claims.push(trimmed.to_string());
            }
        }

        claims
    }

    /// Check if a claim is supported by a source
    fn claim_supported_by_source(&self, claim: &str, source: &str) -> bool {
        // Simplified implementation - would use semantic similarity in production
        let claim_lower = claim.to_lowercase();
        let source_lower = source.to_lowercase();

        // Extract key terms from claim
        let key_terms: Vec<&str> = claim_lower
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .collect();

        // Check if key terms appear in source
        let matching_terms = key_terms
            .iter()
            .filter(|term| source_lower.contains(*term))
            .count();

        let match_ratio = if key_terms.is_empty() {
            0.0
        } else {
            matching_terms as f32 / key_terms.len() as f32
        };

        match_ratio > 0.5
    }

    /// Check relevance to query
    fn check_relevance(&self, answer: &str, query: &str, issues: &mut Vec<ValidationIssue>) -> f32 {
        let answer_lower = answer.to_lowercase();
        let query_lower = query.to_lowercase();

        // Extract query keywords
        let query_keywords: Vec<&str> = query_lower
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .collect();

        // Check coverage of query keywords in answer
        let covered_keywords = query_keywords
            .iter()
            .filter(|kw| answer_lower.contains(*kw))
            .count();

        let coverage = if query_keywords.is_empty() {
            1.0
        } else {
            covered_keywords as f32 / query_keywords.len() as f32
        };

        // Check for off-topic content
        let answer_words: Vec<&str> = answer_lower.split_whitespace().collect();
        let off_topic_threshold = 0.3;
        let off_topic_words = answer_words
            .iter()
            .filter(|w| !query_lower.contains(*w) && w.len() > 4)
            .count();
        
        let off_topic_ratio = if answer_words.is_empty() {
            0.0
        } else {
            off_topic_words as f32 / answer_words.len() as f32
        };

        if off_topic_ratio > off_topic_threshold {
            issues.push(ValidationIssue::IrrelevantContent {
                detail: format!("Answer may contain off-topic content ({:.1}% unrelated)", 
                    off_topic_ratio * 100.0),
            });
        }

        // Combine scores
        let relevance = coverage * (1.0 - off_topic_ratio * 0.5);
        relevance.clamp(0.0, 1.0)
    }

    /// Check completeness of answer
    fn check_completeness(&self, answer: &str, query: &str, issues: &mut Vec<ValidationIssue>) -> f32 {
        let mut missing_aspects = Vec::new();
        let query_lower = query.to_lowercase();

        // Check for question words and expected answer components
        let question_indicators = [
            ("what", vec!["is", "are", "refers to", "means", "defined as"]),
            ("how", vec!["by", "through", "via", "using", "steps", "process"]),
            ("why", vec!["because", "due to", "reason", "causes"]),
            ("when", vec!["time", "date", "period", "during", "in"]),
            ("where", vec!["location", "place", "in", "at", "from"]),
            ("who", vec!["person", "people", "organization", "by"]),
            ("compare", vec!["difference", "similar", "versus", "better", "worse"]),
        ];

        for (indicator, expected_phrases) in &question_indicators {
            if query_lower.contains(indicator) {
                let has_expected = expected_phrases
                    .iter()
                    .any(|phrase| answer.to_lowercase().contains(phrase));
                
                if !has_expected {
                    missing_aspects.push(format!("Expected explanation for '{}' question", indicator));
                }
            }
        }

        // Check answer length adequacy
        let word_count = answer.split_whitespace().count();
        let expected_length = match query_lower.split_whitespace().count() {
            n if n < 5 => 20,
            n if n < 10 => 50,
            _ => 100,
        };

        if word_count < expected_length / 2 {
            missing_aspects.push("Answer seems too brief".to_string());
        }

        // Calculate completeness score before moving missing_aspects
        let base_score: f32 = 1.0;
        let deduction = missing_aspects.len() as f32 * 0.15;
        
        if !missing_aspects.is_empty() {
            issues.push(ValidationIssue::IncompleteAnswer { missing: missing_aspects });
        }

        (base_score - deduction).clamp(0.3, 1.0)
    }

    /// Detect potential hallucinations
    fn detect_hallucinations(
        &self,
        answer: &str,
        sources: &[SourceDocument],
        issues: &mut Vec<ValidationIssue>,
    ) -> f32 {
        let mut risk_score: f32 = 0.0;
        let answer_lower = answer.to_lowercase();

        // Check for specific hallucination indicators
        let hallucination_indicators = [
            "i think", "probably", "maybe", "perhaps", "might be",
            "i believe", "it seems", "appears to be", "could be",
            "我觉得", "可能", "也许", "大概",
        ];

        for indicator in &hallucination_indicators {
            if answer_lower.contains(indicator) {
                risk_score += 0.1;
            }
        }

        // Check for unsupported specific claims
        let specific_claims = self.extract_specific_claims(answer);
        for claim in &specific_claims {
            let mut supported = false;
            for source in sources {
                if self.claim_supported_by_source(claim, &source.content) {
                    supported = true;
                    break;
                }
            }
            if !supported {
                issues.push(ValidationIssue::PotentialHallucination {
                    claim: claim.clone(),
                });
                risk_score += 0.2;
            }
        }

        // Check for contradictions within the answer
        let contradictions = self.find_contradictions(answer);
        for (stmt1, stmt2) in contradictions {
            issues.push(ValidationIssue::Contradiction { statement1: stmt1, statement2: stmt2 });
            risk_score += 0.3;
        }

        risk_score.clamp(0.0, 1.0)
    }

    /// Extract specific claims that are more likely to be hallucinated
    fn extract_specific_claims(&self, answer: &str) -> Vec<String> {
        let mut claims = Vec::new();
        let sentences: Vec<&str> = answer.split(['.', '!', '?']).collect();

        for sentence in sentences {
            let trimmed = sentence.trim();
            
            // Look for specific numbers, dates, or statistics
            let has_specifics = trimmed.chars().any(|c| c.is_numeric()) ||
                trimmed.contains('%') ||
                trimmed.contains("percent") ||
                trimmed.contains("million") ||
                trimmed.contains("billion") ||
                trimmed.contains("thousand");

            if has_specifics && trimmed.len() > 15 {
                claims.push(trimmed.to_string());
            }
        }

        claims
    }

    /// Find contradictions within the answer
    fn find_contradictions(&self, answer: &str) -> Vec<(String, String)> {
        let mut contradictions = Vec::new();
        let sentences: Vec<&str> = answer.split(['.', '!', '?']).collect();

        // Simple contradiction detection - look for negations
        for (i, sentence1) in sentences.iter().enumerate() {
            let s1_lower = sentence1.to_lowercase();
            
            for sentence2 in &sentences[i + 1..] {
                let s2_lower = sentence2.to_lowercase();
                
                // Check for direct contradictions (e.g., "is" vs "is not")
                if self.are_contradictory(&s1_lower, &s2_lower) {
                    contradictions.push((sentence1.to_string(), sentence2.to_string()));
                }
            }
        }

        contradictions
    }

    /// Check if two statements are contradictory
    fn are_contradictory(&self, s1: &str, s2: &str) -> bool {
        // Simple heuristic: check for negation patterns
        let negations = ["not ", "no ", "never ", "n't "];
        
        // Extract core statement (simplified)
        let s1_core = s1.replace("not ", "").replace("no ", "").replace("n't ", "");
        let s2_core = s2.replace("not ", "").replace("no ", "").replace("n't ", "");

        // If cores are similar but one has negation
        let s1_has_negation = negations.iter().any(|n| s1.contains(n));
        let s2_has_negation = negations.iter().any(|n| s2.contains(n));

        if s1_has_negation != s2_has_negation {
            let similarity = strsim::jaro_winkler(&s1_core, &s2_core) as f32;
            return similarity > 0.7;
        }

        false
    }

    /// Calculate overall validation score
    fn calculate_overall_score(&self, scores: &ValidationScores) -> f32 {
        // Weighted combination
        let factual_weight = 0.35;
        let relevance_weight = 0.25;
        let completeness_weight = 0.25;
        let hallucination_penalty = 0.15;

        let base_score = scores.factual_consistency * factual_weight +
            scores.relevance * relevance_weight +
            scores.completeness * completeness_weight;

        // Apply hallucination penalty
        let penalty = scores.hallucination_risk * hallucination_penalty;
        
        (base_score - penalty).clamp(0.0, 1.0)
    }

    /// Generate improvement suggestions
    fn generate_suggestions(&self, issues: &[ValidationIssue], scores: &ValidationScores) -> Vec<String> {
        let mut suggestions = Vec::new();

        if scores.factual_consistency < 0.7 {
            suggestions.push("Verify facts against provided sources".to_string());
        }

        if scores.relevance < 0.7 {
            suggestions.push("Focus more directly on the user's question".to_string());
        }

        if scores.completeness < 0.7 {
            suggestions.push("Provide a more comprehensive answer".to_string());
        }

        if scores.hallucination_risk > 0.3 {
            suggestions.push("Avoid making unsupported claims".to_string());
        }

        for issue in issues {
            match issue {
                ValidationIssue::FactualInconsistency { detail, .. } => {
                    suggestions.push(format!("Fact check: {}", detail));
                }
                ValidationIssue::IncompleteAnswer { missing } => {
                    for m in missing {
                        suggestions.push(format!("Address: {}", m));
                    }
                }
                ValidationIssue::Contradiction { .. } => {
                    suggestions.push("Resolve contradictions in the answer".to_string());
                }
                _ => {}
            }
        }

        suggestions
    }
}

impl Default for AnswerValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Validation scores
#[derive(Debug, Default)]
struct ValidationScores {
    factual_consistency: f32,
    relevance: f32,
    completeness: f32,
    hallucination_risk: f32,
}

/// Source document for validation
#[derive(Debug, Clone)]
pub struct SourceDocument {
    pub id: String,
    pub content: String,
    pub metadata: serde_json::Value,
}

/// Multi-stage answer generator with validation
pub struct ValidatedAnswerGenerator {
    validator: AnswerValidator,
    max_iterations: usize,
    improvement_threshold: f32,
}

impl ValidatedAnswerGenerator {
    pub fn new() -> Self {
        Self {
            validator: AnswerValidator::new(),
            max_iterations: 3,
            improvement_threshold: 0.1,
        }
    }

    /// Generate and validate answer iteratively
    pub async fn generate_and_validate<F, Fut>(
        &self,
        query: &str,
        sources: &[SourceDocument],
        generator: F,
    ) -> Result<(String, ValidationResult)>
    where
        F: Fn(&str, &[SourceDocument], Option<&[String]>) -> Fut,
        Fut: std::future::Future<Output = Result<String>>,
    {
        let mut best_answer: Option<String> = None;
        let mut best_validation: Option<ValidationResult> = None;

        for iteration in 0..self.max_iterations {
            // Generate answer
            let suggestions = best_validation.as_ref().map(|v| v.suggestions.clone());
            let answer = generator(query, sources, suggestions.as_deref()).await?;

            // Validate answer
            let validation = self.validator.validate(&answer, query, sources).await?;

            info!(
                "Iteration {}: score={:.2}, valid={}",
                iteration + 1,
                validation.overall_score,
                validation.is_valid
            );

            // Track best answer
            let is_better = best_validation.as_ref()
                .map(|best| validation.overall_score > best.overall_score)
                .unwrap_or(true);

            if is_better {
                best_answer = Some(answer);
                best_validation = Some(validation.clone());
            }

            // Check if good enough
            if validation.is_valid && validation.overall_score > 0.85 {
                break;
            }

            // Check for improvement
            if iteration > 0 {
                let prev_score = best_validation.as_ref().map(|v| v.overall_score).unwrap_or(0.0);
                let improvement = validation.overall_score - prev_score;
                if improvement < self.improvement_threshold {
                    break; // Not improving, stop iterating
                }
            }
        }

        Ok((best_answer.unwrap_or_default(), best_validation.unwrap()))
    }
}

impl Default for ValidatedAnswerGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_claims() {
        let validator = AnswerValidator::new();
        let answer = "Rust is a systems programming language. It was created in 2010.";
        let claims = validator.extract_claims(answer);
        assert!(!claims.is_empty());
    }

    #[test]
    fn test_check_relevance() {
        let validator = AnswerValidator::new();
        let answer = "Python is a programming language with simple syntax.";
        let query = "What is Python?";
        let mut issues = Vec::new();
        let score = validator.check_relevance(answer, query, &mut issues);
        assert!(score > 0.5);
    }

    #[test]
    fn test_find_contradictions() {
        let validator = AnswerValidator::new();
        let answer = "Rust is fast. Rust is not fast.";
        let contradictions = validator.find_contradictions(answer);
        assert!(!contradictions.is_empty());
    }

    #[test]
    fn test_overall_score_calculation() {
        let validator = AnswerValidator::new();
        let scores = ValidationScores {
            factual_consistency: 0.9,
            relevance: 0.8,
            completeness: 0.85,
            hallucination_risk: 0.1,
        };
        let overall = validator.calculate_overall_score(&scores);
        assert!(overall > 0.7 && overall <= 1.0);
    }
}
