//! 国内技能平台统一管理器
//!
//! 提供对 SkillHub 和 ModelScope 等国内技能平台的统一访问接口

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

pub use super::skill_hub::{SkillHubClient, SkillHubSkill};
pub use super::model_scope::{ModelScopeClient, ModelScopeSkill};

/// 统一的平台源枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillPlatform {
    /// 腾讯 SkillHub
    SkillHub,
    /// 阿里 ModelScope
    ModelScope,
    /// 全球 ClawHub (原有)
    ClawHub,
}

impl Default for SkillPlatform {
    fn default() -> Self {
        Self::ClawHub
    }
}

impl std::fmt::Display for SkillPlatform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillPlatform::SkillHub => write!(f, "SkillHub"),
            SkillPlatform::ModelScope => write!(f, "ModelScope"),
            SkillPlatform::ClawHub => write!(f, "ClawHub"),
        }
    }
}

/// 统一的技能条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedSkill {
    /// 技能名称
    pub name: String,
    /// 技能描述
    pub description: String,
    /// 版本
    pub version: String,
    /// 作者
    pub author: Option<String>,
    /// 来源平台
    pub platform: SkillPlatform,
    /// 原始 URL
    pub source_url: String,
    /// 分类
    pub category: Option<String>,
    /// 标签
    pub tags: Vec<String>,
}

impl From<SkillHubSkill> for UnifiedSkill {
    fn from(skill: SkillHubSkill) -> Self {
        Self {
            name: skill.name,
            description: skill.description,
            version: skill.version,
            author: skill.author,
            platform: SkillPlatform::SkillHub,
            source_url: skill.source_url.unwrap_or_default(),
            category: skill.category,
            tags: skill.tags,
        }
    }
}

impl From<ModelScopeSkill> for UnifiedSkill {
    fn from(skill: ModelScopeSkill) -> Self {
        let name = skill.name;
        Self {
            name: name.clone(),
            description: skill.description,
            version: skill.version.unwrap_or_else(|| "unknown".to_string()),
            author: skill.author,
            platform: SkillPlatform::ModelScope,
            source_url: format!("https://modelscope.cn/skills/{}", name),
            category: skill.category,
            tags: skill.tags,
        }
    }
}

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedSearchResult {
    pub skills: Vec<UnifiedSkill>,
    pub total: usize,
    pub platforms_used: Vec<SkillPlatform>,
}

/// 国内平台管理器
pub struct ChinaPlatformManager {
    skillhub: Option<SkillHubClient>,
    modelscope: Option<ModelScopeClient>,
    default_platform: SkillPlatform,
}

impl Default for ChinaPlatformManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ChinaPlatformManager {
    /// 创建新的管理器
    pub fn new() -> Self {
        Self {
            skillhub: Some(SkillHubClient::default_config()),
            modelscope: Some(ModelScopeClient::default_config()),
            default_platform: SkillPlatform::SkillHub,
        }
    }

    /// 创建仅使用 CLI 的管理器 (无网络依赖)
    pub fn cli_only() -> Self {
        Self {
            skillhub: Some(SkillHubClient::default_config()),
            modelscope: None,
            default_platform: SkillPlatform::SkillHub,
        }
    }

    /// 设置默认平台
    pub fn with_default_platform(mut self, platform: SkillPlatform) -> Self {
        self.default_platform = platform;
        self
    }

    /// 检查 SkillHub CLI 是否可用
    pub fn is_skillhub_available(&self) -> bool {
        SkillHubClient::is_cli_installed()
    }

    /// 检查 ModelScope CLI 是否可用
    pub fn is_modelscope_available(&self) -> bool {
        ModelScopeClient::is_cli_installed()
    }

    /// 获取可用的平台列表
    pub fn available_platforms(&self) -> Vec<SkillPlatform> {
        let mut platforms = vec![SkillPlatform::ClawHub]; // ClawHub 始终可用
        if self.is_skillhub_available() {
            platforms.push(SkillPlatform::SkillHub);
        }
        if self.is_modelscope_available() {
            platforms.push(SkillPlatform::ModelScope);
        }
        platforms
    }

    /// 搜索技能
    pub async fn search(&self, query: &str, platform: Option<SkillPlatform>) -> Result<UnifiedSearchResult> {
        let platform = platform.unwrap_or(self.default_platform);
        
        match platform {
            SkillPlatform::SkillHub => {
                self.search_skillhub(query).await
            }
            SkillPlatform::ModelScope => {
                self.search_modelscope(query).await
            }
            SkillPlatform::ClawHub => {
                // 使用现有的 registry
                self.search_clawhub(query).await
            }
        }
    }

    /// 从所有可用平台搜索
    pub async fn search_all(&self, query: &str) -> Result<UnifiedSearchResult> {
        let mut all_skills = Vec::new();
        let mut platforms_used = Vec::new();

        // 从 SkillHub 搜索
        if let Some(client) = &self.skillhub {
            if self.is_skillhub_available() {
                match client.search_via_cli(query).await {
                    Ok(skills) => {
                        info!("Found {} skills from SkillHub", skills.len());
                        all_skills.extend(skills.into_iter().map(UnifiedSkill::from));
                        platforms_used.push(SkillPlatform::SkillHub);
                    }
                    Err(e) => {
                        info!("SkillHub search failed: {}", e);
                    }
                }
            }
        }

        // 从 ModelScope 搜索
        if let Some(client) = &self.modelscope {
            match client.search_skills(query).await {
                Ok(skills) => {
                    info!("Found {} skills from ModelScope", skills.len());
                    all_skills.extend(skills.into_iter().map(UnifiedSkill::from));
                    platforms_used.push(SkillPlatform::ModelScope);
                }
                Err(e) => {
                    info!("ModelScope search failed: {}", e);
                }
            }
        }

        Ok(UnifiedSearchResult {
            total: all_skills.len(),
            skills: all_skills,
            platforms_used,
        })
    }

