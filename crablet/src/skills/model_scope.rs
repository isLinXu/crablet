//! ModelScope 技能平台接入模块
//!
//! ModelScope (魔搭社区) 是阿里推出的 AI 模型服务平台
//! 提供 MS-Agent 技能系统，支持模型的发现、训练、评估等功能
//!
//! 官网: https://www.modelscope.cn
//! GitHub: https://github.com/modelscope/ms-agent
//! Skills Central: https://www.modelscope.cn/skills
//! MCP Marketplace: https://www.modelscope.cn/mcp

use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;
use tracing::{info, warn, debug};

/// ModelScope 平台配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelScopeConfig {
    /// API 基础 URL
    pub api_base: Option<String>,
    /// GitHub 技能仓库 (官方 MS-Agent)
    pub github_repo: Option<String>,
    /// 备用 GitHub 仓库 (社区维护)
    pub community_repo: Option<String>,
    /// 超时时间(秒)
    pub timeout_secs: Option<u64>,
    /// API Token (可选)
    pub api_token: Option<String>,
    /// 默认安装目录
    pub default_install_dir: Option<PathBuf>,
    /// 最大重试次数
    pub max_retries: Option<u32>,
}

impl Default for ModelScopeConfig {
    fn default() -> Self {
        Self {
            api_base: Some("https://api-inference.modelscope.cn/v1".to_string()),
            // 官方 MS-Agent 仓库
            github_repo: Some("modelscope/ms-agent".to_string()),
            // 社区维护的 Skills 仓库
            community_repo: Some("hyf020908/modelscope-skills".to_string()),
            timeout_secs: Some(30),
            api_token: None,
            default_install_dir: Some(PathBuf::from("~/.workbuddy/skills")),
            max_retries: Some(3),
        }
    }
}

/// ModelScope 技能条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelScopeSkill {
    /// 技能名称
    pub name: String,
    /// 技能描述
    pub description: String,
    /// 版本
    pub version: Option<String>,
    /// 作者
    pub author: Option<String>,
    /// 入口文件 (通常是 SKILL.md)
    pub entry_point: String,
    /// 依赖的脚本
    pub scripts: Vec<String>,
    /// 分类
    pub category: Option<String>,
    /// 标签
    pub tags: Vec<String>,
    /// 需要的系统依赖
    pub system_requirements: Vec<String>,
    /// 需要的模型
    pub models: Vec<String>,
    /// 使用示例
    pub examples: Vec<String>,
}

/// ModelScope 数据集条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelScopeDataset {
    /// 数据集名称
    pub name: String,
    /// 数据集描述
    pub description: String,
    /// 数据集 URL
    pub url: String,
    /// 任务类型
    pub task: Option<String>,
}

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelScopeSearchResult {
    pub skills: Vec<ModelScopeSkill>,
    pub datasets: Vec<ModelScopeDataset>,
    pub total: usize,
    pub page: usize,
}

/// Skill 测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelScopeTestResult {
    /// 测试是否通过
    pub passed: bool,
    /// 测试名称
    pub test_name: String,
    /// 输出内容
    pub output: String,
    /// 错误信息(如果有)
    pub error: Option<String>,
    /// 执行时间(毫秒)
    pub duration_ms: u64,
}

/// Skill 安装结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelScopeInstallResult {
    /// 安装是否成功
    pub success: bool,
    /// 技能名称
    pub skill_name: String,
    /// 安装路径
    pub install_path: PathBuf,
    /// 消息
    pub message: String,
    /// 警告(如果有)
    pub warnings: Vec<String>,
}

/// ModelScope 客户端
pub struct ModelScopeClient {
    config: ModelScopeConfig,
    client: Client,
}

