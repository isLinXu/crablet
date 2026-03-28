//! Intelligent Context Window Management System
//!
//! 智能上下文窗口管理:
//! - 动态 token 预算分配
//! - 重要性感知压缩
//! - 分层摘要

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// 消息重要性评分
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportanceLevel {
    Critical,   // 必须保留
    High,       // 重要
    Medium,     // 一般
    Low,        // 可压缩
    Ignore,     // 可丢弃
}

/// 消息结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub token_count: usize,
    pub importance: ImportanceLevel,
    pub keywords: Vec<String>,
    pub embedding: Option<Vec<f32>>,
}

/// 压缩配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub max_tokens: usize,          // 最大 token 数
    pub preserve_recent: usize,     // 保留最近 N 条消息
    pub min_importance: ImportanceLevel,  // 最低保留重要性
    pub summary_ratio: f32,         // 摘要压缩比
    pub enable_semantic_dedup: bool,  // 启用语义去重
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            max_tokens: 128000,
            preserve_recent: 10,
            min_importance: ImportanceLevel::Low,
            summary_ratio: 0.3,
            enable_semantic_dedup: true,
        }
    }
}

/// 智能上下文管理器
pub struct SmartContextManager {
    config: CompressionConfig,
    messages: VecDeque<ContextMessage>,
    keyword_index: HashMap<String, Vec<usize>>,  // 关键词 -> 消息索引
    total_tokens: usize,
    compression_enabled: bool,
}

impl SmartContextManager {
    pub fn new(config: CompressionConfig) -> Self {
        Self {
            config,
            messages: VecDeque::new(),
            keyword_index: HashMap::new(),
            total_tokens: 0,
            compression_enabled: true,
        }
    }
    
    /// 添加消息
    pub fn add_message(&mut self, mut message: ContextMessage) {
        // 自动评估重要性
        if message.importance == ImportanceLevel::Ignore {
            message.importance = self.auto_assess_importance(&message.content);
        }
        
        // 提取关键词
        message.keywords = self.extract_keywords(&message.content);
        
        // 估算 token 数 (简单估算: 1 token ≈ 4 字符)
        message.token_count = message.content.len() / 4;
        
        // 更新索引
        for keyword in &message.keywords {
            self.keyword_index
                .entry(keyword.clone())
                .or_insert_with(Vec::new)
                .push(self.messages.len());
        }
        
        // 添加消息
        let token_count = message.token_count;
        self.messages.push_back(message);
        self.total_tokens += token_count;
        
        // 如果超过预算，触发压缩
        if self.total_tokens > self.config.max_tokens {
            self.compress();
        }
    }
    
    /// 自动评估消息重要性
    fn auto_assess_importance(&self, content: &str) -> ImportanceLevel {
        let content_lower = content.to_lowercase();
        
        // 高重要性关键词
        let high_importance = ["error", "failed", "critical", "important", "必须", "关键", "错误"];
        for kw in high_importance {
            if content_lower.contains(kw) {
                return ImportanceLevel::High;
            }
        }
        
        // 中等重要性
        let medium_importance = ["result", "success", "completed", "结果", "完成"];
        for kw in medium_importance {
            if content_lower.contains(kw) {
                return ImportanceLevel::Medium;
            }
        }
        
        // 低重要性默认
        ImportanceLevel::Low
    }
    
    /// 提取关键词
    fn extract_keywords(&self, content: &str) -> Vec<String> {
        // 简单实现：提取连续的中英文词
        let mut keywords = Vec::new();
        
        // 分词 (简化版)
        let words: Vec<&str> = content.split(|c: char| !c.is_alphanumeric())
            .filter(|s| s.len() > 2)
            .collect();
        
        // 选取频率最高的 5 个词
        let mut word_freq: HashMap<&str, usize> = HashMap::new();
        for word in words {
            *word_freq.entry(word).or_insert(0) += 1;
        }
        
        let mut sorted: Vec<_> = word_freq.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        
        for (word, _) in sorted.iter().take(5) {
            keywords.push(word.to_lowercase());
        }
        
        keywords
    }
    
    /// 压缩上下文
    fn compress(&mut self) {
        tracing::info!("Compressing context from {} tokens", self.total_tokens);
        
        // 策略 1: 保留最近的消息
        let mut keep_indices: Vec<usize> = (self.messages.len().saturating_sub(self.config.preserve_recent)..self.messages.len()).collect();
        
        // 策略 2: 保留高重要性消息
        for (i, msg) in self.messages.iter().enumerate() {
            if msg.importance >= self.config.min_importance && !keep_indices.contains(&i) {
                keep_indices.push(i);
            }
        }
        
        // 策略 3: 语义去重 (如果启用)
        if self.config.enable_semantic_dedup {
            keep_indices = self.semantic_dedup(keep_indices);
        }
        
        // 策略 4: 对低重要性消息进行摘要
        keep_indices.sort();
        
        let mut new_messages = VecDeque::new();
        let mut new_tokens = 0;
        
        for i in keep_indices {
            if let Some(msg) = self.messages.get(i) {
                // 如果是低重要性消息，尝试压缩
                let processed = if msg.importance < ImportanceLevel::Medium && msg.content.len() > 200 {
                    self.summarize_message(msg)
                } else {
                    msg.clone()
                };
                
                new_messages.push_back(processed);
                new_tokens += processed.token_count;
            }
        }
        
        // 更新状态
        self.messages = new_messages;
        self.total_tokens = new_tokens;
        
        // 重建索引
        self.rebuild_index();
        
        tracing::info!("Compressed to {} tokens", self.total_tokens);
    }
    
