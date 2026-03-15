//! 语义化技能搜索模块
//! 
//! 基于向量相似度实现智能技能发现
//! 支持自然语言查询、技能推荐、相似技能发现

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, debug, warn};


/// 嵌入服务
#[derive(Debug, Clone)]
pub struct EmbeddingService;

impl EmbeddingService {
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        #[cfg(feature = "knowledge")]
        {
            // 使用 fastembed 生成嵌入向量
            use fastembed::{TextEmbedding, EmbeddingModel, InitOptions};
            
            let options = InitOptions::new(EmbeddingModel::BGESmallENV15)
                .with_show_download_progress(false);
            
            let mut model = TextEmbedding::try_new(options)?;
            
            let embeddings = model.embed(vec![text], None)?;
            if let Some(embedding) = embeddings.first() {
                Ok(embedding.clone())
            } else {
                Ok(vec![])
            }
        }
        
        #[cfg(not(feature = "knowledge"))]
        {
            let _ = text;
            Ok(vec![])
        }
    }
}

/// 技能搜索索引
#[derive(Debug, Clone)]
pub struct SkillSearchIndex {
    /// 技能向量嵌入 (skill_name -> embedding)
    embeddings: HashMap<String, Vec<f32>>,
    /// 技能元数据缓存
    metadata: HashMap<String, SkillSearchMetadata>,
    /// 嵌入服务
    embedding_service: EmbeddingService,
    /// 索引版本
    version: u64,
}

/// 技能搜索元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSearchMetadata {
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub author: String,
    pub version: String,
    pub category: SkillCategory,
    pub usage_count: u64,
    pub rating: f32,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// 技能分类
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SkillCategory {
    Development,
    DataAnalysis,
    SystemAdmin,
    ContentCreation,
    Communication,
    Automation,
    Security,
    Testing,
    Documentation,
    Other(String),
}

impl std::fmt::Display for SkillCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillCategory::Development => write!(f, "Development"),
            SkillCategory::DataAnalysis => write!(f, "Data Analysis"),
            SkillCategory::SystemAdmin => write!(f, "System Administration"),
            SkillCategory::ContentCreation => write!(f, "Content Creation"),
            SkillCategory::Communication => write!(f, "Communication"),
            SkillCategory::Automation => write!(f, "Automation"),
            SkillCategory::Security => write!(f, "Security"),
            SkillCategory::Testing => write!(f, "Testing"),
            SkillCategory::Documentation => write!(f, "Documentation"),
            SkillCategory::Other(s) => write!(f, "{}", s),
        }
    }
}

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSearchResult {
    pub skill_name: String,
    pub metadata: SkillSearchMetadata,
    pub similarity_score: f32,
    pub match_type: MatchType,
    pub matched_keywords: Vec<String>,
}

/// 匹配类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchType {
    /// 语义相似度匹配
    Semantic,
    /// 关键词匹配
    Keyword,
    /// 标签匹配
    Tag,
    /// 分类匹配
    Category,
    /// 混合匹配
    Hybrid,
}

/// 搜索查询
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub query: String,
    pub filters: SearchFilters,
    pub limit: usize,
    pub min_similarity: f32,
}

/// 搜索过滤器
#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    pub categories: Option<Vec<SkillCategory>>,
    pub tags: Option<Vec<String>>,
    pub author: Option<String>,
    pub min_rating: Option<f32>,
    pub skill_type: Option<String>,
}

/// 搜索建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSuggestion {
    pub text: String,
    pub category: Option<SkillCategory>,
    pub confidence: f32,
}

impl SkillSearchIndex {
    /// 创建新的搜索索引
    pub fn new(embedding_service: EmbeddingService) -> Self {
        Self {
            embeddings: HashMap::new(),
            metadata: HashMap::new(),
            embedding_service,
            version: 0,
        }
    }

