use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use regex::Regex;

/// 查询意图类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum QueryIntent {
    Factual,      // 事实查询
    Summary,      // 摘要请求
    Comparison,   // 比较分析
    Reasoning,    // 推理分析
    HowTo,        // 操作指南
    Definition,   // 定义解释
    Temporal,     // 时间相关
    Unknown,
}

impl QueryIntent {
    pub fn as_str(&self) -> &'static str {
        match self {
            QueryIntent::Factual => "factual",
            QueryIntent::Summary => "summary",
            QueryIntent::Comparison => "comparison",
            QueryIntent::Reasoning => "reasoning",
            QueryIntent::HowTo => "howto",
            QueryIntent::Definition => "definition",
            QueryIntent::Temporal => "temporal",
            QueryIntent::Unknown => "unknown",
        }
    }
}

/// 提取的实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEntity {
    pub name: String,
    pub entity_type: EntityType,
    pub position: (usize, usize), // start, end
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EntityType {
    Person,
    Organization,
    Location,
    Concept,
    Technology,
    Product,
    Date,
    Unknown,
}

/// 查询分析结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryAnalysis {
    pub original: String,
    pub intent: QueryIntent,
    pub entities: Vec<ExtractedEntity>,
    pub rewritten: Vec<String>, // 重写后的多个查询变体
    pub sub_queries: Vec<String>, // 子查询分解
    pub keywords: Vec<String>, // 提取的关键词
    pub temporal_hints: Option<TemporalHints>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalHints {
    pub has_time_constraint: bool,
    pub time_range: Option<(String, String)>, // (start, end)
    pub recency_preference: f32, // 0.0-1.0, higher means prefer recent
}

/// 查询分析器
pub struct QueryAnalyzer {
    stopwords: HashSet<String>,
}

impl QueryAnalyzer {
    pub fn new() -> Self {
        let mut stopwords = HashSet::new();
        for word in [
            "the", "and", "with", "from", "that", "this", "for", "into", "about",
            "using", "have", "been", "will", "your", "then", "when", "where",
            "what", "which", "while", "also", "there", "their", "them", "的", "了",
            "在", "是", "我", "有", "和", "就", "不", "人", "都", "一", "一个",
            "上", "也", "很", "到", "说", "要", "去", "你", "会", "着", "没有",
            "看", "好", "自己", "这", "那", "这些", "那些", "什么", "怎么",
        ] {
            stopwords.insert(word.to_string());
        }
        
        Self { stopwords }
    }

    /// 分析查询
    pub async fn analyze(&self, query: &str) -> Result<QueryAnalysis> {
        let intent = self.classify_intent(query);
        let entities = self.extract_entities(query);
        let keywords = self.extract_keywords(query);
        let rewritten = self.rewrite_query(query, &entities);
        let sub_queries = self.decompose_query(query, intent);
        let temporal_hints = self.extract_temporal_hints(query);

        Ok(QueryAnalysis {
            original: query.to_string(),
            intent,
            entities,
            rewritten,
            sub_queries,
            keywords,
            temporal_hints,
        })
    }

    /// 意图分类（基于规则）
    fn classify_intent(&self, query: &str) -> QueryIntent {
        let lower = query.to_lowercase();
        
        // 摘要关键词
        if lower.contains("总结") || lower.contains("摘要") || lower.contains("summarize")
            || lower.contains("概括") || lower.contains("overview") {
            return QueryIntent::Summary;
        }
        
        // 比较关键词
        if lower.contains("比较") || lower.contains("对比") || lower.contains("difference")
            || lower.contains("vs") || lower.contains("versus") || lower.contains("区别") {
            return QueryIntent::Comparison;
        }
        
        // 操作指南
        if lower.contains("如何") || lower.contains("怎么") || lower.contains("how to")
            || lower.contains("步骤") || lower.contains("guide") {
            return QueryIntent::HowTo;
        }
        
        // 定义解释
        if lower.contains("什么是") || lower.contains("定义") || lower.contains("explain")
            || lower.contains("what is") || lower.contains("meaning") {
            return QueryIntent::Definition;
        }
        
        // 时间相关
        if lower.contains("什么时候") || lower.contains("时间") || lower.contains("when")
            || lower.contains("recent") || lower.contains("latest") || lower.contains("202") {
            return QueryIntent::Temporal;
        }
        
        // 推理分析
        if lower.contains("为什么") || lower.contains("原因") || lower.contains("分析")
            || lower.contains("why") || lower.contains("reason") || lower.contains("analyze") {
            return QueryIntent::Reasoning;
        }
        
        // 默认事实查询
        QueryIntent::Factual
    }

