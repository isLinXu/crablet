//! Semantic Skill Matcher
//! 
//! Provides semantic matching capabilities for skills using embeddings and similarity search.
//! This enables matching user queries to skills based on meaning rather than exact keyword matches.

use std::collections::HashMap;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

// Note: Embedder is conditionally available with the "knowledge" feature
#[cfg(feature = "knowledge")]
use crate::knowledge::embedder::{Embedder, cosine_similarity};

/// Match result with confidence score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMatch {
    pub skill_name: String,
    pub confidence: f32,
    pub matched_keywords: Vec<String>,
    pub semantic_score: f32,
    pub keyword_score: f32,
}

/// Semantic matcher for skills
#[derive(Clone)]
pub struct SemanticMatcher {
    /// Pre-computed skill embeddings
    skill_embeddings: HashMap<String, Vec<f32>>,
    /// Skill metadata for keyword matching
    skill_metadata: HashMap<String, SkillMetadata>,
    /// Embedding model (only available with knowledge feature)
    #[cfg(feature = "knowledge")]
    embedder: Option<Embedder>,
    #[cfg(not(feature = "knowledge"))]
    _embedder: Option<()>,
    /// Threshold for considering a match valid
    threshold: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    pub keywords: Vec<String>,
    pub examples: Vec<String>,
}

impl SemanticMatcher {
    /// Create a new semantic matcher
    pub fn new() -> Self {
        Self {
            skill_embeddings: HashMap::new(),
            skill_metadata: HashMap::new(),
            #[cfg(feature = "knowledge")]
            embedder: None,
            #[cfg(not(feature = "knowledge"))]
            _embedder: None,
            threshold: 0.65, // Default threshold
        }
    }

    /// Set custom threshold
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Initialize with embedder
    #[cfg(feature = "knowledge")]
    pub async fn with_embedder(mut self, embedder: crate::knowledge::embedder::Embedder) -> Result<Self> {
        self.embedder = Some(embedder);
        Ok(self)
    }

    /// Register a skill with its metadata
    pub async fn register_skill(
        &mut self,
        name: &str,
        description: &str,
        keywords: Vec<String>,
        examples: Vec<String>,
    ) -> Result<()> {
        let metadata = SkillMetadata {
            name: name.to_string(),
            description: description.to_string(),
            keywords: keywords.clone(),
            examples: examples.clone(),
        };

        // Compute embedding for the skill
        #[cfg(feature = "knowledge")]
        if let Some(ref embedder) = self.embedder {
            // Combine description, keywords, and examples for rich semantic representation
            let skill_text = format!(
                "{} {} {} {}",
                description,
                keywords.join(" "),
                examples.join(" "),
                name.replace("_", " ")
            );
            
            let embedding = embedder.embed(&skill_text).await?;
            self.skill_embeddings.insert(name.to_string(), embedding);
            tracing::debug!("Registered skill '{}' with semantic embedding", name);
        }

        self.skill_metadata.insert(name.to_string(), metadata);
        Ok(())
    }

    /// Find matching skills for a query using hybrid approach
    pub async fn find_matches(&self, query: &str, top_k: usize) -> Result<Vec<SemanticMatch>> {
        let mut matches: Vec<SemanticMatch> = Vec::new();

        // 1. Semantic matching using embeddings
        let semantic_matches = self.semantic_match(query, top_k * 2).await?;
        
        // 2. Keyword matching
        let keyword_matches = self.keyword_match(query);

        // 3. Combine and score
        let mut combined_scores: HashMap<String, (f32, f32, Vec<String>)> = HashMap::new();

        // Add semantic scores
        for (skill_name, score) in semantic_matches {
            combined_scores.insert(skill_name.clone(), (score, 0.0, vec![]));
        }

        // Add keyword scores
        for (skill_name, score, keywords) in keyword_matches {
            let entry = combined_scores.entry(skill_name.clone()).or_insert((0.0, 0.0, vec![]));
            entry.1 = score;
            entry.2 = keywords;
        }

        // Calculate final scores using weighted combination
        for (skill_name, (semantic_score, keyword_score, matched_keywords)) in combined_scores {
            // Weight: 60% semantic, 40% keyword
            let confidence = semantic_score * 0.6 + keyword_score * 0.4;
            
            if confidence >= self.threshold {
                matches.push(SemanticMatch {
                    skill_name,
                    confidence,
                    matched_keywords,
                    semantic_score,
                    keyword_score,
                });
            }
        }

        // Sort by confidence and take top_k
        matches.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        matches.truncate(top_k);

        Ok(matches)
    }

    /// Semantic matching using cosine similarity
    #[cfg(feature = "knowledge")]
    async fn semantic_match(&self, query: &str, top_k: usize) -> Result<Vec<(String, f32)>> {
        if self.skill_embeddings.is_empty() || self.embedder.is_none() {
            return Ok(vec![]);
        }

        let embedder = self.embedder.as_ref().unwrap();
        let query_embedding = embedder.embed(query).await?;

        let mut similarities: Vec<(String, f32)> = self
            .skill_embeddings
            .iter()
            .map(|(skill_name, skill_embedding)| {
                let similarity = cosine_similarity(&query_embedding, skill_embedding);
                (skill_name.clone(), similarity)
            })
            .collect();

        // Sort by similarity descending
        similarities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        similarities.truncate(top_k);

        Ok(similarities)
    }

