//! Skill Discoverer - 技能自动发现引擎
//!
//! 从成功的执行历史中自动发现可复用的技能模式

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::skills::{SkillDefinition, SkillParameter};

/// 执行模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPattern {
    pub id: String,
    pub name: String,
    pub description: String,
    pub input_pattern: String,     // 输入匹配模式
    pub output_template: String,   // 输出模板
    pub tool_sequence: Vec<String>, // 工具调用序列
    pub frequency: u32,            // 出现频率
    pub success_rate: f64,         // 成功率
    pub avg_latency_ms: u64,       // 平均延迟
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub example_inputs: Vec<String>,
}

/// 发现的技能候选
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCandidate {
    pub pattern: ExecutionPattern,
    pub confidence: f64,
    pub generated_skill: SkillDefinition,
    pub rationale: String,
}

/// 技能发现配置
#[derive(Debug, Clone)]
pub struct SkillDiscoveryConfig {
    pub min_frequency: u32,           // 最小出现频率
    pub min_success_rate: f64,        // 最小成功率
    pub max_patterns: usize,          // 最大模式数量
    pub similarity_threshold: f64,    // 相似度阈值
    pub analysis_window_days: i64,    // 分析窗口（天）
}

impl Default for SkillDiscoveryConfig {
    fn default() -> Self {
        Self {
            min_frequency: 3,
            min_success_rate: 0.8,
            max_patterns: 100,
            similarity_threshold: 0.85,
            analysis_window_days: 7,
        }
    }
}

/// 技能发现引擎
pub struct SkillDiscoverer {
    patterns: Arc<RwLock<Vec<ExecutionPattern>>>,
    config: SkillDiscoveryConfig,
}

impl SkillDiscoverer {
    pub fn new() -> Self {
        Self {
            patterns: Arc::new(RwLock::new(Vec::new())),
            config: SkillDiscoveryConfig::default(),
        }
    }

    pub fn with_config(config: SkillDiscoveryConfig) -> Self {
        Self {
            patterns: Arc::new(RwLock::new(Vec::new())),
            config,
        }
    }

    /// 分析执行历史，发现技能模式
    pub async fn discover_patterns(&self, executions: &[ExecutionRecord]) -> Vec<SkillCandidate> {
        let mut candidates = Vec::new();
        
        // 1. 聚类相似查询
        let clusters = self.cluster_executions(executions).await;
        debug!("Found {} execution clusters", clusters.len());

        // 2. 分析每个聚类
        for (cluster_id, cluster_execs) in clusters {
            if cluster_execs.len() < self.config.min_frequency as usize {
                continue;
            }

            if let Some(pattern) = self.extract_pattern(&cluster_id, &cluster_execs).await {
                // 3. 生成技能候选
                if let Some(candidate) = self.generate_skill_candidate(pattern).await {
                    candidates.push(candidate);
                }
            }
        }

        // 4. 按置信度排序
        candidates.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        
        info!("Discovered {} skill candidates", candidates.len());
        candidates
    }

    /// 聚类执行记录
    async fn cluster_executions(
        &self,
        executions: &[ExecutionRecord],
    ) -> HashMap<String, Vec<ExecutionRecord>> {
        let mut clusters: HashMap<String, Vec<ExecutionRecord>> = HashMap::new();
        
        for exec in executions {
            // 基于意图和工具序列生成聚类键
            let cluster_key = self.generate_cluster_key(exec);
            
            clusters
                .entry(cluster_key)
                .or_default()
                .push(exec.clone());
        }

        clusters
    }

    fn generate_cluster_key(&self, exec: &ExecutionRecord) -> String {
        // 使用工具序列和查询类型作为聚类键
        let tool_sig = exec.tool_sequence.join("->");
        let intent_prefix = exec.query.split_whitespace()
            .next()
            .unwrap_or("unknown")
            .to_lowercase();
        
        format!("{}:{}", intent_prefix, tool_sig)
    }

    /// 从聚类中提取模式
    async fn extract_pattern(
        &self,
        cluster_id: &str,
        executions: &[ExecutionRecord],
    ) -> Option<ExecutionPattern> {
        if executions.is_empty() {
            return None;
        }

        let total = executions.len();
        let successful = executions.iter().filter(|e| e.success).count();
        let success_rate = successful as f64 / total as f64;

        if success_rate < self.config.min_success_rate {
            return None;
        }

        let avg_latency = executions.iter().map(|e| e.latency_ms).sum::<u64>() / total as u64;
        
        // 提取共同工具序列
        let common_tools = self.extract_common_tools(executions);
        
        // 生成输入模式（简化版）
        let input_pattern = self.generate_input_pattern(executions);
        
        // 收集示例输入
        let examples: Vec<String> = executions
            .iter()
            .take(5)
            .map(|e| e.query.clone())
            .collect();

        let timestamps: Vec<DateTime<Utc>> = executions.iter().map(|e| e.timestamp).collect();
        let first_seen = timestamps.iter().min().copied().unwrap_or(Utc::now());
        let last_seen = timestamps.iter().max().copied().unwrap_or(Utc::now());

        Some(ExecutionPattern {
            id: format!("pattern_{}", &uuid::Uuid::new_v4().to_string()[..8]),
            name: self.generate_pattern_name(cluster_id),
            description: format!("Auto-discovered pattern from {} executions", total),
            input_pattern,
            output_template: self.infer_output_template(executions),
            tool_sequence: common_tools,
            frequency: total as u32,
            success_rate,
            avg_latency_ms: avg_latency,
            first_seen,
            last_seen,
            example_inputs: examples,
        })
    }

