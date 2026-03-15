//! Memory Weaver - Integrates and Consolidates Memories
//!
//! The Memory Weaver is responsible for:
//! - Extracting memories from sessions
//! - Consolidating similar memories
//! - Building connections between memories
//! - Optimizing memory storage

use std::collections::HashMap;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::Utc;
use tracing::{info, debug};

use crate::memory::fusion::MemoryError;
use crate::memory::fusion::layer_session::SessionLayer;
use crate::memory::fusion::layer_user::{Memory, MemoryType, create_memory_from_session};

/// Semantic memory configuration (local definition)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMemoryConfig {
    pub backend: String,
    pub enabled: bool,
}

/// Memory Weaver - Integrates memories across layers
pub struct MemoryWeaver {
    /// Configuration
    config: SemanticMemoryConfig,
    
    /// Extraction patterns
    extraction_patterns: RwLock<Vec<ExtractionPattern>>,
    
    /// Consolidation queue
    consolidation_queue: RwLock<Vec<Memory>>,
}

/// Extraction pattern for identifying memories
#[derive(Debug, Clone)]
pub struct ExtractionPattern {
    /// Pattern name
    pub name: String,
    
    /// Pattern type
    pub pattern_type: PatternType,
    
    /// Keywords to match
    pub keywords: Vec<String>,
    
    /// Memory category for extracted memories
    pub category: String,
    
    /// Importance boost
    pub importance_boost: f64,
}

/// Pattern type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternType {
    /// Explicit statement ("I like...", "I prefer...")
    Explicit,
    /// Implicit preference
    Implicit,
    /// Goal or intention
    Goal,
    /// Emotional state
    Emotional,
    /// Decision
    Decision,
}

/// Extracted memory candidate
#[derive(Debug, Clone)]
pub struct MemoryCandidate {
    /// Content
    pub content: String,
    
    /// Category
    pub category: String,
    
    /// Memory type
    pub memory_type: MemoryType,
    
    /// Importance score
    pub importance: f64,
    
    /// Source message
    pub source: String,
    
    /// Confidence
    pub confidence: f64,
}

/// Consolidation result
#[derive(Debug, Clone)]
pub struct ConsolidationResult {
    /// Memories merged
    pub merged: usize,
    
    /// Memories archived
    pub archived: usize,
    
    /// New connections created
    pub connections: usize,
}

impl MemoryWeaver {
    /// Create a new Memory Weaver
    pub fn new(config: SemanticMemoryConfig) -> Self {
        let patterns = Self::default_patterns();
        
        Self {
            config,
            extraction_patterns: RwLock::new(patterns),
            consolidation_queue: RwLock::new(Vec::new()),
        }
    }
    
    /// Default extraction patterns
    fn default_patterns() -> Vec<ExtractionPattern> {
        vec![
            // Explicit preferences
            ExtractionPattern {
                name: "explicit_preference".to_string(),
                pattern_type: PatternType::Explicit,
                keywords: vec![
                    "i like".to_string(),
                    "i prefer".to_string(),
                    "i want".to_string(),
                    "i need".to_string(),
                    "my favorite".to_string(),
                ],
                category: "preferences".to_string(),
                importance_boost: 0.3,
            },
            // Dislikes
            ExtractionPattern {
                name: "explicit_dislike".to_string(),
                pattern_type: PatternType::Explicit,
                keywords: vec![
                    "i don't like".to_string(),
                    "i dislike".to_string(),
                    "i hate".to_string(),
                    "i avoid".to_string(),
                ],
                category: "preferences".to_string(),
                importance_boost: 0.3,
            },
            // Goals
            ExtractionPattern {
                name: "goal_statement".to_string(),
                pattern_type: PatternType::Goal,
                keywords: vec![
                    "i want to".to_string(),
                    "my goal is".to_string(),
                    "i'm trying to".to_string(),
                    "i plan to".to_string(),
                ],
                category: "goals".to_string(),
                importance_boost: 0.4,
            },
            // Facts
            ExtractionPattern {
                name: "personal_fact".to_string(),
                pattern_type: PatternType::Explicit,
                keywords: vec![
                    "i am".to_string(),
                    "i work".to_string(),
                    "i live".to_string(),
                    "my name is".to_string(),
                ],
                category: "facts".to_string(),
                importance_boost: 0.5,
            },
            // Decisions
            ExtractionPattern {
                name: "decision".to_string(),
                pattern_type: PatternType::Decision,
                keywords: vec![
                    "i decided".to_string(),
                    "i chose".to_string(),
                    "i will".to_string(),
                    "let's go with".to_string(),
                ],
                category: "decisions".to_string(),
                importance_boost: 0.3,
            },
        ]
    }
    
