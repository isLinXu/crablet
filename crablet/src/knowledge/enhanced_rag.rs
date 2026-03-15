use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::join;

use crate::knowledge::query_analyzer::{QueryAnalysis, QueryAnalyzer, QueryIntent};
use crate::knowledge::vector_store::VectorStore;
use crate::memory::semantic::SharedKnowledgeGraph;

/// 检索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedDocument {
    pub content: String,
    pub source: String,
    pub score: f32,
    pub retrieval_type: RetrievalType,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RetrievalType {
    Vector,
    Keyword,
    Graph,
    Expanded,
}

impl RetrievalType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RetrievalType::Vector => "vector",
            RetrievalType::Keyword => "keyword",
            RetrievalType::Graph => "graph",
            RetrievalType::Expanded => "expanded",
        }
    }
}

/// 多路召回结果
#[derive(Debug, Clone)]
pub struct MultiRouteResults {
    pub vector: Vec<RetrievedDocument>,
    pub keyword: Vec<RetrievedDocument>,
    pub graph: Vec<RetrievedDocument>,
    pub expanded: Vec<RetrievedDocument>,
}

impl MultiRouteResults {
    pub fn all(&self) -> Vec<RetrievedDocument> {
        let mut all = Vec::new();
        all.extend(self.vector.clone());
        all.extend(self.keyword.clone());
        all.extend(self.graph.clone());
        all.extend(self.expanded.clone());
        all
    }
}

/// 重排序配置
#[derive(Debug, Clone)]
pub struct RerankConfig {
    pub vector_weight: f32,
    pub keyword_weight: f32,
    pub graph_weight: f32,
    pub diversity_weight: f32,
    pub recency_weight: f32,
    pub max_results: usize,
}

impl Default for RerankConfig {
    fn default() -> Self {
        Self {
            vector_weight: 0.4,
            keyword_weight: 0.2,
            graph_weight: 0.2,
            diversity_weight: 0.1,
            recency_weight: 0.1,
            max_results: 10,
        }
    }
}

/// 增强版RAG系统
pub struct EnhancedRAG {
    query_analyzer: QueryAnalyzer,
    vector_store: Arc<VectorStore>,
    knowledge_graph: SharedKnowledgeGraph,
    config: RerankConfig,
    // 缓存
    query_cache: moka::future::Cache<String, Vec<RetrievedDocument>>,
}

impl EnhancedRAG {
    pub fn new(
        vector_store: Arc<VectorStore>,
        knowledge_graph: SharedKnowledgeGraph,
    ) -> Self {
        Self::with_config(vector_store, knowledge_graph, RerankConfig::default())
    }

    pub fn with_config(
        vector_store: Arc<VectorStore>,
        knowledge_graph: SharedKnowledgeGraph,
        config: RerankConfig,
    ) -> Self {
        let query_cache = moka::future::Cache::builder()
            .max_capacity(1000)
            .time_to_live(std::time::Duration::from_secs(300))
            .build();

        Self {
            query_analyzer: QueryAnalyzer::new(),
            vector_store,
            knowledge_graph,
            config,
            query_cache,
        }
    }

    /// 主检索接口
    pub async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<RetrievedDocument>> {
        // 1. 检查缓存
        if let Some(cached) = self.query_cache.get(query).await {
            return Ok(cached);
        }

        // 2. 查询分析
        let analysis = self.query_analyzer.analyze(query).await?;
        
        // 3. 多路召回
        let multi_results = self.multi_route_retrieve(&analysis, top_k * 2).await?;
        
        // 4. 融合与重排序
        let ranked = self.fuse_and_rerank(multi_results, &analysis, top_k).await?;
        
        // 5. 更新缓存
        self.query_cache.insert(query.to_string(), ranked.clone()).await;
        
        Ok(ranked)
    }

    /// 多路召回
    async fn multi_route_retrieve(
        &self,
        analysis: &QueryAnalysis,
        top_k: usize,
    ) -> Result<MultiRouteResults> {
        // 并行执行多种检索策略
        let (vector_results, keyword_results, graph_results, expanded_results) = join!(
            self.vector_search(&analysis.rewritten, top_k),
            self.keyword_search(&analysis.keywords, top_k),
            self.graph_search(&analysis.entities, top_k),
            self.expanded_search(&analysis.sub_queries, top_k),
        );

        Ok(MultiRouteResults {
            vector: vector_results?,
            keyword: keyword_results?,
            graph: graph_results?,
            expanded: expanded_results?,
        })
    }

