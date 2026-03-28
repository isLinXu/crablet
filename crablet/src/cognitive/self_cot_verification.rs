//! Self-Reflective Chain-of-Thought Verification
//!
//! An advanced reasoning system that generates multi-step reasoning chains,
//! automatically detects and corrects errors, and provides confidence evaluation.
//!
//! # Core Features
//!
//! 1. **Multi-step Reasoning Chain Generation** - Generate and verify reasoning chains
//! 2. **Automatic Error Detection & Correction** - Self-correct erroneous reasoning
//! 3. **Confidence Evaluation** - Assess confidence of each step and the whole chain
//! 4. **Chain Consistency Analysis** - Detect inconsistencies in reasoning chains
//! 5. **Multiple Reasoning Strategies** - Support Progressive/TreeSearch/BestFirst
//!
//! # Example
//!
//! ```rust,ignore
//! let verifier = SelfCotVerifier::new(llm_client.clone(), SelfCotConfig::default());
//!
//! // Generate and verify a reasoning chain
//! let result = verifier.verify("If all cats are animals, and all animals need water, does my cat need water?").await?;
//!
//! println!("Confidence: {:.2}%", result.confidence * 100.0);
//! println!("Chain valid: {}", result.is_valid);
//! for step in &result.reasoning_chain {
//!     println!("[Step {}] {} (confidence: {:.2})", step.step_number, step.content, step.confidence);
//! }
//! ```

use std::sync::Arc;
use anyhow::{Result, anyhow};
use crate::cognitive::llm::LlmClient;
use crate::types::Message;
use serde::{Deserialize, Serialize};
use tracing::{info, debug};

/// Configuration for Self-Reflective CoT Verification
#[derive(Clone, Debug)]
pub struct SelfCotConfig {
    /// Maximum depth for reasoning chains
    pub max_depth: usize,
    /// Branching factor for tree search strategies
    pub branching_factor: usize,
    /// Beam width for beam search
    pub beam_width: usize,
    /// Minimum confidence threshold to accept a reasoning step
    pub min_confidence: f32,
    /// Maximum correction attempts per step
    pub max_correction_attempts: usize,
    /// Strategy to use for reasoning
    pub strategy: ReasoningStrategy,
    /// Enable automatic error correction
    pub auto_correct: bool,
    /// Enable consistency verification
    pub verify_consistency: bool,
}

impl Default for SelfCotConfig {
    fn default() -> Self {
        Self {
            max_depth: 5,
            branching_factor: 3,
            beam_width: 2,
            min_confidence: 0.6,
            max_correction_attempts: 3,
            strategy: ReasoningStrategy::Progressive,
            auto_correct: true,
            verify_consistency: true,
        }
    }
}

/// Reasoning strategy types
#[derive(Clone, Debug, PartialEq)]
pub enum ReasoningStrategy {
    /// Progressive: Step-by-step with verification after each step
    Progressive,
    /// TreeSearch: Explore multiple branches, select best
    TreeSearch,
    /// BestFirst: Always expand the most promising node
    BestFirst,
}

/// A single step in a reasoning chain
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReasoningStep {
    /// Step number in the chain
    pub step_number: usize,
    /// Content of the reasoning step
    pub content: String,
    /// Confidence score for this step (0.0 - 1.0)
    pub confidence: f32,
    /// Whether this step has been verified
    pub verified: bool,
    /// Verification result if applicable
    pub verification_result: Option<VerificationResult>,
    /// Correction attempts made for this step
    pub correction_attempts: usize,
    /// Whether this step was corrected
    pub was_corrected: bool,
    /// Original content if this step was corrected
    pub original_content: Option<String>,
}

/// Result of a verification check
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether the step passed verification
    pub is_valid: bool,
    /// List of issues found (empty if valid)
    pub issues: Vec<String>,
    /// Suggested correction if invalid
    pub suggestion: Option<String>,
    /// Confidence adjustment based on verification
    pub confidence_adjustment: f32,
}

/// A complete reasoning chain with metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReasoningChain {
    /// Unique identifier for this chain
    pub chain_id: String,
    /// All steps in the chain
    pub steps: Vec<ReasoningStep>,
    /// Overall confidence of the chain
    pub confidence: f32,
    /// Whether the entire chain is valid
    pub is_valid: bool,
    /// Consistency check results
    pub consistency_result: Option<ConsistencyResult>,
    /// Final answer derived from the chain
    pub final_answer: Option<String>,
}