impl ModelScopeClient {
    /// 创建新的 ModelScope 客户端
    pub fn new(config: ModelScopeConfig) -> Self {
        let timeout = config.timeout_secs.unwrap_or(30);
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout))
            .build()
            .unwrap_or_default();

        Self { config, client }
    }

    /// 使用默认配置创建客户端
    pub fn default_config() -> Self {
        Self::new(ModelScopeConfig::default())
    }

    /// 检查 modelscope CLI 是否已安装
    pub fn is_cli_installed() -> bool {
        which::which("modelscope").is_ok() || which::which("ms").is_ok()
    }

    /// 获取默认安装目录
    pub fn get_default_install_dir(&self) -> PathBuf {
        self.config.default_install_dir.clone()
            .unwrap_or_else(|| PathBuf::from("~/.workbuddy/skills"))
            .expand_user().unwrap_or_else(|_| PathBuf::from("./skills"))
    }

    /// 获取技能列表 (官方 MS-Agent)
    pub async fn list_skills(&self) -> Result<Vec<ModelScopeSkill>> {
        let github_repo = self.config.github_repo.as_deref()
            .unwrap_or("modelscope/ms-agent");
        let url = format!(
            "https://raw.githubusercontent.com/{}/main/skills/skills.json",
            github_repo
        );

        info!("Fetching skills from: {}", url);

        let resp = self.client.get(&url).send().await
            .context("Failed to fetch ModelScope skills list")?;

        if !resp.status().is_success() {
            warn!("Failed to fetch official skills, trying community repo");
            return self.list_community_skills().await;
        }

        #[derive(Deserialize)]
        struct SkillsResponse {
            skills: Vec<serde_json::Value>,
        }

        let response: SkillsResponse = resp.json().await
            .context("Failed to parse skills list")?;

        let skills: Vec<ModelScopeSkill> = response.skills
            .into_iter()
            .filter_map(|s| {
                serde_json::from_value(s).ok()
            })
            .collect();

        Ok(skills)
    }

    /// 获取社区技能列表 (备用)
    pub async fn list_community_skills(&self) -> Result<Vec<ModelScopeSkill>> {
        let community_repo = self.config.community_repo.as_deref()
            .unwrap_or("hyf020908/modelscope-skills");
        let url = format!(
            "https://raw.githubusercontent.com/{}/main/.claude-plugin/marketplace.json",
            community_repo
        );

        info!("Fetching community skills from: {}", url);

        let resp = self.client.get(&url).send().await
            .context("Failed to fetch community skills list")?;

        if !resp.status().is_success() {
            anyhow::bail!("Failed to fetch community skills list: {}", resp.status());
        }

        #[derive(Deserialize)]
        struct MarketplaceResponse {
            skills: Vec<serde_json::Value>,
        }

        let marketplace: MarketplaceResponse = resp.json().await
            .context("Failed to parse skills list")?;

        let skills: Vec<ModelScopeSkill> = marketplace.skills
            .into_iter()
            .filter_map(|s| {
                serde_json::from_value(s).ok()
            })
            .collect();

        Ok(skills)
    }

    /// 搜索技能 (同时搜索官方和社区)
    pub async fn search_skills(&self, query: &str) -> Result<Vec<ModelScopeSkill>> {
        let mut all_skills = Vec::new();

        // 搜索官方技能
        if let Ok(official) = self.list_skills().await {
            all_skills.extend(official);
        }

        // 搜索社区技能
        if let Ok(community) = self.list_community_skills().await {
            all_skills.extend(community);
        }

        // 去重 (基于名称)
        let mut unique_skills = std::collections::HashMap::new();
        for skill in all_skills {
            unique_skills.entry(skill.name.clone())
                .or_insert(skill);
        }

        let query_lower = query.to_lowercase();
        let filtered: Vec<ModelScopeSkill> = unique_skills
            .into_values()
            .filter(|s| {
                s.name.to_lowercase().contains(&query_lower) ||
                s.description.to_lowercase().contains(&query_lower) ||
                s.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .collect();

        Ok(filtered)
    }

    /// 获取单个技能详情
    pub async fn get_skill(&self, name: &str) -> Result<ModelScopeSkill> {
        let github_repo = self.config.github_repo.as_deref()
            .unwrap_or("hyf020908/modelscope-skills");
        let url = format!(
            "https://raw.githubusercontent.com/{}/main/skills/{}/SKILL.md",
            github_repo, name
        );
        
        info!("Fetching skill from: {}", url);
        
        let resp = self.client.get(&url).send().await
            .context("Failed to fetch skill")?;
        
        if !resp.status().is_success() {
            anyhow::bail!("Skill '{}' not found: {}", name, resp.status());
        }
        
        let content = resp.text().await
            .context("Failed to read skill content")?;
        
        // 解析 SKILL.md 文件
        let skill = self.parse_skill_md(name, &content)?;
        
        Ok(skill)
    }

    /// 解析 SKILL.md 文件
    fn parse_skill_md(&self, name: &str, content: &str) -> Result<ModelScopeSkill> {
        // SKILL.md 格式通常是 YAML front matter + Markdown 内容
        let mut description = String::new();
        let mut tags = Vec::new();
        let mut examples = Vec::new();
        
        let lines: Vec<&str> = content.lines().collect();
        let mut in_front_matter = false;
        let mut front_matter_lines = Vec::new();
        
        for line in &lines {
            if line.trim() == "---" {
                if in_front_matter {
                    in_front_matter = false;
                } else {
                    in_front_matter = true;
                }
                continue;
            }
            
            if in_front_matter {
                front_matter_lines.push(*line);
            } else if line.starts_with("# ") {
                // 标题行，可能是描述
                if description.is_empty() {
                    description = line.trim_start_matches("# ").to_string();
                }
            } else if line.starts_with("## Example") || line.starts_with("## 示例") {
                // 接下来是示例
                let example_start = lines.iter().position(|l| *l == *line).unwrap_or(0);
                for example_line in &lines[example_start + 1..] {
                    if example_line.starts_with('#') || example_line.trim().is_empty() {
                        break;
                    }
                    if !example_line.trim().is_empty() {
                        examples.push(example_line.trim().to_string());
                    }
                }
            }
        }
        
        // 解析 front matter
        for line in &front_matter_lines {
            if line.starts_with("tags:") || line.starts_with("标签:") {
                tags = line.split(':').nth(1)
                    .map(|s| {
                        s.trim()
                            .trim_matches(|c| c == '[' || c == ']')
                            .split(',')
                            .map(|t| t.trim().trim_matches('"').to_string())
                            .collect()
                    })
                    .unwrap_or_default();
            }
        }
        
        Ok(ModelScopeSkill {
            name: name.to_string(),
            description,
            version: None,
            author: None,
            entry_point: format!("skills/{}/SKILL.md", name),
            scripts: Vec::new(),
            category: None,
            tags,
            system_requirements: Vec::new(),
            models: Vec::new(),
            examples,
        })
    }

    /// 安装技能到本地目录
    pub async fn install_skill(&self, name: &str, target_dir: &Path) -> Result<ModelScopeInstallResult> {
        let default_dir = self.get_default_install_dir();
        let target_dir = if target_dir.as_os_str().is_empty() {
            &default_dir
        } else {
            target_dir
        };

        info!("Installing skill '{}' to {:?}", name, target_dir);

        // 确保目标目录存在
        if !target_dir.exists() {
            fs::create_dir_all(target_dir).await
                .context("Failed to create target directory")?;
        }

        let target_path = target_dir.join(name);

        if target_path.exists() {
            return Ok(ModelScopeInstallResult {
                success: false,
                skill_name: name.to_string(),
                install_path: target_path.clone(),
                message: format!("Skill directory already exists: {:?}", target_path),
                warnings: vec![],
            });
        }

        // 尝试从官方仓库安装
        match self.install_from_repo(name, target_dir, self.config.github_repo.as_deref()).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                warn!("Failed to install from official repo: {}", e);
                // 尝试从社区仓库安装
                match self.install_from_repo(name, target_dir, self.config.community_repo.as_deref()).await {
                    Ok(result) => return Ok(result),
                    Err(e2) => {
                        return Ok(ModelScopeInstallResult {
                            success: false,
                            skill_name: name.to_string(),
                            install_path: target_path,
                            message: format!("Failed to install from both repos: {} / {}", e, e2),
                            warnings: vec![],
                        });
                    }
                }
            }
        }
    }

    /// 从指定仓库安装技能
    async fn install_from_repo(&self, name: &str, target_dir: &Path, repo: Option<&str>) -> Result<ModelScopeInstallResult> {
        let github_repo = repo.unwrap_or("modelscope/ms-agent");
        let skill_dir = format!("skills/{}", name);
        let target_path = target_dir.join(name);

        // 使用 GitHub API 获取仓库内容
        let api_url = format!(
            "https://api.github.com/repos/{}/contents/{}",
            github_repo, skill_dir
        );

        debug!("Fetching skill files from: {}", api_url);

        let resp = self.client.get(&api_url)
            .header("User-Agent", "ModelScope-Crablet-Client")
            .send().await
            .context("Failed to fetch skill files")?;

        if !resp.status().is_success() {
            // 如果 API 失败，尝试直接克隆仓库
            warn!("GitHub API failed, falling back to git clone");
            return self.install_via_git(name, target_dir, github_repo).await;
        }

        #[derive(Deserialize)]
        struct GitHubContent {
            #[serde(rename = "type")]
            file_type: String,
            download_url: Option<String>,
            name: String,
            #[serde(default)]
            path: String,
        }

        let contents: Vec<GitHubContent> = resp.json().await
            .context("Failed to parse GitHub API response")?;

        if contents.is_empty() {
            anyhow::bail!("Skill '{}' not found in repository", name);
        }

        // 创建技能目录
        fs::create_dir_all(&target_path).await
            .context("Failed to create skill directory")?;

        // 下载每个文件
        for item in contents {
            if item.file_type == "file" {
                if let Some(download_url) = item.download_url {
                    debug!("Downloading file: {}", item.name);
                    let file_resp = self.client.get(&download_url).send().await
                        .context("Failed to download file")?;

                    let file_content = file_resp.text().await
                        .context("Failed to read file content")?;

                    let file_path = target_path.join(&item.name);
                    fs::write(&file_path, file_content).await
                        .context("Failed to write file")?;
                }
            } else if item.file_type == "dir" {
                // 递归下载子目录
                self.download_directory(&target_path, github_repo, &item.path).await?;
            }
        }

        info!("Successfully installed skill '{}'", name);

        Ok(ModelScopeInstallResult {
            success: true,
            skill_name: name.to_string(),
            install_path: target_path,
            message: format!("Successfully installed '{}'", name),
            warnings: vec![],
        })
    }

    /// 递归下载目录
    async fn download_directory(&self, base_path: &Path, repo: &str, path: &str) -> Result<()> {
        let api_url = format!(
            "https://api.github.com/repos/{}/contents/{}",
            repo, path
        );

        let resp = self.client.get(&api_url)
            .header("User-Agent", "ModelScope-Crablet-Client")
            .send().await
            .context("Failed to fetch directory contents")?;

        if !resp.status().is_success() {
            anyhow::bail!("Failed to fetch directory: {}", path);
        }

        #[derive(Deserialize)]
        struct GitHubContent {
            #[serde(rename = "type")]
            file_type: String,
            download_url: Option<String>,
            name: String,
            path: String,
        }

        let contents: Vec<GitHubContent> = resp.json().await
            .context("Failed to parse directory contents")?;

        for item in contents {
            let relative_path = item.path.strip_prefix(path)
                .unwrap_or(&item.path);
            let file_path = base_path.join(relative_path);

            if item.file_type == "file" {
                if let Some(download_url) = item.download_url {
                    if let Some(parent) = file_path.parent() {
                        fs::create_dir_all(parent).await.ok();
                    }

                    let file_resp = self.client.get(&download_url).send().await
                        .context("Failed to download file")?;

                    let file_content = file_resp.text().await
                        .context("Failed to read file content")?;

                    fs::write(&file_path, file_content).await
                        .context("Failed to write file")?;
                }
            } else if item.file_type == "dir" {
                // 使用迭代代替递归，避免 boxing
                let dir_api_url = format!(
                    "https://api.github.com/repos/{}/contents/{}",
                    repo, item.path
                );
                
                let dir_resp = self.client.get(&dir_api_url)
                    .header("User-Agent", "ModelScope-Crablet-Client")
                    .send().await
                    .context("Failed to fetch subdirectory contents")?;

                if dir_resp.status().is_success() {
                    let dir_contents: Vec<GitHubContent> = dir_resp.json().await
                        .context("Failed to parse subdirectory contents")?;
                    
                    // 将子目录内容添加到当前列表继续处理
                    for sub_item in dir_contents {
                        let sub_relative_path = sub_item.path.strip_prefix(&item.path)
                            .unwrap_or(&sub_item.path);
                        let sub_file_path = file_path.join(sub_relative_path);
                        
                        if sub_item.file_type == "file" {
                            if let Some(download_url) = sub_item.download_url {
                                if let Some(parent) = sub_file_path.parent() {
                                    fs::create_dir_all(parent).await.ok();
                                }

                                let file_resp = self.client.get(&download_url).send().await
                                    .context("Failed to download file")?;

                                let file_content = file_resp.text().await
                                    .context("Failed to read file content")?;

                                fs::write(&sub_file_path, file_content).await
                                    .context("Failed to write file")?;
                            }
                        } else if sub_item.file_type == "dir" {
                            // 对于更深层的目录，继续递归调用 (这里使用 boxed)
                            Box::pin(self.download_directory(base_path, repo, &sub_item.path)).await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// 通过 git clone 安装技能
    async fn install_via_git(&self, name: &str, target_dir: &Path, repo: &str) -> Result<ModelScopeInstallResult> {
        let url = format!("https://github.com/{}", repo);

        info!("Cloning repository: {}", url);

        let target_path = target_dir.join(name);

        let status = Command::new("git")
            .args(["clone", "--depth", "1", &url, &target_path.to_string_lossy()])
            .status()
            .context("Failed to execute git clone")?;

        if !status.success() {
            anyhow::bail!("Failed to clone repository");
        }

        info!("Successfully installed skill '{}'", name);

        Ok(ModelScopeInstallResult {
            success: true,
            skill_name: name.to_string(),
            install_path: target_path,
            message: format!("Successfully installed '{}' via git clone", name),
            warnings: vec!["Installed entire repository, not just single skill".to_string()],
        })
    }

    /// 批量安装技能
    pub async fn install_batch(&self, skill_names: &[String], target_dir: &Path) -> Result<Vec<ModelScopeInstallResult>> {
        let mut results = Vec::new();

        for name in skill_names {
            let result = self.install_skill(name, target_dir).await
                .unwrap_or_else(|e| ModelScopeInstallResult {
                    success: false,
                    skill_name: name.clone(),
                    install_path: target_dir.join(name),
                    message: format!("Installation failed: {}", e),
                    warnings: vec![e.to_string()],
                });
            results.push(result);
        }

        Ok(results)
    }

    /// 搜索数据集
    pub async fn search_datasets(&self, query: &str) -> Result<Vec<ModelScopeDataset>> {
        let api_base = self.config.api_base.as_deref()
            .unwrap_or("https://modelscope.cn/api/v1");
        let url = format!("{}/datasets?q={}", api_base, query);
        
        info!("Searching datasets: {}", url);
        
        let resp = self.client.get(&url).send().await
            .context("Failed to search datasets")?;
        
        if !resp.status().is_success() {
            anyhow::bail!("Dataset search failed: {}", resp.status());
        }
        
        let result: ModelScopeSearchResult = resp.json().await
            .context("Failed to parse dataset search result")?;
        
        Ok(result.datasets)
    }

    /// 获取推荐技能
    pub async fn get_recommended_skills(&self) -> Result<Vec<ModelScopeSkill>> {
        let all = self.list_skills().await?;
        Ok(all.into_iter().take(10).collect())
    }

    /// 按分类获取技能
    pub async fn get_skills_by_category(&self, category: &str) -> Result<Vec<ModelScopeSkill>> {
        let all = self.list_skills().await?;
        let category_lower = category.to_lowercase();
        Ok(all.into_iter()
            .filter(|s| s.category.as_ref()
                .map(|c| c.to_lowercase().contains(&category_lower))
                .unwrap_or(false))
            .collect())
    }

    /// 列出已安装的技能
    pub async fn list_installed(&self, install_dir: &Path) -> Result<Vec<String>> {
        let mut skills = Vec::new();

        if !install_dir.exists() {
            return Ok(skills);
        }

        let mut entries = fs::read_dir(install_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                if path.join("SKILL.md").exists() {
                    if let Some(name) = path.file_name() {
                        skills.push(name.to_string_lossy().to_string());
                    }
                }
            }
        }

        Ok(skills)
    }

    /// 测试已安装的技能
    pub async fn test_skill(&self, skill_path: &Path) -> Result<Vec<ModelScopeTestResult>> {
        info!("Testing skill at {:?}", skill_path);

        let mut results = Vec::new();

        // 检查是否存在 SKILL.md
        let skill_md = skill_path.join("SKILL.md");
        if !skill_md.exists() {
            warn!("SKILL.md not found, skipping tests");
            return Ok(results);
        }

        // 检查是否有 tests 目录
        let tests_dir = skill_path.join("tests");
        if !tests_dir.exists() {
            info!("No tests directory found, trying to run basic validation");

            let content = fs::read_to_string(&skill_md).await?;

            let has_title = content.contains("# ");
            let has_description = content.to_lowercase().contains("description");

            results.push(ModelScopeTestResult {
                passed: has_title && has_description,
                test_name: "basic_validation".to_string(),
                output: format!("Title: {}, Description: {}", has_title, has_description),
                error: if !(has_title && has_description) { Some("Missing required fields".to_string()) } else { None },
                duration_ms: 0,
            });

            return Ok(results);
        }

        // 运行测试文件
        if let Ok(entries) = fs::read_dir(&tests_dir).await {
            let mut entries = entries;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "py" || ext == "sh" || ext == "ts" || ext == "js") {
                    let test_name = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
                    let result = self.run_test_file(&path, &test_name).await?;
                    results.push(result);
                }
            }
        }

        Ok(results)
    }

    /// 验证技能结构
    pub async fn validate_skill(&self, skill_path: &Path) -> Result<bool> {
        let mut errors = Vec::new();

        let required_files = vec!["SKILL.md"];
        for file in required_files {
            let file_path = skill_path.join(file);
            if !file_path.exists() {
                errors.push(format!("Missing required file: {}", file));
            }
        }

        if let Ok(content) = fs::read_to_string(skill_path.join("SKILL.md")).await {
            if !content.contains("# ") {
                errors.push("SKILL.md must start with a title".to_string());
            }
        }

        if errors.is_empty() {
            Ok(true)
        } else {
            for err in &errors {
                warn!("Skill validation error: {}", err);
            }
            Ok(false)
        }
    }

    /// 获取本地技能信息
    pub async fn get_local_skill_info(&self, skill_path: &Path) -> Result<Option<ModelScopeSkill>> {
        let skill_md = skill_path.join("SKILL.md");

        if !skill_md.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&skill_md).await?;

        let name = skill_path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let description = content.lines()
            .skip_while(|l| !l.starts_with("# "))
            .nth(1)
            .map(|l| l.trim().to_string())
            .unwrap_or_default();

        Ok(Some(ModelScopeSkill {
            name,
            description,
            version: None,
            author: None,
            entry_point: "SKILL.md".to_string(),
            scripts: vec![],
            category: None,
            tags: vec![],
            system_requirements: vec![],
            models: vec![],
            examples: vec![],
        }))
    }

    /// 卸载技能
    pub async fn uninstall_skill(&self, skill_name: &str, install_dir: &Path) -> Result<()> {
        let skill_path = install_dir.join(skill_name);

        if !skill_path.exists() {
            anyhow::bail!("Skill '{}' not found at {:?}", skill_name, skill_path);
        }

        fs::remove_dir_all(&skill_path).await
            .context("Failed to remove skill directory")?;

        info!("Successfully uninstalled '{}'", skill_name);
        Ok(())
    }

    /// 运行单个测试文件
    async fn run_test_file(&self, path: &Path, test_name: &str) -> Result<ModelScopeTestResult> {
        let start = std::time::Instant::now();

        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let output = match extension {
            "py" => {
                Command::new("python3")
                    .arg(path)
                    .output()
            },
            "sh" => {
                Command::new("bash")
                    .arg(path)
                    .output()
            },
            "ts" | "js" => {
                if which::which("deno").is_ok() {
                    Command::new("deno")
                        .args(["run", "-A", path.to_str().unwrap_or("")])
                        .output()
                } else {
                    Command::new("npx")
                        .args(["ts-node", path.to_str().unwrap_or("")])
                        .output()
                }
            },
            _ => {
                return Ok(ModelScopeTestResult {
                    passed: false,
                    test_name: test_name.to_string(),
                    output: String::new(),
                    error: Some(format!("Unsupported test file type: {}", extension)),
                    duration_ms: 0,
                });
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                Ok(ModelScopeTestResult {
                    passed: output.status.success(),
                    test_name: test_name.to_string(),
                    output: stdout,
                    error: if output.status.success() { None } else { Some(stderr) },
                    duration_ms,
                })
            },
            Err(e) => {
                Ok(ModelScopeTestResult {
                    passed: false,
                    test_name: test_name.to_string(),
                    output: String::new(),
                    error: Some(e.to_string()),
                    duration_ms,
                })
            }
        }
    }
}

// 辅助 trait: 展开用户路径
trait PathExt {
    fn expand_user(self) -> Result<PathBuf>;
}

impl PathExt for PathBuf {
    fn expand_user(self) -> Result<PathBuf> {
        if self.starts_with("~/") {
            let home = std::env::var("HOME")
                .context("Failed to get HOME environment variable")?;
            let rest = self.strip_prefix("~/").unwrap();
            Ok(PathBuf::from(home).join(rest))
        } else {
            Ok(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modelscope_config_default() {
        let config = ModelScopeConfig::default();
        assert!(config.api_base.is_some());
        assert!(config.github_repo.is_some());
        assert!(config.community_repo.is_some());
    }

    #[test]
    fn test_modelscope_skill_serde() {
        let json = r#"{
            "name": "modelscope-cli",
            "description": "Execute ModelScope Hub commands via natural language",
            "entry_point": "skills/modelscope-cli/SKILL.md",
            "tags": ["cli", "model hub"],
            "scripts": [],
            "system_requirements": [],
            "models": [],
            "examples": ["Download dataset abc"]
        }"#;

        let skill: ModelScopeSkill = serde_json::from_str(json).unwrap();
        assert_eq!(skill.name, "modelscope-cli");
        assert!(skill.tags.contains(&"cli".to_string()));
    }
}