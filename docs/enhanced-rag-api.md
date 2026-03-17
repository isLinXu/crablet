# 增强版RAG API文档

## 概述

增强版RAG系统提供了多路召回、智能重排序、查询理解等高级功能，显著提升检索质量和命中率。

## 核心特性

### 1. 查询理解 (Query Understanding)
- **意图分类**: 自动识别查询类型（事实、摘要、比较、推理等）
- **实体提取**: 识别查询中的关键实体
- **查询重写**: 扩展同义词、改写查询
- **子查询分解**: 将复杂查询拆分为多个子查询

### 2. 多路召回 (Multi-Route Retrieval)
- **向量检索**: 语义相似度搜索
- **关键词检索**: BM25/TF-IDF精确匹配
- **图检索**: 基于知识图谱的关系遍历
- **语义扩展**: 使用LLM生成相关查询词

### 3. 智能重排序 (Intelligent Reranking)
- **MMR多样性排序**: 平衡相关性和多样性
- **时效性加权**: 优先新文档
- **意图适配**: 根据查询意图调整排序策略

## API接口

### 检索接口

```http
POST /v1/knowledge/enhanced-search
Content-Type: application/json

{
  "query": "查询文本",
  "top_k": 10,
  "config": {
    "enable_vector_search": true,
    "enable_keyword_search": true,
    "enable_graph_search": true,
    "vector_weight": 0.4,
    "keyword_weight": 0.2,
    "graph_weight": 0.2,
    "enable_mmr": true,
    "mmr_lambda": 0.5
  }
}
```

**响应示例:**

```json
{
  "results": [
    {
      "content": "检索到的文本内容",
      "source": "文档来源",
      "score": 0.85,
      "retrieval_type": "vector",
      "metadata": {
        "timestamp": 1700000000,
        "entity": "相关实体"
      }
    }
  ],
  "analysis": {
    "intent": "summary",
    "entities": ["entity1", "entity2"],
    "rewritten_queries": ["扩展查询1", "扩展查询2"],
    "sub_queries": ["子查询1"]
  },
  "stats": {
    "vector_hits": 5,
    "keyword_hits": 3,
    "graph_hits": 2,
    "total_time_ms": 150
  }
}
```

### 查询分析接口

```http
POST /v1/knowledge/analyze-query
Content-Type: application/json

{
  "query": "查询文本"
}
```

**响应示例:**

```json
{
  "original": "查询文本",
  "intent": "summary",
  "entities": [
    {
      "name": "实体名称",
      "entity_type": "Technology",
      "position": [0, 10]
    }
  ],
  "keywords": ["关键词1", "关键词2"],
  "rewritten": ["重写查询1", "重写查询2"],
  "sub_queries": ["子查询1"],
  "temporal_hints": {
    "has_time_constraint": true,
    "recency_preference": 0.8
  }
}
```

### RAG配置接口

```http
GET /v1/knowledge/rag-config
```

获取当前RAG系统配置。

```http
POST /v1/knowledge/rag-config
Content-Type: application/json

{
  "vector_weight": 0.4,
  "keyword_weight": 0.2,
  "graph_weight": 0.2,
  "diversity_weight": 0.1,
  "recency_weight": 0.1,
  "max_results": 10,
  "enable_mmr": true,
  "mmr_lambda": 0.5
}
```

更新RAG配置。

### 统计信息接口

```http
GET /v1/knowledge/rag-stats
```

**响应示例:**

```json
{
  "query_cache_size": 150,
  "hit_rates": {
    "vector": 0.75,
    "keyword": 0.45,
    "graph": 0.30
  },
  "average_latency_ms": 120,
  "total_queries": 1000
}
```

## 前端集成

### RAG配置面板

前端提供了可视化的RAG配置面板，用户可以通过界面调整：

1. **检索策略**: 启用/禁用不同的检索方式
2. **权重配置**: 调整各策略的权重
3. **高级选项**: MMR、去重、查询重写等

### 使用示例

```tsx
import { RagConfigPanel } from '@/components/rag/RagConfigPanel';

function ChatComponent() {
  const [showConfig, setShowConfig] = useState(false);
  const [ragConfig, setRagConfig] = useState(null);

  return (
    <>
      <button onClick={() => setShowConfig(true)}>
        配置RAG
      </button>
      
      <RagConfigPanel
        isOpen={showConfig}
        onClose={() => setShowConfig(false)}
        onConfigChange={setRagConfig}
      />
    </>
  );
}
```

## 性能优化

### 缓存策略
- **查询结果缓存**: 5分钟TTL
- **嵌入向量缓存**: 10分钟TTL
- **知识图谱子图缓存**: 2分钟TTL

### 异步并行
- 多路召回并行执行
- 预计算热门查询
- 流式返回结果

## 最佳实践

### 1. 查询优化
- 使用具体的关键词
- 避免过于宽泛的查询
- 利用查询重写功能

### 2. 权重调优
- 事实查询: 提高关键词权重
- 语义查询: 提高向量权重
- 关系查询: 提高图检索权重

### 3. 结果调优
- 适当调整 `max_results`
- 启用MMR避免重复内容
- 根据需求调整时效性权重

## 故障排查

### 检索命中率低
1. 检查知识库内容是否充足
2. 调整检索策略权重
3. 启用查询重写和扩展

### 响应速度慢
1. 检查缓存命中率
2. 减少 `max_results`
3. 禁用部分检索策略

### 结果重复
1. 启用去重功能
2. 调整MMR lambda参数
3. 增加多样性权重
