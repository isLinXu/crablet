//! Learner - 从经验中学习并提取模式

use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::Result;
use crate::cognitive::meta_controller::monitor::ExecutionMetrics;
use crate::cognitive::meta_controller::reflector::ProblemDiagnosis;
// use crate::cognitive::meta_controller::reflector::ProblemType; // Removed to avoid unused import

/// 学习器
pub struct Learner {
    patterns: Arc<tokio::sync::RwLock<Vec<Pattern>>>,
    knowledge_base: Arc<tokio::sync::RwLock<KnowledgeBase>>,
    max_patterns: usize,
}

/// 模式类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PatternType {
    /// 任务模式
    TaskPattern,
    /// 策略模式
    StrategyPattern,
    /// 错误模式
    ErrorPattern,
}

/// 模式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pattern {
    /// 模式 ID
    pub id: String,
    /// 模式类型
    pub pattern_type: PatternType,
    /// 模式名称
    pub name: String,
    /// 模式描述
    pub description: String,
    /// 触发条件
    pub trigger_conditions: Vec<String>,
    /// 成功率
    pub success_rate: f32,
    /// 使用次数
    pub usage_count: u64,
    /// 创建时间
    pub created_at: String,
    /// 最后更新时间
    pub updated_at: String,
}

/// 学习到的知识
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedKnowledge {
    /// 知识 ID
    pub knowledge_id: String,
    /// 知识类型
    pub knowledge_type: String,
    /// 知识内容
    pub content: String,
    /// 相关模式
    pub related_patterns: Vec<String>,
    /// 置信度
    pub confidence: f32,
}

/// 知识库
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KnowledgeBase {
    /// 存储的知识条目
    knowledge: HashMap<String, LearnedKnowledge>,
    /// 知识分类
    categories: HashMap<String, Vec<String>>,
    /// 知识关系图
    graph: HashMap<String, Vec<String>>,
}

impl Default for KnowledgeBase {
    fn default() -> Self {
        Self {
            knowledge: HashMap::new(),
            categories: HashMap::new(),
            graph: HashMap::new(),
        }
    }
}

impl Learner {
    /// 创建新的学习器
    pub fn new(max_patterns: usize) -> Self {
        Self {
            patterns: Arc::new(tokio::sync::RwLock::new(Vec::new())),
            knowledge_base: Arc::new(tokio::sync::RwLock::new(KnowledgeBase::default())),
            max_patterns,
        }
    }

    /// 从经验中学习
    pub async fn learn_from_experience(
        &self,
        task: &str,
        metrics: &ExecutionMetrics,
        diagnosis: &ProblemDiagnosis,
    ) -> Result<Vec<LearnedKnowledge>> {
        debug!("Learning from experience for task: {}", task);

        let mut learned = Vec::new();

        // 提取任务模式
        if let Some(pattern) = self.extract_task_pattern(task, metrics, diagnosis).await? {
            learned.push(pattern);
        }

        // 提取错误模式
        if !metrics.success {
            if let Some(knowledge) = self.extract_error_pattern(task, diagnosis).await? {
                learned.push(knowledge);
            }
        }

        // 提取策略模式
        if let Some(knowledge) = self.extract_strategy_pattern(task, metrics, diagnosis).await? {
            learned.push(knowledge);
        }

        info!("Extracted {} new knowledge items", learned.len());
        Ok(learned)
    }