    /// 语义去重
    fn semantic_dedup(&self, indices: Vec<usize>) -> Vec<usize> {
        // 简化实现：基于内容相似度去重
        let mut unique = Vec::new();
        
        for i in indices {
            if let Some(msg) = self.messages.get(i) {
                let mut is_duplicate = false;
                
                for kept in &unique {
                    if let Some(kept_msg) = self.messages.get(*kept) {
                        // 简单的相似度检查
                        let similarity = self.compute_similarity(&msg.content, &kept_msg.content);
                        if similarity > 0.8 {
                            // 如果已有更高重要性的版本，丢弃这个
                            if kept_msg.importance >= msg.importance {
                                is_duplicate = true;
                                break;
                            }
                        }
                    }
                }
                
                if !is_duplicate {
                    unique.push(i);
                }
            }
        }
        
        unique
    }
    
    /// 计算内容相似度
    fn compute_similarity(&self, a: &str, b: &str) -> f32 {
        let words_a: std::collections::HashSet<_> = a.split_whitespace().collect();
        let words_b: std::collections::HashSet<_> = b.split_whitespace().collect();
        
        let intersection = words_a.intersection(&words_b).count();
        let union = words_a.union(&words_b).count();
        
        if union == 0 { 0.0 } else { intersection as f32 / union as f32 }
    }
    
    /// 摘要消息 (简化版)
    fn summarize_message(&self, msg: &ContextMessage) -> ContextMessage {
        // 简单截取 + 标记
        let summary = if msg.content.len() > 200 {
            format!("[摘要] {}", &msg.content[..200])
        } else {
            msg.content.clone()
        };
        
        ContextMessage {
            id: msg.id.clone(),
            role: msg.role.clone(),
            content: summary,
            timestamp: msg.timestamp,
            token_count: summary.len() / 4,
            importance: msg.importance,
            keywords: msg.keywords.clone(),
            embedding: None,
        }
    }
    
    /// 重建关键词索引
    fn rebuild_index(&mut self) {
        self.keyword_index.clear();
        
        for (i, msg) in self.messages.iter().enumerate() {
            for keyword in &msg.keywords {
                self.keyword_index
                    .entry(keyword.clone())
                    .or_insert_with(Vec::new)
                    .push(i);
            }
        }
    }
    
    /// 关键词搜索
    pub fn search_by_keyword(&self, keyword: &str) -> Vec<ContextMessage> {
        self.keyword_index
            .get(&keyword.to_lowercase())
            .map(|indices| {
                indices.iter()
                    .filter_map(|&i| self.messages.get(i).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }
    
    /// 获取当前所有消息
    pub fn get_messages(&self) -> Vec<ContextMessage> {
        self.messages.iter().cloned().collect()
    }
    
    /// 获取当前 token 数
    pub fn current_tokens(&self) -> usize {
        self.total_tokens
    }
    
    /// 获取预算使用率
    pub fn budget_usage(&self) -> f32 {
        self.total_tokens as f32 / self.config.max_tokens as f32
    }
}

/// 分层摘要器
pub struct HierarchicalSummarizer {
    levels: Vec<SummarizationLevel>,
}

#[derive(Debug, Clone)]
pub struct SummarizationLevel {
    pub name: String,
    pub ratio: f32,        // 压缩比
    pub min_length: usize, // 最小原始长度
}

impl HierarchicalSummarizer {
    pub fn new() -> Self {
        Self {
            levels: vec![
                SummarizationLevel { name: "brief".to_string(), ratio: 0.1, min_length: 1000 },
                SummarizationLevel { name: "concise".to_string(), ratio: 0.3, min_length: 500 },
                SummarizationLevel { name: "standard".to_string(), ratio: 0.5, min_length: 200 },
            ],
        }
    }
    
    /// 根据目标 token 数选择合适的摘要级别
    pub fn select_level(&self, target_tokens: usize, current_tokens: usize) -> Option<&SummarizationLevel> {
        if current_tokens <= target_tokens {
            return None;  // 不需要压缩
        }
        
        let ratio = target_tokens as f32 / current_tokens as f32;
        
        for level in &self.levels {
            if level.ratio >= ratio && current_tokens >= level.min_length {
                return Some(level);
            }
        }
        
        Some(&self.levels.last().unwrap())  // 最低级别
    }
}

impl Default for HierarchicalSummarizer {
    fn default() -> Self {
        Self::new()
    }
}