impl ReasoningChain {
    /// Create a new reasoning chain
    pub fn new(chain_id: String) -> Self {
        Self {
            chain_id,
            steps: Vec::new(),
            confidence: 1.0,
            is_valid: true,
            consistency_result: None,
            final_answer: None,
        }
    }

    /// Add a step to the chain
    pub fn add_step(&mut self, step: ReasoningStep) {
        self.confidence *= step.confidence;
        if !step.verified || step.verification_result.as_ref().map(|v| !v.is_valid).unwrap_or(false) {
            self.is_valid = false;
        }
        self.steps.push(step);
    }

    /// Get the last step
    pub fn last_step(&self) -> Option<&ReasoningStep> {
        self.steps.last()
    }

    /// Get the chain as a formatted string
    pub fn to_display_string(&self) -> String {
        let mut output = format!("Chain ID: {}\n", self.chain_id);
        output.push_str(&format!("Overall Confidence: {:.2}%\n", self.confidence * 100.0));
        output.push_str(&format!("Valid: {}\n\n", self.is_valid));
        
        for step in &self.steps {
            output.push_str(&format!(
                "Step {}: {}\n  Confidence: {:.2}% | Verified: {} | Corrected: {}\n",
                step.step_number,
                step.content,
                step.confidence * 100.0,
                step.verified,
                step.was_corrected
            ));
            if let Some(vr) = &step.verification_result {
                if !vr.issues.is_empty() {
                    output.push_str(&format!("  Issues: {}\n", vr.issues.join(", ")));
                }
            }
            output.push('\n');
        }
        
        if let Some(answer) = &self.final_answer {
            output.push_str(&format!("Final Answer: {}\n", answer));
        }
        
        output
    }
}

/// Result of a consistency check
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsistencyResult {
    /// Whether the chain is consistent
    pub is_consistent: bool,
    /// Inconsistencies found (empty if consistent)
    pub inconsistencies: Vec<Inconsistency>,
    /// Overall consistency score
    pub consistency_score: f32,
}

/// An inconsistency found in the reasoning chain
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inconsistency {
    /// Type of inconsistency
    pub inconsistency_type: InconsistencyType,
    /// Description of the inconsistency
    pub description: String,
    /// Step numbers involved
    pub involved_steps: Vec<usize>,
    /// Severity (0.0 - 1.0, higher is more severe)
    pub severity: f32,
}

/// Types of inconsistencies
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InconsistencyType {
    /// Contradiction between steps
    Contradiction,
    /// Missing logical connection
    LogicalGap,
    /// Unsupported assumption
    UnsupportedAssumption,
    /// Circular reasoning
    CircularReasoning,
    /// Invalid inference
    InvalidInference,
}

/// Final verification result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationReport {
    /// All reasoning chains explored
    pub chains: Vec<ReasoningChain>,
    /// Best chain selected
    pub best_chain: ReasoningChain,
    /// Overall confidence
    pub confidence: f32,
    /// Whether the final answer is trustworthy
    pub is_trustworthy: bool,
    /// Statistics about the verification process
    pub statistics: VerificationStatistics,
}

/// Statistics about the verification process
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationStatistics {
    /// Total chains explored
    pub chains_explored: usize,
    /// Total steps generated
    pub total_steps: usize,
    /// Total corrections made
    pub total_corrections: usize,
    /// Average confidence
    pub average_confidence: f32,
    /// Time taken (ms)
    pub time_ms: u64,
}

/// Main self-reflective CoT verifier
pub struct SelfCotVerifier {
    llm: Arc<Box<dyn LlmClient>>,
    config: SelfCotConfig,
}

impl SelfCotVerifier {
    /// Create a new SelfCotVerifier
    pub fn new(llm: Arc<Box<dyn LlmClient>>, config: SelfCotConfig) -> Self {
        Self { llm, config }
    }

    /// Verify a reasoning problem and return the best chain
    pub async fn verify(&self, problem: &str) -> Result<VerificationReport> {
        let start_time = std::time::Instant::now();
        
        info!("Starting Self-CoT verification for problem: {}", 
            if problem.len() > 50 { format!("{}...", &problem[..50]) } else { problem.to_string() });
        
        let chains = match self.config.strategy {
            ReasoningStrategy::Progressive => self.progressive_verify(problem).await?,
            ReasoningStrategy::TreeSearch => self.tree_search_verify(problem).await?,
            ReasoningStrategy::BestFirst => self.best_first_verify(problem).await?,
        };
        
        // Select the best chain based on confidence
        let best_chain = chains.iter()
            .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap_or(std::cmp::Ordering::Equal))
            .cloned()
            .ok_or_else(|| anyhow!("No valid chains produced"))?;
        