    fn extract_common_tools(&self, executions: &[ExecutionRecord]) -> Vec<String> {
        if executions.is_empty() {
            return Vec::new();
        }

        // 找到最长的公共工具序列前缀
        let mut common = executions[0].tool_sequence.clone();
        
        for exec in executions.iter().skip(1) {
            common = common
                .iter()
                .zip(exec.tool_sequence.iter())
                .take_while(|(a, b)| a == b)
                .map(|(a, _)| a.clone())
                .collect();
        }

        common
    }

    fn generate_input_pattern(&self, executions: &[ExecutionRecord]) -> String {
        // 提取查询的共同前缀/模式
        if executions.is_empty() {
            return ".*".to_string();
        }

        // 简化的模式生成：提取共同关键词
        let words: Vec<HashSet<String>> = executions
            .iter()
            .map(|e| {
                e.query
                    .to_lowercase()
                    .split_whitespace()
                    .map(|s| s.to_string())
                    .collect()
            })
            .collect();

        let mut common_words: HashSet<String> = words[0].clone();
        for word_set in words.iter().skip(1) {
            common_words = common_words
                .intersection(word_set)
                .cloned()
                .collect();
        }

        if common_words.is_empty() {
            ".*".to_string()
        } else {
            format!("(?i).*({}).*", common_words.into_iter().collect::<Vec<_>>().join("|"))
        }
    }

    fn infer_output_template(&self, executions: &[ExecutionRecord]) -> String {
        // 分析输出格式，提取模板
        // 简化实现：返回最常见的输出前缀
        let mut prefix_counts: HashMap<String, usize> = HashMap::new();
        
        for exec in executions {
            if let Some(ref output) = exec.output {
                let prefix: String = output.chars().take(50).collect();
                *prefix_counts.entry(prefix).or_insert(0) += 1;
            }
        }

        prefix_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(prefix, _)| prefix)
            .unwrap_or_else(|| "{{result}}".to_string())
    }

    fn generate_pattern_name(&self, cluster_id: &str) -> String {
        // 从聚类ID生成可读名称
        let parts: Vec<&str> = cluster_id.split(':').collect();
        if parts.len() >= 2 {
            format!("auto_{}_{}", parts[0], &uuid::Uuid::new_v4().to_string()[..6])
        } else {
            format!("auto_pattern_{}", &uuid::Uuid::new_v4().to_string()[..8])
        }
    }

    /// 生成技能候选
    async fn generate_skill_candidate(&self, pattern: ExecutionPattern) -> Option<SkillCandidate> {
        // 计算置信度
        let confidence = self.calculate_confidence(&pattern);
        
        if confidence < 0.5 {
            return None;
        }

        // 生成技能定义
        let skill_def = self.create_skill_definition(&pattern);

        let rationale = format!(
            "Discovered from {} successful executions with {:.1}% success rate. \
             Average latency: {}ms. Tool sequence: {:?}",
            pattern.frequency,
            pattern.success_rate * 100.0,
            pattern.avg_latency_ms,
            pattern.tool_sequence
        );

        Some(SkillCandidate {
            pattern,
            confidence,
            generated_skill: skill_def,
            rationale,
        })
    }

    fn calculate_confidence(&self, pattern: &ExecutionPattern) -> f64 {
        let freq_score = (pattern.frequency as f64 / 10.0).min(1.0); // 频率分数
        let success_score = pattern.success_rate; // 成功率分数
        let stability_score = if pattern.frequency >= 5 { 1.0 } else { 0.5 }; // 稳定性分数

        // 加权平均
        freq_score * 0.3 + success_score * 0.5 + stability_score * 0.2
    }

    fn create_skill_definition(&self, pattern: &ExecutionPattern) -> SkillDefinition {
        // 从模式生成技能定义
        let params = self.extract_parameters(&pattern.example_inputs);

        SkillDefinition {
            name: pattern.name.clone(),
            description: pattern.description.clone(),
            parameters: params,
            handler: format!("auto_generated:{}", pattern.id),
            examples: pattern.example_inputs.clone(),
            metadata: serde_json::json!({
                "auto_discovered": true,
                "pattern_id": pattern.id,
                "tool_sequence": pattern.tool_sequence,
                "success_rate": pattern.success_rate,
                "frequency": pattern.frequency,
            }),
        }
    }

    fn extract_parameters(&self, examples: &[String]) -> Vec<SkillParameter> {
        // 从示例中提取参数
        // 简化实现：检测常见的可变部分
        let mut params = Vec::new();

        // 检测数字参数
        let has_numbers = examples.iter().any(|e| e.chars().any(|c| c.is_ascii_digit()));
        if has_numbers {
            params.push(SkillParameter {
                name: "value".to_string(),
                description: "Numeric value".to_string(),
                param_type: "number".to_string(),
                required: false,
                default: None,
            });
        }

        // 检测查询参数
        params.push(SkillParameter {
            name: "query".to_string(),
            description: "Search query or input text".to_string(),
            param_type: "string".to_string(),
            required: true,
            default: None,
        });

        params
    }

    /// 添加已知模式（用于测试或预定义）
    pub async fn add_pattern(&self, pattern: ExecutionPattern) {
        let mut patterns = self.patterns.write().await;
        patterns.push(pattern);
        
        if patterns.len() > self.config.max_patterns {
            patterns.remove(0);
        }
    }

    /// 获取所有模式
    pub async fn get_patterns(&self) -> Vec<ExecutionPattern> {
        self.patterns.read().await.clone()
    }

    /// 查找匹配输入的模式
    pub async fn find_matching_patterns(&self, input: &str) -> Vec<ExecutionPattern> {
        let patterns = self.patterns.read().await;
        
        patterns
            .iter()
            .filter(|p| self.matches_pattern(input, &p.input_pattern))
            .cloned()
            .collect()
    }

    fn matches_pattern(&self, input: &str, pattern: &str) -> bool {
        // 简化匹配：检查关键词包含
        if pattern == ".*" {
            return true;
        }
        
        let input_lower = input.to_lowercase();
        let pattern_lower = pattern.to_lowercase();
        
        // 提取关键词（简单处理）
        let keywords: Vec<&str> = pattern_lower
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 2)
            .collect();
        
        keywords.iter().any(|kw| input_lower.contains(kw))
    }
}