    /// Extract memories from a session
    pub async fn extract_from_session(&self, session: &SessionLayer) -> Result<Vec<Memory>, MemoryError> {
        let messages = session.get_messages().await;
        let mut extracted = Vec::new();
        
        for message in messages {
            // Only extract from user messages
            if message.role != "user" {
                continue;
            }
            
            let content = message.text().unwrap_or_default().to_lowercase();
            let original_content = message.text().unwrap_or_default();
            
            // Check each pattern
            let patterns = self.extraction_patterns.read().await;
            for pattern in patterns.iter() {
                for keyword in &pattern.keywords {
                    if content.contains(keyword) {
                        // Extract the relevant part
                        if let Some(extracted_content) = self.extract_content(&original_content, keyword) {
                            let candidate = MemoryCandidate {
                                content: extracted_content,
                                category: pattern.category.clone(),
                                memory_type: self.pattern_to_memory_type(pattern.pattern_type),
                                importance: 0.5 + pattern.importance_boost,
                                source: original_content.clone(),
                                confidence: 0.7, // Base confidence
                            };
                            
                            // Convert to memory
                            let memory = self.candidate_to_memory(candidate, session.session_id());
                            extracted.push(memory);
                            
                            debug!("Extracted memory: {} (category: {})", 
                                extracted.last().unwrap().content, 
                                pattern.category
                            );
                        }
                        break; // Only extract once per pattern
                    }
                }
            }
        }
        
        info!("Extracted {} memories from session {}", extracted.len(), session.session_id());
        Ok(extracted)
    }
    
    /// Extract content around a keyword
    fn extract_content(&self, full_content: &str, keyword: &str) -> Option<String> {
        let lower_content = full_content.to_lowercase();
        
        if let Some(pos) = lower_content.find(keyword) {
            // Extract from keyword to end of sentence
            let start = pos;
            let rest = &full_content[start..];
            
            // Find sentence end
            let end = rest.find(|c: char| c == '.' || c == '!' || c == '?')
                .map(|i| i + 1)
                .unwrap_or(rest.len());
            
            let extracted = &rest[..end];
            
            // Clean up
            let cleaned = extracted.trim().to_string();
            
            if cleaned.len() > 10 { // Minimum length check
                return Some(cleaned);
            }
        }
        
        None
    }
    
    /// Convert pattern type to memory type
    fn pattern_to_memory_type(&self, pattern_type: PatternType) -> MemoryType {
        match pattern_type {
            PatternType::Explicit => MemoryType::ExplicitFact,
            PatternType::Implicit => MemoryType::Inferred,
            PatternType::Goal => MemoryType::Goal,
            PatternType::Emotional => MemoryType::Emotional,
            PatternType::Decision => MemoryType::Decision,
        }
    }
    
    /// Convert candidate to memory
    fn candidate_to_memory(&self, candidate: MemoryCandidate, session_id: &str) -> Memory {
        create_memory_from_session(
            candidate.content,
            candidate.category,
            session_id.to_string(),
        )
    }
    
    /// Add memory to consolidation queue
    pub async fn queue_for_consolidation(&self, memory: Memory) {
        let mut queue = self.consolidation_queue.write().await;
        queue.push(memory);
        
        // Trigger consolidation if queue is large enough
        if queue.len() >= 10 {
            drop(queue);
            let _ = self.consolidate().await;
        }
    }
    