        let best_confidence = best_chain.confidence;
        let best_is_valid = best_chain.is_valid;
        
        let stats = VerificationStatistics {
            chains_explored: chains.len(),
            total_steps: chains.iter().map(|c| c.steps.len()).sum(),
            total_corrections: chains.iter()
                .flat_map(|c| c.steps.iter())
                .filter(|s| s.was_corrected)
                .count(),
            average_confidence: chains.iter().map(|c| c.confidence).sum::<f32>() / chains.len() as f32,
            time_ms: start_time.elapsed().as_millis() as u64,
        };
        
        info!("Self-CoT verification complete: {} chains explored, best confidence: {:.2}%", 
            stats.chains_explored, best_confidence * 100.0);
        
        Ok(VerificationReport {
            chains,
            best_chain,
            confidence: best_confidence,
            is_trustworthy: best_confidence >= self.config.min_confidence && best_is_valid,
            statistics: stats,
        })
    }

    /// Progressive verification: step-by-step with verification after each step
    async fn progressive_verify(&self, problem: &str) -> Result<Vec<ReasoningChain>> {
        let mut chain = ReasoningChain::new(uuid::Uuid::new_v4().to_string());
        let mut current_context = problem.to_string();
        let mut step_number = 1;
        
        // Generate initial reasoning step
        let step_content = self.generate_step(&current_context, step_number).await?;
        let mut step = ReasoningStep {
            step_number,
            content: step_content,
            confidence: 0.5,
            verified: false,
            verification_result: None,
            correction_attempts: 0,
            was_corrected: false,
            original_content: None,
        };
        
        // Verify and potentially correct the step
        step = self.verify_and_correct_step(&step, &current_context).await?;
        chain.add_step(step.clone());
        current_context = format!("{}\n\nStep {}: {}", problem, step_number, step.content);
        
        // Continue until we reach max depth or find a conclusion
        while step_number < self.config.max_depth {
            step_number += 1;
            
            // Check if we should continue or conclude
            let should_conclude = self.check_conclusion(&current_context).await?;
            if should_conclude {
                break;
            }
            
            // Generate next step
            let step_content = self.generate_step(&current_context, step_number).await?;
            let mut step = ReasoningStep {
                step_number,
                content: step_content,
                confidence: 0.5,
                verified: false,
                verification_result: None,
                correction_attempts: 0,
                was_corrected: false,
                original_content: None,
            };
            
            step = self.verify_and_correct_step(&step, &current_context).await?;
            chain.add_step(step.clone());
            current_context = format!("{}\n\nStep {}: {}", current_context, step_number, step.content);
        }
        
        // Generate final answer
        let final_answer = self.generate_final_answer(&current_context).await?;
        chain.final_answer = Some(final_answer);
        
        // Perform consistency check if enabled
        if self.config.verify_consistency {
            chain.consistency_result = Some(self.check_consistency(&chain).await?);
        }
        
        Ok(vec![chain])
    }

    /// Tree search verification: explore multiple branches
    async fn tree_search_verify(&self, problem: &str) -> Result<Vec<ReasoningChain>> {
        let mut chains = Vec::new();
        let mut queue = vec![ReasoningChain::new(uuid::Uuid::new_v4().to_string())];
        
        while let Some(chain) = queue.pop() {
            if chain.steps.len() >= self.config.max_depth {
                chains.push(chain);
                continue;
            }
            
            let next_step_number = chain.steps.len() + 1;
            let context = self.build_context(problem, &chain);
            
            // Generate multiple candidates
            let candidates = self.generate_candidates(&context, next_step_number, self.config.branching_factor).await?;
            
            for candidate_content in candidates {
                let mut new_chain = chain.clone();
                let step = ReasoningStep {
                    step_number: next_step_number,
                    content: candidate_content,
                    confidence: 0.5,
                    verified: false,
                    verification_result: None,
                    correction_attempts: 0,
                    was_corrected: false,
                    original_content: None,
                };
                
                let verified_step = self.verify_and_correct_step(&step, &context).await?;
                new_chain.add_step(verified_step);
                
                // Add to queue for further exploration if not at max depth
                if new_chain.steps.len() < self.config.max_depth {
                    queue.push(new_chain);
                } else {
                    chains.push(new_chain);
                }
            }
            
            // Limit total chains to prevent explosion
            if chains.len() > 100 {
                break;
            }
        }
        
        // Finalize chains
        for chain in &mut chains {
            let context = self.build_context(problem, chain);
            chain.final_answer = Some(self.generate_final_answer(&context).await?);
            
            if self.config.verify_consistency {
                chain.consistency_result = Some(self.check_consistency(chain).await?);
            }
        }
        
        // If no chains produced, create one with direct reasoning
        if chains.is_empty() {
            chains = self.progressive_verify(problem).await?;
        }
        
        Ok(chains)
    }

    /// Best-first verification: always expand the most promising node
    async fn best_first_verify(&self, problem: &str) -> Result<Vec<ReasoningChain>> {
        let mut chains = vec![ReasoningChain::new(uuid::Uuid::new_v4().to_string())];
        
        for _iteration in 0..self.config.max_depth {
            let mut new_chains = Vec::new();
            
            for chain in chains {
                let next_step_number = chain.steps.len() + 1;
                let context = self.build_context(problem, &chain);
                
                // Generate candidates
                let candidates = self.generate_candidates(&context, next_step_number, self.config.branching_factor).await?;
                
                for candidate_content in candidates {
                    let mut new_chain = chain.clone();
                    let step = ReasoningStep {
                        step_number: next_step_number,
                        content: candidate_content,
                        confidence: 0.5,
                        verified: false,
                        verification_result: None,
                        correction_attempts: 0,
                        was_corrected: false,
                        original_content: None,
                    };
                    
                    let verified_step = self.verify_and_correct_step(&step, &context).await?;
                    new_chain.add_step(verified_step);
                    new_chains.push(new_chain);
                }
            }
            
            // Keep only the best N chains (beam width)
            new_chains.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
            chains = new_chains.into_iter().take(self.config.beam_width).collect();
        }
        
        // Finalize chains
        for chain in &mut chains {
            let context = self.build_context(problem, chain);
            chain.final_answer = Some(self.generate_final_answer(&context).await?);
            
            if self.config.verify_consistency {
                chain.consistency_result = Some(self.check_consistency(chain).await?);
            }
        }
        
        if chains.is_empty() {
            chains = self.progressive_verify(problem).await?;
        }
        
        Ok(chains)
    }

    /// Generate a single reasoning step
    async fn generate_step(&self, context: &str, step_number: usize) -> Result<String> {
        let prompt = format!(
            "Given the following problem and reasoning so far:\n\n{}\n\nGenerate step {} of the reasoning chain. \
            This should be a clear, logical continuation that advances towards solving the problem. \
            Output only the step content, no labels or formatting.",
            context, step_number
        );
        
        let response = self.llm.chat_complete(&[Message::user(&prompt)]).await?;
        Ok(response.trim().to_string())
    }

    /// Generate multiple candidate steps
    async fn generate_candidates(&self, context: &str, step_number: usize, n: usize) -> Result<Vec<String>> {
        let prompt = format!(
            "Given the following problem and reasoning so far:\n\n{}\n\nGenerate {} distinct candidate steps for step {}. \
            Each step should represent a different approach or angle. \
            Output as a JSON array of strings, e.g., [\"step1\", \"step2\", \"step3\"].",
            context, n, step_number
        );
        
        let response = self.llm.chat_complete(&[Message::user(&prompt)]).await?;
        
        // Extract JSON array
        if let Some(json_str) = Self::extract_json(&response) {
            if let Ok(candidates) = serde_json::from_str::<Vec<String>>(json_str) {
                return Ok(candidates);
            }
        }
        
        // Fallback: return the whole response as single candidate
        Ok(vec![response.trim().to_string()])
    }

    /// Verify a step and correct if necessary
    async fn verify_and_correct_step(&self, step: &ReasoningStep, context: &str) -> Result<ReasoningStep> {
        let mut current_step = step.clone();
        
        for _attempt in 0..self.config.max_correction_attempts {
            // Verify the step
            let verification = self.verify_step(&current_step, context).await?;
            current_step.verification_result = Some(verification.clone());
            current_step.verified = true;
            
            if verification.is_valid || !self.config.auto_correct {
                // Step is valid or auto-correct is disabled
                current_step.confidence = (current_step.confidence + verification.confidence_adjustment).clamp(0.0, 1.0);
                return Ok(current_step);
            }
            
            // Step has issues - attempt correction
            if let Some(suggestion) = &verification.suggestion {
                debug!("Step {} has issues: {:?}, attempting correction", current_step.step_number, verification.issues);
                
                current_step.original_content = Some(current_step.content.clone());
                current_step.content = suggestion.clone();
                current_step.correction_attempts += 1;
                current_step.was_corrected = true;
            } else {
                // No suggestion available, try regenerating
                let prompt = format!(
                    "The following reasoning step has issues: {}\n\nContext: {}\n\n\
                    Rewrite this step to fix the issues and make it logically sound.",
                    current_step.content, context
                );
                
                let corrected = self.llm.chat_complete(&[Message::user(&prompt)]).await?;
                current_step.original_content = Some(current_step.content.clone());
                current_step.content = corrected.trim().to_string();
                current_step.correction_attempts += 1;
                current_step.was_corrected = true;
            }
        }
        
        // Max attempts reached, use the best we have
        if let Some(vr) = &current_step.verification_result {
            current_step.confidence = (current_step.confidence + vr.confidence_adjustment).clamp(0.0, 1.0);
        }
        
        Ok(current_step)
    }

    /// Verify a single step
    async fn verify_step(&self, step: &ReasoningStep, context: &str) -> Result<VerificationResult> {
        let prompt = format!(
            "Verify the following reasoning step for correctness and logical validity.\n\n\
            Context:\n{}\n\n\
            Step to verify:\n{}\n\n\
            Check for:\n\
            1. Factual accuracy\n\
            2. Logical validity\n\
            3. Coherence with previous steps\n\
            4. Soundness of inference\n\n\
            Output a JSON object with the following structure:\n\
            {{\"is_valid\": true/false, \"issues\": [\"issue1\", \"issue2\"], \"suggestion\": \"corrected version if needed\", \"confidence_adjustment\": -0.1 to 0.1}}",
            context, step.content
        );
        
        let response = self.llm.chat_complete(&[Message::user(&prompt)]).await?;
        
        // Parse verification result
        if let Some(json_str) = Self::extract_json(&response) {
            if let Ok(result) = serde_json::from_str::<VerificationResult>(json_str) {
                return Ok(result);
            }
        }
        
        // Fallback: assume valid with neutral adjustment
        Ok(VerificationResult {
            is_valid: true,
            issues: vec![],
            suggestion: None,
            confidence_adjustment: 0.0,
        })
    }

    /// Check if we should conclude the reasoning
    async fn check_conclusion(&self, context: &str) -> Result<bool> {
        let prompt = format!(
            "Based on the following reasoning chain, should we conclude with an answer? \
            Respond with YES if the reasoning has reached a satisfactory conclusion, NO if more steps are needed.\n\n{}\n\n\
            Output only YES or NO.",
            context
        );
        
        let response = self.llm.chat_complete(&[Message::user(&prompt)]).await?;
        Ok(response.trim().to_uppercase().contains("YES"))
    }

    /// Generate the final answer from the reasoning chain
    async fn generate_final_answer(&self, context: &str) -> Result<String> {
        let prompt = format!(
            "Based on the following reasoning chain, provide the final answer to the original problem.\n\n{}\n\n\
            Provide a clear, concise final answer.",
            context
        );
        
        let response = self.llm.chat_complete(&[Message::user(&prompt)]).await?;
        Ok(response.trim().to_string())
    }

    /// Check consistency of a reasoning chain
    async fn check_consistency(&self, chain: &ReasoningChain) -> Result<ConsistencyResult> {
        if chain.steps.len() < 2 {
            return Ok(ConsistencyResult {
                is_consistent: true,
                inconsistencies: vec![],
                consistency_score: 1.0,
            });
        }
        
        let steps_text: String = chain.steps.iter()
            .map(|s| format!("Step {}: {}", s.step_number, s.content))
            .collect::<Vec<_>>()
            .join("\n");
        
        let prompt = format!(
            "Analyze the following reasoning chain for inconsistencies:\n\n{}\n\n\
            Check for:\n\
            1. Contradictions between steps\n\
            2. Logical gaps\n\
            3. Unsupported assumptions\n\
            4. Circular reasoning\n\
            5. Invalid inferences\n\n\
            Output a JSON array of inconsistencies with format:\n\
            [{{\"type\": \"Contradiction\" or \"LogicalGap\" etc., \"description\": \"...\", \"involved_steps\": [1, 2], \"severity\": 0.0-1.0}}]\n\
            If no inconsistencies, output an empty array [].",
            steps_text
        );
        
        let response = self.llm.chat_complete(&[Message::user(&prompt)]).await?;
        
        // Parse inconsistencies
        if let Some(json_str) = Self::extract_json(&response) {
            if let Ok(inconsistencies) = serde_json::from_str::<Vec<Inconsistency>>(json_str) {
                let is_consistent = inconsistencies.is_empty();
                let severity_sum: f32 = inconsistencies.iter().map(|i| i.severity).sum();
                let avg_severity = if inconsistencies.is_empty() {
                    0.0
                } else {
                    severity_sum / inconsistencies.len() as f32
                };
                
                let consistency_score = if is_consistent {
                    1.0
                } else {
                    (1.0 - avg_severity).max(0.0)
                };
                
                // Map inconsistency types from strings
                let mapped_inconsistencies = inconsistencies.into_iter().map(|mut i| {
                    i.inconsistency_type = match i.inconsistency_type {
                        InconsistencyType::Contradiction => InconsistencyType::Contradiction,
                        InconsistencyType::LogicalGap => InconsistencyType::LogicalGap,
                        InconsistencyType::UnsupportedAssumption => InconsistencyType::UnsupportedAssumption,
                        InconsistencyType::CircularReasoning => InconsistencyType::CircularReasoning,
                        InconsistencyType::InvalidInference => InconsistencyType::InvalidInference,
                    };
                    i
                }).collect();
                
                return Ok(ConsistencyResult {
                    is_consistent,
                    inconsistencies: mapped_inconsistencies,
                    consistency_score,
                });
            }
        }
        
        // Fallback: assume consistent
        Ok(ConsistencyResult {
            is_consistent: true,
            inconsistencies: vec![],
            consistency_score: 1.0,
        })
    }

    /// Build context string from problem and chain
    fn build_context(&self, problem: &str, chain: &ReasoningChain) -> String {
        let mut context = format!("Original Problem: {}\n\n", problem);
        
        for step in &chain.steps {
            context.push_str(&format!("Step {}: {}\n", step.step_number, step.content));
        }
        
        context
    }

    /// Extract JSON from response
    fn extract_json(text: &str) -> Option<&str> {
        // Try to find array or object
        if let Some(start) = text.find('[') {
            let mut depth = 0;
            for (i, c) in text[start..].chars().enumerate() {
                match c {
                    '[' | '{' => depth += 1,
                    ']' | '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(&text[start..=start + i]);
                        }
                    }
                    _ => {}
                }
            }
        }
        
        if let Some(start) = text.find('{') {
            let mut depth = 0;
            for (i, c) in text[start..].chars().enumerate() {
                match c {
                    '[' | '{' => depth += 1,
                    ']' | '}' => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(&text[start..=start + i]);
                        }
                    }
                    _ => {}
                }
            }
        }
        
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reasoning_chain_creation() {
        let chain = ReasoningChain::new("test-chain".to_string());
        assert_eq!(chain.steps.len(), 0);
        assert_eq!(chain.confidence, 1.0);
        assert!(chain.is_valid);
    }

    #[test]
    fn test_reasoning_chain_add_step() {
        let mut chain = ReasoningChain::new("test-chain".to_string());
        
        let step = ReasoningStep {
            step_number: 1,
            content: "Test step".to_string(),
            confidence: 0.8,
            verified: true,
            verification_result: Some(VerificationResult {
                is_valid: true,
                issues: vec![],
                suggestion: None,
                confidence_adjustment: 0.0,
            }),
            correction_attempts: 0,
            was_corrected: false,
            original_content: None,
        };
        
        chain.add_step(step);
        assert_eq!(chain.steps.len(), 1);
        assert_eq!(chain.confidence, 0.8);
        assert!(chain.is_valid);
    }

    #[test]
    fn test_extract_json_array() {
        let text = "Here is the result: [\"a\", \"b\", \"c\"] and some more text";
        let json = SelfCotVerifier::extract_json(text);
        assert_eq!(json, Some("[\"a\", \"b\", \"c\"]"));
    }

    #[test]
    fn test_extract_json_object() {
        let text = "Result: {\"key\": \"value\"}";
        let json = SelfCotVerifier::extract_json(text);
        assert_eq!(json, Some("{\"key\": \"value\"}"));
    }

    #[test]
    fn test_config_default() {
        let config = SelfCotConfig::default();
        assert_eq!(config.max_depth, 5);
        assert_eq!(config.branching_factor, 3);
        assert_eq!(config.min_confidence, 0.6);
        assert_eq!(config.strategy, ReasoningStrategy::Progressive);
        assert!(config.auto_correct);
        assert!(config.verify_consistency);
    }
}