    /// 向量检索
    async fn vector_search(
        &self,
        queries: &[String],
        top_k: usize,
    ) -> Result<Vec<RetrievedDocument>> {
        let mut all_results = Vec::new();
        
        for query in queries {
            let results = self.vector_store.search(query, top_k).await?;
            for (content, score, metadata) in results {
                all_results.push(RetrievedDocument {
                    content,
                    source: metadata.get("source")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    score,
                    retrieval_type: RetrievalType::Vector,
                    metadata,
                });
            }
        }
        
        // 去重
        self.deduplicate_by_content(all_results)
    }

    /// 关键词检索（简化版，可扩展为BM25）
    async fn keyword_search(
        &self,
        keywords: &[String],
        top_k: usize,
    ) -> Result<Vec<RetrievedDocument>> {
        if keywords.is_empty() {
            return Ok(Vec::new());
        }

        // 构建关键词查询
        let keyword_query = keywords.join(" ");
        
        // 使用向量存储的搜索作为基础，但后续可以替换为专门的关键词索引
        let results = self.vector_store.search(&keyword_query, top_k).await?;
        
        let mut docs = Vec::new();
        for (content, score, metadata) in results {
            // 关键词匹配度评分
            let keyword_score = self.calculate_keyword_score(&content, keywords);
            
            docs.push(RetrievedDocument {
                content,
                source: metadata.get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                score: keyword_score * score, // 结合原始分数
                retrieval_type: RetrievalType::Keyword,
                metadata,
            });
        }
        
        self.deduplicate_by_content(docs)
    }

    /// 图检索
    async fn graph_search(
        &self,
        entities: &[crate::knowledge::query_analyzer::ExtractedEntity],
        top_k: usize,
    ) -> Result<Vec<RetrievedDocument>> {
        let mut results = Vec::new();
        
        for entity in entities.iter().take(5) {
            let relations = self.knowledge_graph.find_related(&entity.name).await?;
            
            for (direction, relation, target) in relations.iter().take(top_k / entities.len().max(1)) {
                let content = format!("{} {} {} {}", entity.name, direction, relation, target);
                results.push(RetrievedDocument {
                    content,
                    source: "knowledge_graph".to_string(),
                    score: 0.8, // 图关系默认分数
                    retrieval_type: RetrievalType::Graph,
                    metadata: serde_json::json!({
                        "entity": entity.name,
                        "relation": relation,
                        "target": target,
                    }),
                });
            }
        }
        
        Ok(results)
    }

    /// 扩展查询检索
    async fn expanded_search(
        &self,
        sub_queries: &[String],
        top_k: usize,
    ) -> Result<Vec<RetrievedDocument>> {
        let mut all_results = Vec::new();
        
        for query in sub_queries.iter().take(3) {
            let results = self.vector_store.search(query, top_k / sub_queries.len().max(1)).await?;
            for (content, score, metadata) in results {
                all_results.push(RetrievedDocument {
                    content,
                    source: metadata.get("source")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    score: score * 0.9, // 子查询结果略微降权
                    retrieval_type: RetrievalType::Expanded,
                    metadata,
                });
            }
        }
        
        self.deduplicate_by_content(all_results)
    }

    /// 融合与重排序
    async fn fuse_and_rerank(
        &self,
        multi_results: MultiRouteResults,
        analysis: &QueryAnalysis,
        top_k: usize,
    ) -> Result<Vec<RetrievedDocument>> {
        let mut all_docs = multi_results.all();
        
        // 1. 根据检索类型加权
        for doc in &mut all_docs {
            let type_weight = match doc.retrieval_type {
                RetrievalType::Vector => self.config.vector_weight,
                RetrievalType::Keyword => self.config.keyword_weight,
                RetrievalType::Graph => self.config.graph_weight,
                RetrievalType::Expanded => self.config.vector_weight * 0.9,
            };
            doc.score *= type_weight;
        }
        
        // 2. 应用时效性权重
        if let Some(temporal) = &analysis.temporal_hints {
            if temporal.has_time_constraint {
                for doc in &mut all_docs {
                    let recency_boost = self.calculate_recency_boost(&doc.metadata, temporal.recency_preference);
                    doc.score = doc.score * (1.0 - self.config.recency_weight) 
                        + recency_boost * self.config.recency_weight;
                }
            }
        }
        
        // 3. MMR多样性重排序
        let diverse_results = self.mmr_rerank(all_docs, top_k, 0.5);
        
        // 4. 根据意图调整排序
        let final_results = self.adjust_for_intent(diverse_results, analysis.intent);
        
        Ok(final_results.into_iter().take(top_k).collect())
    }