impl Default for SkillDiscoverer {
    fn default() -> Self {
        Self::new()
    }
}

/// 执行记录（用于技能发现）
#[derive(Debug, Clone)]
pub struct ExecutionRecord {
    pub query: String,
    pub output: Option<String>,
    pub tool_sequence: Vec<String>,
    pub success: bool,
    pub latency_ms: u64,
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pattern_discovery() {
        let discoverer = SkillDiscoverer::new();

        // 创建测试执行记录
        let executions = vec![
            ExecutionRecord {
                query: "search for rust tutorials".to_string(),
                output: Some("Found 10 rust tutorials...".to_string()),
                tool_sequence: vec!["web_search".to_string(), "summarize".to_string()],
                success: true,
                latency_ms: 1500,
                timestamp: Utc::now(),
            },
            ExecutionRecord {
                query: "search for python guides".to_string(),
                output: Some("Found 8 python guides...".to_string()),
                tool_sequence: vec!["web_search".to_string(), "summarize".to_string()],
                success: true,
                latency_ms: 1400,
                timestamp: Utc::now(),
            },
            ExecutionRecord {
                query: "search for go documentation".to_string(),
                output: Some("Found go docs...".to_string()),
                tool_sequence: vec!["web_search".to_string(), "summarize".to_string()],
                success: true,
                latency_ms: 1600,
                timestamp: Utc::now(),
            },
        ];

        let candidates = discoverer.discover_patterns(&executions).await;
        
        assert!(!candidates.is_empty());
        let candidate = &candidates[0];
        assert!(candidate.confidence > 0.5);
        assert_eq!(candidate.pattern.tool_sequence, vec!["web_search", "summarize"]);
    }

    #[test]
    fn test_common_tools_extraction() {
        let discoverer = SkillDiscoverer::new();
        
        let executions = vec![
            ExecutionRecord {
                query: "test".to_string(),
                output: None,
                tool_sequence: vec!["a".to_string(), "b".to_string(), "c".to_string()],
                success: true,
                latency_ms: 100,
                timestamp: Utc::now(),
            },
            ExecutionRecord {
                query: "test2".to_string(),
                output: None,
                tool_sequence: vec!["a".to_string(), "b".to_string(), "d".to_string()],
                success: true,
                latency_ms: 100,
                timestamp: Utc::now(),
            },
        ];

        let common = discoverer.extract_common_tools(&executions);
        assert_eq!(common, vec!["a", "b"]);
    }
}