    /// 从 SkillHub 搜索
    async fn search_skillhub(&self, query: &str) -> Result<UnifiedSearchResult> {
        let client = self.skillhub.as_ref()
            .ok_or_else(|| anyhow::anyhow!("SkillHub client not initialized"))?;

        let skills = if self.is_skillhub_available() {
            client.search_via_cli(query).await?
        } else {
            // 回退到 API
            let result = client.search_via_api(query, 1, 20).await?;
            result.skills
        };

        Ok(UnifiedSearchResult {
            total: skills.len(),
            skills: skills.into_iter().map(UnifiedSkill::from).collect(),
            platforms_used: vec![SkillPlatform::SkillHub],
        })
    }

    /// 从 ModelScope 搜索
    async fn search_modelscope(&self, query: &str) -> Result<UnifiedSearchResult> {
        let client = self.modelscope.as_ref()
            .ok_or_else(|| anyhow::anyhow!("ModelScope client not initialized"))?;

        let skills = client.search_skills(query).await?;

        Ok(UnifiedSearchResult {
            total: skills.len(),
            skills: skills.into_iter().map(UnifiedSkill::from).collect(),
            platforms_used: vec![SkillPlatform::ModelScope],
        })
    }

    /// 从 ClawHub 搜索
    async fn search_clawhub(&self, _query: &str) -> Result<UnifiedSearchResult> {
        // 这里可以调用现有的 registry search
        // 返回空结果作为占位
        Ok(UnifiedSearchResult {
            total: 0,
            skills: Vec::new(),
            platforms_used: vec![SkillPlatform::ClawHub],
        })
    }

    /// 安装技能
    pub async fn install(&self, name: &str, platform: SkillPlatform, target_dir: PathBuf) -> Result<()> {
        match platform {
            SkillPlatform::SkillHub => {
                let client = self.skillhub.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("SkillHub client not initialized"))?;
                client.install(name, &target_dir).await?;
                Ok(())
            }
            SkillPlatform::ModelScope => {
                let client = self.modelscope.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("ModelScope client not initialized"))?;
                let result = client.install_skill(name, &target_dir).await?;
                if result.success {
                    Ok(())
                } else {
                    anyhow::bail!("Failed to install skill: {}", result.message)
                }
            }
            SkillPlatform::ClawHub => {
                anyhow::bail!("ClawHub install not implemented in ChinaPlatformManager")
            }
        }
    }

    /// 获取精选榜单
    pub async fn get_featured(&self, platform: SkillPlatform) -> Result<Vec<UnifiedSkill>> {
        match platform {
            SkillPlatform::SkillHub => {
                let client = self.skillhub.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("SkillHub client not initialized"))?;
                let items = client.get_featured().await?;
                Ok(items.into_iter().map(|item| UnifiedSkill::from(item.skill)).collect())
            }
            SkillPlatform::ModelScope => {
                let client = self.modelscope.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("ModelScope client not initialized"))?;
                let skills = client.get_recommended_skills().await?;
                Ok(skills.into_iter().map(UnifiedSkill::from).collect())
            }
            SkillPlatform::ClawHub => {
                Ok(Vec::new())
            }
        }
    }

    /// 获取支持的分类
    pub async fn get_categories(&self, platform: SkillPlatform) -> Result<Vec<String>> {
        match platform {
            SkillPlatform::SkillHub => {
                let client = self.skillhub.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("SkillHub client not initialized"))?;
                client.get_categories().await
            }
            SkillPlatform::ModelScope => {
                // ModelScope 使用预定义分类
                Ok(vec![
                    "model-discovery".to_string(),
                    "dataset".to_string(),
                    "training".to_string(),
                    "evaluation".to_string(),
                    "deployment".to_string(),
                    "mcp".to_string(),
                ])
            }
            SkillPlatform::ClawHub => {
                Ok(Vec::new())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_platform_display() {
        assert_eq!(SkillPlatform::SkillHub.to_string(), "SkillHub");
        assert_eq!(SkillPlatform::ModelScope.to_string(), "ModelScope");
        assert_eq!(SkillPlatform::ClawHub.to_string(), "ClawHub");
    }

    #[test]
    fn test_unified_skill_from_skillhub() {
        let skill = SkillHubSkill {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            version: "1.0.0".to_string(),
            author: Some("tester".to_string()),
            download_url: "https://example.com/test.tar.gz".to_string(),
            source_url: None,
            downloads: Some(100),
            rating: Some(4.5),
            category: Some("tools".to_string()),
            tags: vec!["test".to_string()],
        };

        let unified: UnifiedSkill = skill.into();
        assert_eq!(unified.name, "test-skill");
        assert_eq!(unified.platform, SkillPlatform::SkillHub);
    }

    #[test]
    fn test_china_manager_available_platforms() {
        let manager = ChinaPlatformManager::cli_only();
        let platforms = manager.available_platforms();
        assert!(platforms.contains(&SkillPlatform::ClawHub));
    }
}