    /// 实体提取（简化版，可替换为NER模型）
    fn extract_entities(&self, query: &str) -> Vec<ExtractedEntity> {
        let mut entities = Vec::new();
        
        // 技术术语识别（大写字母开头的连续单词）
        let tech_patterns = [
            "Rust", "Python", "Java", "JavaScript", "TypeScript", "Go", "C++",
            "React", "Vue", "Angular", "Node.js", "Docker", "Kubernetes",
            "AI", "LLM", "GPT", "Claude", "OpenAI", "Anthropic",
            "PDF", "API", "HTTP", "WebSocket", "GraphQL", "REST",
        ];
        
        for pattern in &tech_patterns {
            if let Some(pos) = query.find(pattern) {
                entities.push(ExtractedEntity {
                    name: pattern.to_string(),
                    entity_type: EntityType::Technology,
                    position: (pos, pos + pattern.len()),
                });
            }
        }
        
        // 日期识别（简单模式）
        let date_patterns = [
            r"\d{4}年", r"\d{4}-\d{2}", r"20\d{2}", r"January", r"February",
            r"March", r"April", r"May", r"June", r"July", r"August",
            r"September", r"October", r"November", r"December",
        ];
        
        for pattern in &date_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                for mat in regex.find_iter(query) {
                    entities.push(ExtractedEntity {
                        name: mat.as_str().to_string(),
                        entity_type: EntityType::Date,
                        position: (mat.start(), mat.end()),
                    });
                }
            }
        }
        
        // 去重
        entities.sort_by(|a, b| a.position.0.cmp(&b.position.0));
        entities.dedup_by(|a, b| a.name == b.name);
        
        entities
    }

    /// 关键词提取
    fn extract_keywords(&self, query: &str) -> Vec<String> {
        let words: Vec<String> = query
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .filter(|w| {
                let w = w.to_lowercase();
                w.len() >= 3 && !self.stopwords.contains(&w)
            })
            .map(|w| w.to_lowercase())
            .collect();
        
        // 去重
        let mut unique: Vec<String> = words.into_iter().collect::<HashSet<_>>().into_iter().collect();
        unique.sort();
        unique
    }

    /// 查询重写
    fn rewrite_query(&self, query: &str, entities: &[ExtractedEntity]) -> Vec<String> {
        let mut rewritten = vec![query.to_string()];
        
        // 添加同义词扩展
        let expanded = self.expand_synonyms(query);
        if expanded != query {
            rewritten.push(expanded);
        }
        
        // 添加实体强调版本
        if !entities.is_empty() {
            let entity_names: Vec<String> = entities.iter().map(|e| e.name.clone()).collect();
            let emphasized = format!("{} (关于: {})", query, entity_names.join(", "));
            rewritten.push(emphasized);
        }
        
        rewritten
    }

    /// 同义词扩展（简化版）
    fn expand_synonyms(&self, query: &str) -> String {
        let synonyms = [
            ("pdf", "PDF文档 便携式文档格式"),
            ("api", "API接口 应用程序接口"),
            ("rag", "检索增强生成 Retrieval-Augmented Generation"),
            ("llm", "大语言模型 Large Language Model"),
            ("ai", "人工智能 Artificial Intelligence"),
        ];
        
        let mut expanded = query.to_string();
        for (term, expansion) in &synonyms {
            if query.to_lowercase().contains(term) {
                expanded = format!("{} {}", expanded, expansion);
            }
        }
        
        expanded
    }

    /// 查询分解
    fn decompose_query(&self, query: &str, intent: QueryIntent) -> Vec<String> {
        let mut sub_queries = Vec::new();
        
        match intent {
            QueryIntent::Comparison => {
                // 比较查询分解为多个方面
                sub_queries.push(format!("{} 的优点", query));
                sub_queries.push(format!("{} 的缺点", query));
                sub_queries.push(format!("{} 的区别", query));
            }
            QueryIntent::Summary => {
                // 摘要查询分解
                sub_queries.push(format!("{} 主要内容", query));
                sub_queries.push(format!("{} 关键要点", query));
            }
            QueryIntent::HowTo => {
                // 操作指南分解
                sub_queries.push(format!("{} 步骤", query));
                sub_queries.push(format!("{} 方法", query));
            }
            _ => {
                // 默认不分解
                sub_queries.push(query.to_string());
            }
        }
        
        sub_queries
    }

    /// 提取时间提示
    fn extract_temporal_hints(&self, query: &str) -> Option<TemporalHints> {
        let lower = query.to_lowercase();
        
        let has_time = lower.contains("最新") || lower.contains("最近") || lower.contains("recent")
            || lower.contains("latest") || lower.contains("new") || lower.contains("202")
            || lower.contains("今年") || lower.contains("去年");
        
        if !has_time {
            return None;
        }
        
        let recency = if lower.contains("最新") || lower.contains("latest") {
            1.0
        } else if lower.contains("最近") || lower.contains("recent") {
            0.8
        } else {
            0.5
        };
        
        Some(TemporalHints {
            has_time_constraint: true,
            time_range: None, // 可进一步解析具体时间范围
            recency_preference: recency,
        })
    }
}

impl Default for QueryAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_intent_classification() {
        let analyzer = QueryAnalyzer::new();
        
        let analysis = analyzer.analyze("总结这篇文章的要点").await.unwrap();
        assert_eq!(analysis.intent, QueryIntent::Summary);
        
        let analysis = analyzer.analyze("如何安装Rust").await.unwrap();
        assert_eq!(analysis.intent, QueryIntent::HowTo);
        
        let analysis = analyzer.analyze("什么是RAG").await.unwrap();
        assert_eq!(analysis.intent, QueryIntent::Definition);
    }

    #[tokio::test]
    async fn test_entity_extraction() {
        let analyzer = QueryAnalyzer::new();
        
        let analysis = analyzer.analyze("如何使用Rust和PDF解析库").await.unwrap();
        assert!(analysis.entities.iter().any(|e| e.name == "Rust"));
        assert!(analysis.entities.iter().any(|e| e.name == "PDF"));
    }
}