    /// Consolidate memories in queue
    pub async fn consolidate(&self) -> Result<ConsolidationResult, MemoryError> {
        let mut queue = self.consolidation_queue.write().await;
        
        if queue.is_empty() {
            return Ok(ConsolidationResult {
                merged: 0,
                archived: 0,
                connections: 0,
            });
        }
        
        info!("Consolidating {} memories", queue.len());
        
        let mut merged = 0;
        let mut archived = 0;
        let mut connections = 0;
        
        // Group by category
        let mut by_category: HashMap<String, Vec<Memory>> = HashMap::new();
        for memory in queue.drain(..) {
            by_category
                .entry(memory.category.clone())
                .or_default()
                .push(memory);
        }
        
        // Merge similar memories within each category
        for (_category, mut memories) in by_category {
            memories.sort_by(|a, b| b.importance.partial_cmp(&a.importance).unwrap());
            
            let mut i = 0;
            while i < memories.len() {
                let mut j = i + 1;
                while j < memories.len() {
                    if self.are_similar(&memories[i], &memories[j]) {
                        // Merge memories[j] into memories[i]
                        memories[i].importance = (memories[i].importance + memories[j].importance) / 2.0;
                        memories[i].access_count += memories[j].access_count;
                        memories.remove(j);
                        merged += 1;
                    } else {
                        j += 1;
                    }
                }
                i += 1;
            }
            
            // Archive low-importance memories
            let cutoff = Utc::now() - chrono::Duration::days(30);
            let original_len = memories.len();
            memories.retain(|m| {
                m.importance > 0.3 || m.last_accessed > cutoff
            });
            archived += original_len - memories.len();
            
            // Create connections between related memories
            for i in 0..memories.len() {
                for j in (i + 1)..memories.len() {
                    if self.are_related(&memories[i], &memories[j]) {
                        connections += 1;
                    }
                }
            }
        }
        
        info!("Consolidation complete: {} merged, {} archived, {} connections", 
            merged, archived, connections
        );
        
        Ok(ConsolidationResult {
            merged,
            archived,
            connections,
        })
    }
    
    /// Check if two memories are similar
    fn are_similar(&self, a: &Memory, b: &Memory) -> bool {
        // Simple similarity check based on content overlap
        // In a real implementation, this would use embeddings
        
        let a_lower = a.content.to_lowercase();
        let b_lower = b.content.to_lowercase();
        let a_words: std::collections::HashSet<_> = a_lower
            .split_whitespace()
            .collect();
        let b_words: std::collections::HashSet<_> = b_lower
            .split_whitespace()
            .collect();
        
        let intersection: std::collections::HashSet<_> = a_words.intersection(&b_words).collect();
        let union: std::collections::HashSet<_> = a_words.union(&b_words).collect();
        
        if union.is_empty() {
            return false;
        }
        
        let jaccard = intersection.len() as f64 / union.len() as f64;
        jaccard > 0.7 // 70% similarity threshold
    }
    
    /// Check if two memories are related
    fn are_related(&self, a: &Memory, b: &Memory) -> bool {
        // Check for shared keywords
        let a_lower = a.content.to_lowercase();
        let b_lower = b.content.to_lowercase();
        let a_words: std::collections::HashSet<_> = a_lower
            .split_whitespace()
            .collect();
        let b_words: std::collections::HashSet<_> = b_lower
            .split_whitespace()
            .collect();
        
        let shared: std::collections::HashSet<_> = a_words.intersection(&b_words).collect();
        
        // Related if they share significant keywords
        shared.len() >= 3
    }
    
    /// Optimize memory storage
    pub async fn optimize(&self) -> Result<usize, MemoryError> {
        info!("Optimizing memory storage...");
        
        // In a real implementation, this would:
        // 1. Rebuild vector indices
        // 2. Compact storage
        // 3. Remove duplicates
        // 4. Archive old memories
        
        // For now, just consolidate
        let result = self.consolidate().await?;
        
        info!("Optimization complete: {} merged, {} archived", 
            result.merged, result.archived
        );
        
        Ok(result.merged + result.archived)
    }
    
    /// Add custom extraction pattern
    pub async fn add_pattern(&self, pattern: ExtractionPattern) {
        let mut patterns = self.extraction_patterns.write().await;
        patterns.push(pattern);
    }
    
    /// Get extraction patterns
    pub async fn get_patterns(&self) -> Vec<ExtractionPattern> {
        self.extraction_patterns.read().await.clone()
    }
}

/// Weaver statistics
#[derive(Debug, Clone)]
pub struct WeaverStats {
    pub patterns_count: usize,
    pub queue_size: usize,
    pub total_extracted: u64,
    pub total_consolidated: u64,
}