    /// 提取任务模式
    async fn extract_task_pattern(
        &self,
        task: &str,
        metrics: &ExecutionMetrics,
        _diagnosis: &ProblemDiagnosis,
    ) -> Result<Option<LearnedKnowledge>> {
        // 分析任务特征
        let features = self.analyze_task_features(task);

        // 检查是否已经有类似的模式
        let similar_pattern = self.find_similar_pattern(&features).await;

        if let Some(existing) = similar_pattern {
            // 更新现有模式
            self.update_pattern(&existing.id, metrics.success).await;
            return Ok(None);
        }

        // 创建新模式
        let pattern = Pattern {
            id: uuid::Uuid::new_v4().to_string(),
            pattern_type: PatternType::TaskPattern,
            name: format!("Task Pattern: {}", features.category),
            description: format!(
                "Pattern for tasks in category '{}' with complexity {:.2}",
                features.category, features.complexity
            ),
            trigger_conditions: vec![
                format!("category = '{}'", features.category),
                format!("complexity > {:.2}", features.complexity * 0.8),
            ],
            success_rate: if metrics.success { 1.0 } else { 0.0 },
            usage_count: 1,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        };

        // 添加模式
        let mut patterns = self.patterns.write().await;
        if patterns.len() >= self.max_patterns {
            // 移除最少使用的模式
            patterns.sort_by(|a, b| a.usage_count.cmp(&b.usage_count));
            patterns.remove(0);
        }
        patterns.push(pattern.clone());

        // 创建知识条目
        let knowledge = LearnedKnowledge {
            knowledge_id: uuid::Uuid::new_v4().to_string(),
            knowledge_type: "task_pattern".to_string(),
            content: format!(
                "Task category: {}, complexity: {:.2}, keywords: {:?}",
                features.category, features.complexity, features.keywords
            ),
            related_patterns: vec![pattern.id.clone()],
            confidence: metrics.confidence,
        };

        // 添加到知识库
        self.add_knowledge(knowledge.clone()).await;

        Ok(Some(knowledge))
    }

    /// 提取错误模式
    async fn extract_error_pattern(
        &self,
        _task: &str,
        diagnosis: &ProblemDiagnosis,
    ) -> Result<Option<LearnedKnowledge>> {
        let knowledge = LearnedKnowledge {
            knowledge_id: uuid::Uuid::new_v4().to_string(),
            knowledge_type: "error_pattern".to_string(),
            content: format!(
                "Error pattern: {:?}, severity: {:.2}, root_cause: {:?}",
                diagnosis.problem_type, diagnosis.severity, diagnosis.root_cause
            ),
            related_patterns: vec![],
            confidence: 1.0 - diagnosis.severity,
        };

        self.add_knowledge(knowledge.clone()).await;
        Ok(Some(knowledge))
    }

    /// 提取策略模式
    async fn extract_strategy_pattern(
        &self,
        task: &str,
        metrics: &ExecutionMetrics,
        _diagnosis: &ProblemDiagnosis,
    ) -> Result<Option<LearnedKnowledge>> {
        // 分析成功的策略
        if metrics.success && metrics.confidence > 0.8 {
            let knowledge = LearnedKnowledge {
                knowledge_id: uuid::Uuid::new_v4().to_string(),
                knowledge_type: "successful_strategy".to_string(),
                content: format!(
                    "Successful strategy for task: {}. Confidence: {:.2}, Duration: {}ms",
                    task, metrics.confidence, metrics.resources.duration_ms
                ),
                related_patterns: vec![],
                confidence: metrics.confidence,
            };

            self.add_knowledge(knowledge.clone()).await;
            Ok(Some(knowledge))
        } else {
            Ok(None)
        }
    }

    /// 分析任务特征
    fn analyze_task_features(&self, task: &str) -> TaskFeatures {
        let lower = task.to_lowercase();
        let words: Vec<&str> = task.split_whitespace().collect();
        
        // 分类任务
        let category = if lower.contains("code") || lower.contains("function") || lower.contains("class") {
            "coding"
        } else if lower.contains("analyze") || lower.contains("research") {
            "analysis"
        } else if lower.contains("explain") || lower.contains("describe") {
            "explanation"
        } else if lower.contains("fix") || lower.contains("debug") {
            "debugging"
        } else {
            "general"
        };

        // 计算复杂度
        let complexity = (words.len() as f32 / 50.0).min(1.0);

        // 提取关键词
        let keywords: Vec<String> = words
            .iter()
            .filter(|w| w.len() > 4)
            .take(5)
            .map(|s| s.to_string())
            .collect();

        TaskFeatures {
            category: category.to_string(),
            complexity,
            keywords,
        }
    }

    /// 查找类似模式
    async fn find_similar_pattern(&self, features: &TaskFeatures) -> Option<Pattern> {
        let patterns = self.patterns.read().await;
        patterns.iter().find(|p| {
            p.pattern_type == PatternType::TaskPattern
                && p.trigger_conditions.iter().any(|c| c.contains(&features.category))
        }).cloned()
    }