    /// Semantic matching stub when knowledge feature is disabled
    #[cfg(not(feature = "knowledge"))]
    async fn semantic_match(&self, _query: &str, _top_k: usize) -> Result<Vec<(String, f32)>> {
        Ok(vec![])
    }

    /// Keyword-based matching with fuzzy matching support
    fn keyword_match(&self, query: &str) -> Vec<(String, f32, Vec<String>)> {
        let query_lower = query.to_lowercase();
        let query_words: Vec<&str> = query_lower.split_whitespace().collect();
        
        let mut matches: Vec<(String, f32, Vec<String>)> = Vec::new();

        for (skill_name, metadata) in &self.skill_metadata {
            let mut matched_keywords = Vec::new();
            let mut total_score = 0.0;

            // Check description match
            if metadata.description.to_lowercase().contains(&query_lower) {
                total_score += 0.3;
                matched_keywords.push("description".to_string());
            }

            // Check keyword matches
            for keyword in &metadata.keywords {
                let keyword_lower = keyword.to_lowercase();
                
                // Exact match
                if query_lower.contains(&keyword_lower) {
                    total_score += 0.4;
                    matched_keywords.push(keyword.clone());
                    continue;
                }

                // Word-level match
                for word in &query_words {
                    if keyword_lower.contains(word) || word.contains(&keyword_lower) {
                        total_score += 0.2;
                        matched_keywords.push(keyword.clone());
                        break;
                    }
                }
            }

            // Check example matches
            for example in &metadata.examples {
                let example_lower = example.to_lowercase();
                let similarity = strsim::jaro_winkler(&query_lower, &example_lower) as f32;
                if similarity > 0.7 {
                    total_score += similarity * 0.3;
                    matched_keywords.push(format!("example: {}", &example[..example.len().min(30)]));
                }
            }

            // Name match bonus
            if metadata.name.to_lowercase().contains(&query_lower) ||
               query_lower.contains(&metadata.name.to_lowercase()) {
                total_score += 0.5;
                matched_keywords.push("name_match".to_string());
            }

            if total_score > 0.0 {
                // Normalize score to 0-1 range
                let normalized_score = (total_score / 2.0).min(1.0);
                matches.push((skill_name.clone(), normalized_score, matched_keywords));
            }
        }

        matches
    }

    /// Get skill suggestions for partial queries (autocomplete)
    pub fn suggest_skills(&self, partial_query: &str, limit: usize) -> Vec<String> {
        let partial_lower = partial_query.to_lowercase();
        
        let mut suggestions: Vec<(String, f32)> = self
            .skill_metadata
            .iter()
            .map(|(name, metadata)| {
                let name_lower = name.to_lowercase();
                let desc_lower = metadata.description.to_lowercase();
                
                let mut score = 0.0;
                
                // Name starts with query
                if name_lower.starts_with(&partial_lower) {
                    score += 1.0;
                }
                // Name contains query
                else if name_lower.contains(&partial_lower) {
                    score += 0.8;
                }
                // Description contains query
                else if desc_lower.contains(&partial_lower) {
                    score += 0.5;
                }
                // Keyword contains query
                else if metadata.keywords.iter().any(|k| k.to_lowercase().contains(&partial_lower)) {
                    score += 0.6;
                }
                
                (name.clone(), score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        suggestions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        suggestions.truncate(limit);
        
        suggestions.into_iter().map(|(name, _)| name).collect()
    }

    /// Update threshold based on historical performance
    pub fn adapt_threshold(&mut self, success_rate: f32) {
        // Adjust threshold based on success rate
        // If success rate is low, lower threshold to increase recall
        // If success rate is high, raise threshold to increase precision
        let target_rate = 0.8;
        let adjustment = (success_rate - target_rate) * 0.1;
        self.threshold = (self.threshold - adjustment).clamp(0.3, 0.9);
        info!("Adapted semantic matcher threshold to {:.2}", self.threshold);
    }

    /// Get current threshold
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Clear all registered skills
    pub fn clear(&mut self) {
        self.skill_embeddings.clear();
        self.skill_metadata.clear();
    }
}

impl Default for SemanticMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple cosine similarity implementation for tests
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        dot_product / (norm_a * norm_b)
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c)).abs() < 0.001);

        let d = vec![1.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &d) > 0.7);
    }

    #[test]
    fn test_keyword_match() {
        let mut matcher = SemanticMatcher::new();
        
        // Register a skill synchronously for testing
        let metadata = SkillMetadata {
            name: "weather".to_string(),
            description: "Get weather information".to_string(),
            keywords: vec!["weather".to_string(), "temperature".to_string(), "forecast".to_string()],
            examples: vec!["What's the weather today?".to_string()],
        };
        matcher.skill_metadata.insert("weather".to_string(), metadata);

        let matches = matcher.keyword_match("weather today");
        assert!(!matches.is_empty());
        assert_eq!(matches[0].0, "weather");
    }
}
