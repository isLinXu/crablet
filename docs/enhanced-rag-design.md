# 增强版RAG系统设计

## 1. 系统架构概览

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Enhanced RAG System                               │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│  │Query         │  │Query         │  │Multi-Route   │  │Result        │   │
│  │Understanding │→ │Rewrite       │→ │Retrieval     │→ │Fusion        │   │
│  │              │  │              │  │              │  │              │   │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘   │
│         │                 │                 │                 │            │
│         ▼                 ▼                 ▼                 ▼            │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │                        Retrieval Strategies                          │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │  │
│  │  │Vector    │  │Keyword   │  │Graph     │  │Semantic  │            │  │
│  │  │Search    │  │Search    │  │Traversal │  │Expansion │            │  │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘            │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
│                                    │                                       │
│                                    ▼                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │                      Intelligent Reranking                           │  │
│  │  - Cross-encoder scoring                                             │  │
│  │  - Diversity-aware ranking                                           │  │
│  │  - Temporal relevance boost                                          │  │
│  │  - Source authority weighting                                        │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
│                                    │                                       │
│                                    ▼                                       │
│  ┌─────────────────────────────────────────────────────────────────────┐  │
│  │                    Context Compression                               │  │
│  │  - Redundancy removal                                                │  │
│  │  - Hierarchical summarization                                        │  │
│  │  - Token budget management                                           │  │
│  └─────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────┘
```

## 2. 核心模块设计

### 2.1 查询理解与重写 (Query Understanding & Rewrite)

**功能：**
- 意图识别：分类查询类型（事实查询、摘要、比较、推理等）
- 查询重写：扩展同义词、处理否定、识别实体
- 子查询分解：将复杂查询拆分为多个子查询

**实现：**
```rust
pub struct QueryAnalyzer {
    intent_classifier: IntentClassifier,
    entity_extractor: EntityExtractor,
    query_expander: QueryExpander,
}

impl QueryAnalyzer {
    pub async fn analyze(&self, query: &str) -> QueryAnalysis {
        let intent = self.intent_classifier.classify(query).await;
        let entities = self.entity_extractor.extract(query).await;
        let rewritten = self.query_expander.expand(query, &entities).await;
        let sub_queries = self.decompose_query(query, &intent).await;
        
        QueryAnalysis {
            original: query.to_string(),
            intent,
            entities,
            rewritten,
            sub_queries,
        }
    }
}
```

### 2.2 多路召回 (Multi-Route Retrieval)

**策略：**
1. **向量检索**：语义相似度搜索
2. **关键词检索**：BM25/TF-IDF 精确匹配
3. **图检索**：基于知识图谱的关系遍历
4. **语义扩展**：使用LLM生成相关查询词

**实现：**
```rust
pub struct MultiRouteRetriever {
    vector_store: Arc<VectorStore>,
    keyword_index: Arc<KeywordIndex>,
    knowledge_graph: Arc<dyn KnowledgeGraph>,
    semantic_expander: Arc<SemanticExpander>,
}

impl MultiRouteRetriever {
    pub async fn retrieve(&self, query: &QueryAnalysis, top_k: usize) -> RetrievalResults {
        let (vector_results, keyword_results, graph_results, expanded_results) = tokio::join!(
            self.vector_search(&query.rewritten, top_k * 2),
            self.keyword_search(&query.original, top_k),
            self.graph_traversal(&query.entities, top_k),
            self.semantic_expansion(&query.sub_queries, top_k),
        );
        
        RetrievalResults {
            vector: vector_results,
            keyword: keyword_results,
            graph: graph_results,
            expanded: expanded_results,
        }
    }
}
```

### 2.3 智能重排序 (Intelligent Reranking)

**因素：**
- 交叉编码器分数（查询-文档相关性）
- 多样性分数（避免重复内容）
- 时效性权重（优先新文档）
- 来源权威性（知识库 vs 网页）
- 用户反馈权重（历史点击/评分）

**实现：**
```rust
pub struct Reranker {
    cross_encoder: CrossEncoder,
    diversity_calculator: DiversityCalculator,
    feedback_learner: FeedbackLearner,
}

impl Reranker {
    pub async fn rerank(&self, results: RetrievalResults, query: &str) -> Vec<RankedDocument> {
        // 1. 计算交叉编码器分数
        let cross_scores = self.cross_encoder.score(query, &results.all()).await;
        
        // 2. MMR (Maximal Marginal Relevance) 多样性排序
        let diverse_results = self.mmr_rerank(results.all(), cross_scores, lambda=0.5);
        
        // 3. 应用反馈权重
        self.apply_feedback_weights(&mut diverse_results).await;
        
        diverse_results
    }
}
```

### 2.4 上下文压缩 (Context Compression)

**策略：**
- 冗余检测：去除重复或相似内容
- 分层摘要：长文档生成多级摘要
- Token预算管理：根据模型限制动态调整

**实现：**
```rust
pub struct ContextCompressor {
    redundancy_detector: RedundancyDetector,
    summarizer: HierarchicalSummarizer,
    token_budget: usize,
}

impl ContextCompressor {
    pub async fn compress(&self, documents: Vec<RankedDocument>) -> CompressedContext {
        // 1. 去除冗余
        let unique_docs = self.redundancy_detector.deduplicate(documents);
        
        // 2. 分层摘要
        let summaries = self.summarizer.summarize(&unique_docs).await;
        
        // 3. 根据token预算选择内容
        self.select_by_budget(summaries, self.token_budget)
    }
}
```

## 3. 性能优化

### 3.1 缓存策略
- 查询结果缓存（TTL 5分钟）
- 嵌入向量缓存
- 知识图谱子图缓存

### 3.2 异步并行
- 多路召回并行执行
- 预计算热门查询
- 流式返回结果

### 3.3 索引优化
- HNSW向量索引
- 倒排索引压缩
- 增量更新支持

## 4. 可观测性

### 4.1 指标收集
- 检索延迟（P50/P95/P99）
- 命中率（向量/关键词/图）
- 重排序前后NDCG对比
- 用户反馈统计

### 4.2 追踪
- 查询处理流水线追踪
- 每个检索策略的贡献度
- 上下文压缩率

## 5. 前端增强

### 5.1 RAG配置面板
- 检索策略开关
- 重排序权重调整
- Token预算设置
- 反馈提交按钮

### 5.2 结果可视化
- 检索来源标注
- 相关性分数显示
- 知识图谱关系图
- 上下文使用统计