    /// MMR (Maximal Marginal Relevance) 多样性重排序
    fn mmr_rerank(
        &self,
        mut docs: Vec<RetrievedDocument>,
        top_k: usize,
        lambda: f32,
    ) -> Vec<RetrievedDocument> {
        if docs.is_empty() {
            return docs;
        }
        
        // 按分数排序
        docs.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        
        let mut selected = Vec::new();
        let mut remaining: Vec<RetrievedDocument> = docs;
        
        while selected.len() < top_k && !remaining.is_empty() {
            if selected.is_empty() {
                // 选择第一个最高分的
                selected.push(remaining.remove(0));
            } else {
                // 计算MMR分数
                let mut best_idx = 0;
                let mut best_mmr_score = f32::MIN;
                
                for (idx, doc) in remaining.iter().enumerate() {
                    let relevance = doc.score;
                    let max_sim = selected.iter()
                        .map(|s| self.content_similarity(&doc.content, &s.content))
                        .fold(0.0f32, f32::max);
                    
                    let mmr_score = lambda * relevance - (1.0 - lambda) * max_sim;
                    
                    if mmr_score > best_mmr_score {
                        best_mmr_score = mmr_score;
                        best_idx = idx;
                    }
                }
                
                selected.push(remaining.remove(best_idx));
            }
        }
        
        selected
    }

    /// 根据意图调整结果
    fn adjust_for_intent(
        &self,
        mut docs: Vec<RetrievedDocument>,
        intent: QueryIntent,
    ) -> Vec<RetrievedDocument> {
        match intent {
            QueryIntent::Summary => {
                // 摘要意图：优先选择覆盖面广的文档
                for doc in &mut docs {
                    if doc.content.len() > 500 {
                        doc.score *= 1.1;
                    }
                }
            }
            QueryIntent::Comparison => {
                // 比较意图：确保多样性
                // MMR已经处理了多样性
            }
            QueryIntent::Temporal => {
                // 时间意图：优先最新文档
                for doc in &mut docs {
                    doc.score *= 1.1;
                }
            }
            _ => {}
        }
        
        // 重新排序
        docs.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        docs
    }

    /// 计算关键词匹配分数
    fn calculate_keyword_score(&self, content: &str, keywords: &[String]) -> f32 {
        let content_lower = content.to_lowercase();
        let mut matches = 0;
        
        for keyword in keywords {
            if content_lower.contains(&keyword.to_lowercase()) {
                matches += 1;
            }
        }
        
        if keywords.is_empty() {
            0.0
        } else {
            matches as f32 / keywords.len() as f32
        }
    }

    /// 计算时效性权重
    fn calculate_recency_boost(&self, metadata: &serde_json::Value, preference: f32) -> f32 {
        // 检查metadata中是否有时间信息
        if let Some(timestamp) = metadata.get("timestamp").and_then(|v| v.as_i64()) {
            let now = chrono::Utc::now().timestamp();
            let age_days = (now - timestamp) / 86400;
            
            // 指数衰减
            let decay = (-0.01 * age_days as f32).exp();
            decay * preference
        } else {
            0.5 // 默认中等时效性
        }
    }

    /// 内容相似度（简化版Jaccard）
    fn content_similarity(&self, a: &str, b: &str) -> f32 {
        let a_words: HashSet<String> = a.to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        let b_words: HashSet<String> = b.to_lowercase()
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        
        let intersection: HashSet<_> = a_words.intersection(&b_words).collect();
        let union: HashSet<_> = a_words.union(&b_words).collect();
        
        if union.is_empty() {
            0.0
        } else {
            intersection.len() as f32 / union.len() as f32
        }
    }

    /// 按内容去重
    fn deduplicate_by_content(
        &self,
        docs: Vec<RetrievedDocument>,
    ) -> Result<Vec<RetrievedDocument>> {
        let mut seen = HashSet::new();
        let mut unique = Vec::new();
        
        for doc in docs {
            // 使用内容的前100个字符作为去重键
            let key = doc.content.chars().take(100).collect::<String>();
            if !seen.contains(&key) {
                seen.insert(key);
                unique.push(doc);
            }
        }
        
        Ok(unique)
    }

    /// 获取检索统计
    pub fn get_stats(&self) -> serde_json::Value {
        serde_json::json!({
            "query_cache_size": self.query_cache.entry_count(),
            "config": {
                "vector_weight": self.config.vector_weight,
                "keyword_weight": self.config.keyword_weight,
                "graph_weight": self.config.graph_weight,
                "max_results": self.config.max_results,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enhanced_rag_retrieval() {
        // 这个测试需要完整的依赖注入，这里仅作为结构示例
        // 实际测试需要 mock VectorStore 和 KnowledgeGraph
    }
}