    /// 更新模式
    async fn update_pattern(&self, pattern_id: &str, success: bool) {
        let mut patterns = self.patterns.write().await;
        if let Some(pattern) = patterns.iter_mut().find(|p| p.id == pattern_id) {
            pattern.usage_count += 1;
            pattern.updated_at = chrono::Utc::now().to_rfc3339();
            
            // 更新成功率
            let total = pattern.usage_count as f32;
            pattern.success_rate = (pattern.success_rate * (total - 1.0) + if success { 1.0 } else { 0.0 }) / total;
        }
    }

    /// 添加知识
    async fn add_knowledge(&self, knowledge: LearnedKnowledge) {
        let mut kb = self.knowledge_base.write().await;
        
        kb.knowledge.insert(knowledge.knowledge_id.clone(), knowledge.clone());
        
        // 添加到分类
        kb.categories
            .entry(knowledge.knowledge_type.clone())
            .or_insert_with(Vec::new)
            .push(knowledge.knowledge_id.clone());
        
        // 建立关系
        for related in &knowledge.related_patterns {
            kb.graph
                .entry(knowledge.knowledge_id.clone())
                .or_insert_with(Vec::new)
                .push(related.clone());
        }
    }

    /// 获取所有模式
    pub async fn get_all_patterns(&self) -> Vec<Pattern> {
        self.patterns.read().await.clone()
    }

    /// 查找相关模式
    pub async fn find_relevant_patterns(&self, task: &str) -> Vec<Pattern> {
        let features = self.analyze_task_features(task);
        let patterns = self.patterns.read().await;
        
        patterns
            .iter()
            .filter(|p| {
                p.trigger_conditions.iter().any(|c| {
                    c.contains(&features.category) || c.contains(&format!("complexity"))
                })
            })
            .cloned()
            .collect()
    }

    /// 导出知识
    pub async fn export_knowledge(&self) -> Vec<LearnedKnowledge> {
        let kb = self.knowledge_base.read().await;
        kb.knowledge.values().cloned().collect()
    }

    /// 统计信息
    pub async fn get_statistics(&self) -> LearnerStats {
        let patterns = self.patterns.read().await;
        let kb = self.knowledge_base.read().await;

        LearnerStats {
            total_patterns: patterns.len(),
            total_knowledge: kb.knowledge.len(),
            avg_success_rate: if patterns.is_empty() {
                0.0
            } else {
                let sum: f32 = patterns.iter().map(|p| p.success_rate).sum();
                sum / patterns.len() as f32
            },
            categories: kb.categories.keys().cloned().collect(),
        }
    }
}

/// 任务特征
#[derive(Debug, Clone)]
struct TaskFeatures {
    category: String,
    complexity: f32,
    keywords: Vec<String>,
}

/// 学习器统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnerStats {
    pub total_patterns: usize,
    pub total_knowledge: usize,
    pub avg_success_rate: f32,
    pub categories: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_learner_creation() {
        let learner = Learner::new(100);
        assert_eq!(learner.max_patterns, 100);
    }

    #[tokio::test]
    async fn test_extract_task_pattern() {
        let learner = Learner::new(100);
        
        let metrics = ExecutionMetrics {
            success: true,
            confidence: 0.9,
            ..Default::default()
        };
        
        let diagnosis = ProblemDiagnosis {
            problem_type: crate::cognitive::meta_controller::reflector::ProblemType::Other("test".into()),
            description: "test".into(),
            severity: 0.5,
            root_cause: None,
            suggested_actions: vec![],
        };
        
        let knowledge = learner.extract_task_pattern("write a function", &metrics, &diagnosis).await.expect("Failed to extract task pattern");
        assert!(knowledge.is_some());
    }

    #[tokio::test]
    async fn test_task_features() {
        let learner = Learner::new(100);
        let features = learner.analyze_task_features("write a function in rust");
        assert_eq!(features.category, "coding");
        assert!(!features.keywords.is_empty());
    }
}