    /// 从技能目录构建索引
    pub async fn build_from_directory(&mut self, skills_dir: &Path) -> Result<()> {
        info!("Building skill search index from {:?}", skills_dir);
        
        let mut entries = tokio::fs::read_dir(skills_dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                if let Err(e) = self.index_skill(&path).await {
                    warn!("Failed to index skill at {:?}: {}", path, e);
                }
            }
        }
        
        self.version += 1;
        info!("Skill search index built: {} skills indexed", self.embeddings.len());
        
        Ok(())
    }

    /// 索引单个技能
    async fn index_skill(&mut self, skill_path: &Path) -> Result<()> {
        let skill_name = skill_path
            .file_name()
            .and_then(|n| n.to_str())
            .context("Invalid skill path")?;

        // 尝试解析 SKILL.md
        let skill_md_path = skill_path.join("SKILL.md");
        if skill_md_path.exists() {
            let content = tokio::fs::read_to_string(&skill_md_path).await?;
            let metadata = self.parse_skill_md(&content, skill_name)?;
            
            // 生成嵌入向量
            let text_to_embed = format!(
                "{} {} {} {}",
                metadata.name,
                metadata.description,
                metadata.tags.join(" "),
                metadata.category.to_string()
            );
            
            let embedding = self.embedding_service.embed(&text_to_embed).await?;
            
            self.embeddings.insert(skill_name.to_string(), embedding);
            self.metadata.insert(skill_name.to_string(), metadata);
        }

        Ok(())
    }

    /// 解析 SKILL.md 文件
    fn parse_skill_md(&self, content: &str, default_name: &str) -> Result<SkillSearchMetadata> {
        let mut name = default_name.to_string();
        let mut description = String::new();
        let mut tags = Vec::new();
        let mut author = "Unknown".to_string();
        let mut version = "0.1.0".to_string();
        let mut category = SkillCategory::Other("General".to_string());

        // 简单的 YAML frontmatter 解析
        if content.starts_with("---") {
            if let Some(end) = content.find("\n---") {
                let frontmatter = &content[3..end];
                
                for line in frontmatter.lines() {
                    if let Some((key, value)) = line.split_once(':') {
                        let key = key.trim();
                        let value = value.trim().trim_matches('"').trim_matches('\'');
                        
                        match key {
                            "name" => name = value.to_string(),
                            "description" => description = value.to_string(),
                            "author" => author = value.to_string(),
                            "version" => version = value.to_string(),
                            "category" => category = self.parse_category(value),
                            _ => {}
                        }
                    }
                }
            }
        }

        // 从内容中提取标签 (#tag 格式)
        for word in content.split_whitespace() {
            if word.starts_with('#') && word.len() > 1 {
                tags.push(word[1..].to_string());
            }
        }

        Ok(SkillSearchMetadata {
            name,
            description,
            tags,
            author,
            version,
            category,
            usage_count: 0,
            rating: 0.0,
            last_updated: chrono::Utc::now(),
        })
    }

    fn parse_category(&self, value: &str) -> SkillCategory {
        match value.to_lowercase().as_str() {
            "development" | "dev" => SkillCategory::Development,
            "data" | "analysis" | "data-analysis" => SkillCategory::DataAnalysis,
            "system" | "admin" | "sysadmin" => SkillCategory::SystemAdmin,
            "content" | "creation" => SkillCategory::ContentCreation,
            "communication" | "chat" => SkillCategory::Communication,
            "automation" | "auto" => SkillCategory::Automation,
            "security" | "sec" => SkillCategory::Security,
            "testing" | "test" => SkillCategory::Testing,
            "documentation" | "docs" => SkillCategory::Documentation,
            _ => SkillCategory::Other(value.to_string()),
        }
    }

    /// 语义搜索
    pub async fn search(&self, query: &SearchQuery) -> Result<Vec<SkillSearchResult>> {
        debug!("Searching skills with query: {}", query.query);

        // 生成查询向量
        let query_embedding = self.embedding_service.embed(&query.query).await?;
        
        let mut results: Vec<SkillSearchResult> = Vec::new();

        for (skill_name, skill_embedding) in &self.embeddings {
            // 计算余弦相似度
            let similarity = cosine_similarity(&query_embedding, skill_embedding);
            
            if similarity < query.min_similarity {
                continue;
            }

            if let Some(metadata) = self.metadata.get(skill_name) {
                // 应用过滤器
                if !self.matches_filters(metadata, &query.filters) {
                    continue;
                }

                // 检查关键词匹配
                let keywords = extract_keywords(&query.query);
                let matched_keywords: Vec<String> = keywords
                    .iter()
                    .filter(|k| {
                        metadata.name.to_lowercase().contains(&k.to_lowercase())
                            || metadata.description.to_lowercase().contains(&k.to_lowercase())
                            || metadata.tags.iter().any(|t| t.to_lowercase() == k.to_lowercase())
                    })
                    .cloned()
                    .collect();

                let match_type = if !matched_keywords.is_empty() && similarity > 0.7 {
                    MatchType::Hybrid
                } else if !matched_keywords.is_empty() {
                    MatchType::Keyword
                } else {
                    MatchType::Semantic
                };

                results.push(SkillSearchResult {
                    skill_name: skill_name.clone(),
                    metadata: metadata.clone(),
                    similarity_score: similarity,
                    match_type,
                    matched_keywords,
                });
            }
        }

        // 按相似度排序
        results.sort_by(|a, b| {
            b.similarity_score
                .partial_cmp(&a.similarity_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 限制结果数量
        results.truncate(query.limit);

        Ok(results)
    }

    /// 检查是否匹配过滤器
    fn matches_filters(&self, metadata: &SkillSearchMetadata, filters: &SearchFilters) -> bool {
        if let Some(ref categories) = filters.categories {
            if !categories.contains(&metadata.category) {
                return false;
            }
        }

        if let Some(ref tags) = filters.tags {
            if !tags.iter().any(|t| metadata.tags.contains(t)) {
                return false;
            }
        }

        if let Some(ref author) = filters.author {
            if !metadata.author.to_lowercase().contains(&author.to_lowercase()) {
                return false;
            }
        }

        if let Some(min_rating) = filters.min_rating {
            if metadata.rating < min_rating {
                return false;
            }
        }

        true
    }

    /// 获取相似技能推荐
    pub async fn find_similar(&self, skill_name: &str, limit: usize) -> Result<Vec<SkillSearchResult>> {
        let skill_embedding = self.embeddings
            .get(skill_name)
            .context("Skill not found in index")?;

        let mut results: Vec<SkillSearchResult> = Vec::new();

        for (name, embedding) in &self.embeddings {
            if name == skill_name {
                continue;
            }

            let similarity = cosine_similarity(skill_embedding, embedding);
            
            if let Some(metadata) = self.metadata.get(name) {
                results.push(SkillSearchResult {
                    skill_name: name.clone(),
                    metadata: metadata.clone(),
                    similarity_score: similarity,
                    match_type: MatchType::Semantic,
                    matched_keywords: vec![],
                });
            }
        }

        results.sort_by(|a, b| {
            b.similarity_score
                .partial_cmp(&a.similarity_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results.truncate(limit);
        Ok(results)
    }

    /// 获取搜索建议
    pub fn get_suggestions(&self, partial_query: &str, limit: usize) -> Vec<SearchSuggestion> {
        let partial_lower = partial_query.to_lowercase();
        let mut suggestions: Vec<SearchSuggestion> = Vec::new();

        for metadata in self.metadata.values() {
            // 名称建议
            if metadata.name.to_lowercase().starts_with(&partial_lower) {
                suggestions.push(SearchSuggestion {
                    text: metadata.name.clone(),
                    category: Some(metadata.category.clone()),
                    confidence: 0.9,
                });
            }

            // 标签建议
            for tag in &metadata.tags {
                if tag.to_lowercase().starts_with(&partial_lower) {
                    suggestions.push(SearchSuggestion {
                        text: format!("#{}", tag),
                        category: Some(metadata.category.clone()),
                        confidence: 0.8,
                    });
                }
            }
        }

        // 去重并排序
        suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        suggestions.dedup_by(|a, b| a.text == b.text);
        suggestions.truncate(limit);

        suggestions
    }

    /// 按分类浏览
    pub fn browse_by_category(&self, category: &SkillCategory) -> Vec<&SkillSearchMetadata> {
        self.metadata
            .values()
            .filter(|m| &m.category == category)
            .collect()
    }

    /// 获取热门技能
    pub fn get_trending(&self, limit: usize) -> Vec<&SkillSearchMetadata> {
        let mut skills: Vec<&SkillSearchMetadata> = self.metadata.values().collect();
        skills.sort_by(|a, b| b.usage_count.cmp(&a.usage_count));
        skills.truncate(limit);
        skills
    }

    /// 获取索引统计
    pub fn get_stats(&self) -> IndexStats {
        IndexStats {
            total_skills: self.embeddings.len(),
            version: self.version,
            categories: self.metadata
                .values()
                .map(|m| m.category.clone())
                .collect::<std::collections::HashSet<_>>()
                .len(),
        }
    }
}

/// 索引统计
#[derive(Debug, Clone, Serialize)]
pub struct IndexStats {
    pub total_skills: usize,
    pub version: u64,
    pub categories: usize,
}

/// 计算余弦相似度
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot_product / (norm_a * norm_b)
}

/// 提取关键词
fn extract_keywords(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
        .filter(|w| w.len() > 2)
        .collect()
}

/// 技能搜索管理器
pub struct SkillSearchManager {
    index: SkillSearchIndex,
    skills_dir: std::path::PathBuf,
}

impl SkillSearchManager {
    pub fn new(skills_dir: std::path::PathBuf, embedding_service: EmbeddingService) -> Self {
        Self {
            index: SkillSearchIndex::new(embedding_service),
            skills_dir,
        }
    }

    /// 初始化索引
    pub async fn initialize(&mut self) -> Result<()> {
        if self.skills_dir.exists() {
            self.index.build_from_directory(&self.skills_dir).await?;
        }
        Ok(())
    }

    /// 搜索技能
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SkillSearchResult>> {
        let search_query = SearchQuery {
            query: query.to_string(),
            filters: SearchFilters::default(),
            limit,
            min_similarity: 0.5,
        };
        
        self.index.search(&search_query).await
    }

    /// 自然语言搜索
    pub async fn natural_language_search(&self, description: &str) -> Result<Vec<SkillSearchResult>> {
        info!("Performing natural language skill search: {}", description);
        
        // 使用 LLM 提取搜索意图
        let search_query = SearchQuery {
            query: description.to_string(),
            filters: SearchFilters::default(),
            limit: 10,
            min_similarity: 0.4, // 较低的阈值以获得更多结果
        };
        
        let results = self.index.search(&search_query).await?;
        
        // 如果没有找到结果，尝试更宽泛的搜索
        if results.is_empty() {
            let broad_query = SearchQuery {
                query: description.to_string(),
                filters: SearchFilters::default(),
                limit: 5,
                min_similarity: 0.2,
            };
            return self.index.search(&broad_query).await;
        }
        
        Ok(results)
    }

    /// 获取搜索建议
    pub fn get_suggestions(&self, partial: &str) -> Vec<SearchSuggestion> {
        self.index.get_suggestions(partial, 5)
    }

    /// 推荐相关技能
    pub async fn recommend_related(&self, skill_name: &str) -> Result<Vec<SkillSearchResult>> {
        self.index.find_similar(skill_name, 5).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);

        let c = vec![1.0, 0.0, 0.0];
        let d = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&c, &d) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_extract_keywords() {
        let text = "Find skills for data analysis";
        let keywords = extract_keywords(text);
        assert!(keywords.contains(&"find".to_string()));
        assert!(keywords.contains(&"skills".to_string()));
        assert!(keywords.contains(&"for".to_string()));
        assert!(keywords.contains(&"data".to_string()));
        assert!(keywords.contains(&"analysis".to_string()));
    }
}
