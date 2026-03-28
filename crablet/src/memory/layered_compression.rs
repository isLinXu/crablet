//! Layered Memory Compression System
//!
//! A hierarchical memory management system that automatically compresses
//! and consolidates memories across different levels.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::info;

/// Memory levels in the hierarchy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryLevel {
    Working,
    Episodic,
    Semantic,
}

impl MemoryLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemoryLevel::Working => "working",
            MemoryLevel::Episodic => "episodic",
            MemoryLevel::Semantic => "semantic",
        }
    }
}

/// Compression trigger conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CompressionTrigger {
    TokenThreshold(u64),
    TimeIntervalSeconds(u64),
    Manual,
    Combined(Vec<CompressionTrigger>),
}

impl CompressionTrigger {
    pub fn should_trigger(&self, stats: &MemoryStats) -> bool {
        match self {
            CompressionTrigger::TokenThreshold(threshold) => {
                stats.total_tokens >= *threshold
            }
            CompressionTrigger::TimeIntervalSeconds(interval_secs) => {
                let elapsed = stats.last_compression.timestamp() as i64;
                let now = Utc::now().timestamp();
                (now - elapsed) >= *interval_secs as i64
            }
            CompressionTrigger::Manual => false,
            CompressionTrigger::Combined(triggers) => {
                triggers.iter().any(|t| t.should_trigger(stats))
            }
        }
    }
}

/// Memory statistics for compression decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_tokens: u64,
    pub working_count: usize,
    pub episodic_count: usize,
    pub semantic_count: usize,
    pub last_compression: DateTime<Utc>,
    pub compression_count: u64,
}

impl Default for MemoryStats {
    fn default() -> Self {
        Self {
            total_tokens: 0,
            working_count: 0,
            episodic_count: 0,
            semantic_count: 0,
            last_compression: Utc::now(),
            compression_count: 0,
        }
    }
}

impl MemoryStats {
    pub fn timestamp(&self) -> i64 {
        self.last_compression.timestamp()
    }
}

/// Compression configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub trigger: CompressionTrigger,
    pub working_token_limit: u64,
    pub episodic_archive_threshold: usize,
    pub semantic_extraction_ratio: f64,
    pub auto_compress: bool,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            trigger: CompressionTrigger::TokenThreshold(8000),
            working_token_limit: 10000,
            episodic_archive_threshold: 100,
            semantic_extraction_ratio: 0.3,
            auto_compress: true,
        }
    }
}

/// A compressed memory entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedEntry {
    pub id: String,
    pub original_level: MemoryLevel,
    pub target_level: MemoryLevel,
    pub content: String,
    pub key_info: Vec<String>,
    pub importance: f64,
    pub timestamp: DateTime<Utc>,
    pub compression_method: CompressionMethod,
}

/// Method used for compression
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionMethod {
    Summarize,
    Extract,
    Merge,
    Archive,
}

/// Result of a compression operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionResult {
    pub entries_compressed: usize,
    pub tokens_saved: u64,
    pub entries_extracted: usize,
    pub method: CompressionMethod,
    pub timestamp: DateTime<Utc>,
}

/// Trait for memory extraction strategies
pub trait ExtractionStrategy: Send + Sync {
    fn extract_key_info(&self, content: &str) -> Vec<String>;
    fn calculate_importance(&self, content: &str, metadata: &HashMap<String, String>) -> f64;
    fn summarize(&self, content: &str, max_length: usize) -> String;
}

/// Simple extraction strategy
pub struct SimpleExtractionStrategy;

impl ExtractionStrategy for SimpleExtractionStrategy {
    fn extract_key_info(&self, content: &str) -> Vec<String> {
        content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && trimmed.len() > 20
            })
            .take(10)
            .map(|s| s.trim().to_string())
            .collect()
    }
    
    fn calculate_importance(&self, content: &str, metadata: &HashMap<String, String>) -> f64 {
        let mut score = 0.5_f64;
        
        if content.len() > 500 {
            score += 0.1;
        }
        
        if let Some(priority) = metadata.get("priority") {
            match priority.as_str() {
                "high" => score += 0.3,
                "medium" => score += 0.1,
                "low" => score -= 0.1,
                _ => {}
            }
        }
        
        if content.contains("error") || content.contains("fail") {
            score += 0.2;
        }
        
        score.max(0.0).min(1.0)
    }
    
    fn summarize(&self, content: &str, max_length: usize) -> String {
        if content.len() <= max_length {
            content.to_string()
        } else {
            format!("{}...", &content[..max_length.saturating_sub(3)])
        }
    }
}

/// Simple memory entry for compression
#[derive(Debug, Clone)]
pub struct CompressionEntry {
    pub id: String,
    pub content: String,
    pub metadata: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
}

/// Layered Memory Compression System (simplified)
pub struct HierarchicalMemoryCompression {
    config: CompressionConfig,
    stats: std::sync::Mutex<MemoryStats>,
}

impl HierarchicalMemoryCompression {
    pub fn new(config: CompressionConfig) -> Self {
        Self {
            config,
            stats: std::sync::Mutex::new(MemoryStats::default()),
        }
    }
    
    pub fn new_default() -> Self {
        Self::new(CompressionConfig::default())
    }
    
    pub fn should_compress(&self) -> bool {
        if !self.config.auto_compress {
            return false;
        }
        
        let stats = self.stats.lock().unwrap();
        self.config.trigger.should_trigger(&stats)
    }
    
    pub fn compress(&self, entries: Vec<CompressionEntry>) -> CompressionResult {
        let mut compressed_count = 0;
        let mut tokens_saved = 0;
        let mut key_info_extracted = 0;
        
        for entry in entries.iter().take(20) {
            let strategy = SimpleExtractionStrategy;
            let key_info = strategy.extract_key_info(&entry.content);
            let importance = strategy.calculate_importance(&entry.content, &entry.metadata);
            
            if importance > 0.5 {
                let summary = strategy.summarize(&entry.content, 200);
                tokens_saved += entry.content.len() as u64 - summary.len() as u64;
                key_info_extracted += key_info.len();
                compressed_count += 1;
            }
        }
        
        // Update stats
        {
            let mut stats = self.stats.lock().unwrap();
            stats.compression_count += 1;
            stats.last_compression = Utc::now();
            stats.working_count = entries.len();
            stats.total_tokens = (entries.len() * 100) as u64;
        }
        
        info!(
            "Compressed {} entries, saved {} tokens, extracted {} key points",
            compressed_count,
            tokens_saved,
            key_info_extracted
        );
        
        CompressionResult {
            entries_compressed: compressed_count,
            tokens_saved,
            entries_extracted: key_info_extracted,
            method: CompressionMethod::Summarize,
            timestamp: Utc::now(),
        }
    }
    
    pub fn get_stats(&self) -> MemoryStats {
        self.stats.lock().unwrap().clone()
    }
    
    pub fn force_compress(&self, entries: Vec<CompressionEntry>) -> CompressionResult {
        self.compress(entries)
    }
}

/// Extracted pattern for semantic storage
#[derive(Debug, Clone)]
pub struct ExtractedPattern {
    pub content: String,
    pub key_info: Vec<String>,
    pub frequency: usize,
    pub timestamp: DateTime<Utc>,
}