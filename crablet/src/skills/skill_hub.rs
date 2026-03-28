//! SkillHub 平台接入模块
//!
//! SkillHub 是腾讯推出的面向中国用户优化的 AI Skills 社区
//! 基于 ClawHub 生态，提供国内高速镜像下载
//!
//! 官网: https://skillhub.tencent.com/
//! 安装文档: https://skillhub-1388575217.cos.ap-guangzhou.myqcloud.com/install/skillhub.md

use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;
use tracing::{info, warn, error, debug};

/// SkillHub 平台配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillHubConfig {
    /// 镜像站点 URL (用于加速下载)
    pub mirror_url: Option<String>,
    /// API 基础 URL
    pub api_base: Option<String>,
    /// 超时时间(秒)
    pub timeout_secs: Option<u64>,
    /// 默认安装目录
    pub default_install_dir: Option<PathBuf>,
    /// 是否启用 CLI 回退
    pub fallback_to_cli: Option<bool>,
    /// 重试次数
    pub max_retries: Option<u32>,
}

impl Default for SkillHubConfig {
    fn default() -> Self {
        Self {
            mirror_url: Some("https://skillhub-1388575217.cos.ap-guangzhou.myqcloud.com".to_string()),
            api_base: Some("https://skillhub.tencent.com/api/v1".to_string()),
            timeout_secs: Some(30),
            default_install_dir: Some(PathBuf::from("~/.workbuddy/skills")),
            fallback_to_cli: Some(true),
            max_retries: Some(3),
        }
    }
}

/// SkillHub 技能条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillHubSkill {
    /// 技能名称
    pub name: String,
    /// 技能描述
    pub description: String,
    /// 版本
    pub version: String,
    /// 作者
    pub author: Option<String>,
    /// 下载地址
    pub download_url: String,
    /// 原始来源 (ClawHub URL)
    pub source_url: Option<String>,
    /// 下载量
    pub downloads: Option<u64>,
    /// 评分
    pub rating: Option<f32>,
    /// 分类
    pub category: Option<String>,
    /// 标签
    pub tags: Vec<String>,
}

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillHubSearchResult {
    pub skills: Vec<SkillHubSkill>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
}

/// 精选榜单条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillHubFeaturedItem {
    pub rank: usize,
    pub skill: SkillHubSkill,
    pub reason: Option<String>,
    /// 是否官方认证
    pub verified: bool,
}

/// Skill 测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTestResult {
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
pub struct SkillInstallResult {
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

/// SkillHub 客户端
pub struct SkillHubClient {
    config: SkillHubConfig,
    client: Client,
}

impl SkillHubClient {
    /// 创建新的 SkillHub 客户端
    pub fn new(config: SkillHubConfig) -> Self {
        let timeout = config.timeout_secs.unwrap_or(30);
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(timeout))
            .build()
            .unwrap_or_default();
        
        Self { config, client }
    }

    /// 使用默认配置创建客户端
    pub fn default_config() -> Self {
        Self::new(SkillHubConfig::default())
    }

    /// 检查 CLI 是否已安装
    pub fn is_cli_installed() -> bool {
        which::which("skillhub").is_ok()
    }

    /// 通过 CLI 搜索技能
    pub async fn search_via_cli(&self, query: &str) -> Result<Vec<SkillHubSkill>> {
        info!("Searching SkillHub for: {}", query);
        
        let output = Command::new("skillhub")
            .arg("search")
            .arg(query)
            .output()
            .context("Failed to execute skillhub search command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("SkillHub search failed: {}", stderr);
            anyhow::bail!("SkillHub search failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // 解析 CLI 输出 (JSON 格式)
        let result: SkillHubSearchResult = serde_json::from_str(&stdout)
            .context("Failed to parse SkillHub CLI output")?;
        
        Ok(result.skills)
    }

    /// 通过 CLI 安装技能
    pub async fn install_via_cli(&self, skill_name: &str, target_dir: &Path) -> Result<()> {
        info!("Installing skill '{}' from SkillHub to {:?}", skill_name, target_dir);
        
        // 确保目标目录存在
        if !target_dir.exists() {
            fs::create_dir_all(target_dir).await
                .context("Failed to create target directory")?;
        }

        let output = Command::new("skillhub")
            .arg("install")
            .arg(skill_name)
            .current_dir(target_dir)
            .output()
            .context("Failed to execute skillhub install command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("SkillHub install failed: {}", stderr);
            anyhow::bail!("SkillHub install failed: {}", stderr);
        }

        info!("Successfully installed skill '{}'", skill_name);
        Ok(())
    }

    /// 通过 API 搜索技能 (备用方案，不依赖 CLI)
    pub async fn search_via_api(&self, query: &str, page: usize, page_size: usize) -> Result<SkillHubSearchResult> {
        let api_base = self.config.api_base.as_deref()
            .unwrap_or("https://skillhub.tencent.com/api/v1");
        let url = format!("{}/skills/search?q={}&page={}&page_size={}", api_base, urlencoding::encode(query), page, page_size);
        
        debug!("Fetching: {}", url);
        
        let resp = self.client.get(&url).send().await
            .context("Failed to send request to SkillHub API")?;
        
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("SkillHub API returned status: {}, body: {}", status, body);
        }
        
        let result: SkillHubSearchResult = resp.json().await
            .context("Failed to parse SkillHub API response")?;
        
        Ok(result)
    }

    /// 获取精选榜单
    pub async fn get_featured(&self) -> Result<Vec<SkillHubFeaturedItem>> {
        let api_base = self.config.api_base.as_deref()
            .unwrap_or("https://skillhub.tencent.com/api/v1");
        let url = format!("{}/skills/featured", api_base);
        
        debug!("Fetching featured skills from: {}", url);
        
        let resp = self.client.get(&url).send().await
            .context("Failed to send request to SkillHub API")?;
        
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("SkillHub API returned status: {}, body: {}", status, body);
        }
        
        let result: Vec<SkillHubFeaturedItem> = resp.json().await
            .context("Failed to parse SkillHub API response")?;
        
        Ok(result)
    }

    /// 获取分类列表
    pub async fn get_categories(&self) -> Result<Vec<String>> {
        let api_base = self.config.api_base.as_deref()
            .unwrap_or("https://skillhub.tencent.com/api/v1");
        let url = format!("{}/skills/categories", api_base);
        
        debug!("Fetching categories from: {}", url);
        
        let resp = self.client.get(&url).send().await
            .context("Failed to send request to SkillHub API")?;
        
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("SkillHub API returned status: {}, body: {}", status, body);
        }
        
        let result: Vec<String> = resp.json().await
            .context("Failed to parse SkillHub API response")?;
        
        Ok(result)
    }

    /// 获取技能详情
    pub async fn get_skill_detail(&self, skill_name: &str) -> Result<SkillHubSkill> {
        let api_base = self.config.api_base.as_deref()
            .unwrap_or("https://skillhub.tencent.com/api/v1");
        let url = format!("{}/skills/{}", api_base, urlencoding::encode(skill_name));
        
        debug!("Fetching skill detail: {}", url);
        
        let resp = self.client.get(&url).send().await
            .context("Failed to send request to SkillHub API")?;
        
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("SkillHub API returned status: {}, body: {}", status, body);
        }
        
        let result: SkillHubSkill = resp.json().await
            .context("Failed to parse SkillHub API response")?;
        
        Ok(result)
    }

    /// 通过 API 安装 (直接下载并解压)
    pub async fn install_via_api(&self, skill_name: &str, target_dir: &Path) -> Result<SkillInstallResult> {
        // 首先获取技能详情
        let detail = match self.get_skill_detail(skill_name).await {
            Ok(d) => d,
            Err(e) => {
                // 尝试从搜索结果获取
                let search_result = self.search_via_api(skill_name, 1, 10).await?;
                if let Some(skill) = search_result.skills.into_iter().find(|s| s.name == skill_name) {
                    skill
                } else {
                    return Ok(SkillInstallResult {
                        success: false,
                        skill_name: skill_name.to_string(),
                        install_path: target_dir.join(skill_name),
                        message: format!("Skill '{}' not found", skill_name),
                        warnings: vec![e.to_string()],
                    });
                }
            }
        };
        
        let download_url = if !detail.download_url.is_empty() {
            detail.download_url.clone()
        } else {
            // 构造默认下载 URL
            let mirror = self.config.mirror_url.as_deref()
                .unwrap_or("https://skillhub-1388575217.cos.ap-guangzhou.myqcloud.com");
            format!("{}/skills/{}/{}.tar.gz", mirror, detail.name, detail.version)
        };
        
        info!("Downloading from: {}", download_url);
        
        // 确保目标目录存在
        if !target_dir.exists() {
            fs::create_dir_all(target_dir).await
                .context("Failed to create target directory")?;
        }
        
        let target_path = target_dir.join(format!("{}.tar.gz", skill_name));
        
        // 下载并解压
        let response = self.client.get(&download_url).send().await
            .context("Failed to download skill package")?;
        
        if !response.status().is_success() {
            return Ok(SkillInstallResult {
                success: false,
                skill_name: skill_name.to_string(),
                install_path: target_dir.join(skill_name),
                message: format!("Download failed: {}", response.status()),
                warnings: vec![],
            });
        }
        
        let bytes = response.bytes().await
            .context("Failed to read response body")?;
        
        fs::write(&target_path, &bytes).await
            .context("Failed to write skill package")?;
        
        // 解压
        let output = Command::new("tar")
            .args(["-xzf", &target_path.to_string_lossy()])
            .current_dir(target_dir)
            .output()
            .context("Failed to extract skill package")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // 尝试使用其他解压方式
            let zip_output = Command::new("tar")
                .args(["-xzf", &target_path.to_string_lossy()])
                .current_dir(target_dir)
                .output();
            
            if let Err(e) = zip_output {
                return Ok(SkillInstallResult {
                    success: false,
                    skill_name: skill_name.to_string(),
                    install_path: target_dir.join(skill_name),
                    message: format!("Failed to extract: {}", stderr),
                    warnings: vec![e.to_string()],
                });
            }
        }
        
        // 清理压缩包
        fs::remove_file(&target_path).await.ok();
        
        info!("Successfully installed skill '{}' via API", skill_name);
        
        Ok(SkillInstallResult {
            success: true,
            skill_name: skill_name.to_string(),
            install_path: target_dir.join(skill_name),
            message: format!("Successfully installed '{}'", skill_name),
            warnings: vec![],
        })
    }

    /// 安装技能 (优先使用 CLI，失败时回退到 API)
    pub async fn install(&self, skill_name: &str, target_dir: &Path) -> Result<SkillInstallResult> {
        let fallback_to_cli = self.config.fallback_to_cli.unwrap_or(true);
        
        // 首先尝试使用 CLI
        if fallback_to_cli && Self::is_cli_installed() {
            match self.install_via_cli(skill_name, target_dir).await {
                Ok(_) => {
                    return Ok(SkillInstallResult {
                        success: true,
                        skill_name: skill_name.to_string(),
                        install_path: target_dir.join(skill_name),
                        message: format!("Successfully installed '{}' via CLI", skill_name),
                        warnings: vec![],
                    });
                }
                Err(e) => {
                    warn!("CLI install failed, falling back to API: {}", e);
                }
            }
        }
        
        // CLI 不可用或失败，尝试 API 方式
        warn!("SkillHub CLI not found or install failed, falling back to API-based install");
        
        self.install_via_api(skill_name, target_dir).await
    }

    /// 批量安装多个技能
    pub async fn install_batch(&self, skill_names: &[String], target_dir: &Path) -> Result<Vec<SkillInstallResult>> {
        let mut results = Vec::new();
        
        for name in skill_names {
            let result = self.install(name, target_dir).await
                .unwrap_or_else(|e| SkillInstallResult {
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

    /// 测试已安装的 skill
    pub async fn test_skill(&self, skill_path: &Path) -> Result<Vec<SkillTestResult>> {
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
            
            // 基本验证: 尝试解析 SKILL.md
            let content = fs::read_to_string(&skill_md).await?;
            
            // 检查必要字段
            let has_title = content.contains("# ");
            let has_description = content.to_lowercase().contains("description");
            
            results.push(SkillTestResult {
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

    /// 验证 skill 结构 (增强版)
    pub async fn validate_skill(&self, skill_path: &Path) -> Result<bool> {
        let mut errors = Vec::new();
        
        // 检查必需文件
        let required_files = vec!["SKILL.md"];
        for file in required_files {
            let file_path = skill_path.join(file);
            if !file_path.exists() {
                errors.push(format!("Missing required file: {}", file));
            }
        }
        
        // 检查 SKILL.md 格式
        if let Ok(content) = fs::read_to_string(skill_path.join("SKILL.md")).await {
            if !content.contains("# ") {
                errors.push("SKILL.md must start with a title".to_string());
            }
            // 检查 description 部分
            if !content.to_lowercase().contains("description") && !content.to_lowercase().contains("描述") {
                warn!("SKILL.md might be missing description section");
            }
        }
        
        // 检查可选的配置文件
        if skill_path.join("skill.yaml").exists() {
            if let Ok(content) = fs::read_to_string(skill_path.join("skill.yaml")).await {
                // 验证 YAML 格式
                if let Err(e) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
                    errors.push(format!("Invalid skill.yaml: {}", e));
                }
            }
        }
        
        // 检查 scripts 目录
        let scripts_dir = skill_path.join("scripts");
        if scripts_dir.exists() {
            if let Ok(entries) = fs::read_dir(&scripts_dir).await {
                let mut has_executable = false;
                let mut entries = entries;
                while let Some(entry) = entries.next_entry().await? {
                    let path = entry.path();
                    if path.extension().map_or(false, |ext| ext == "py" || ext == "sh") {
                        has_executable = true;
                        break;
                    }
                }
                if has_executable {
                    info!("Found executable scripts in skill");
                }
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

    /// 获取 skill 的完整信息 (本地)
    pub async fn get_local_skill_info(&self, skill_path: &Path) -> Result<Option<SkillHubSkill>> {
        let skill_md = skill_path.join("SKILL.md");
        
        if !skill_md.exists() {
            return Ok(None);
        }
        
        let content = fs::read_to_string(&skill_md).await?;
        
        // 解析 SKILL.md
        let name = skill_path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        
        // 提取标题后的第一行作为描述
        let description = content.lines()
            .skip_while(|l| !l.starts_with("# "))
            .nth(1)
            .map(|l| l.trim().to_string())
            .unwrap_or_default();
        
        // 尝试从 skill.yaml 获取版本
        let version = if let Ok(yaml_content) = fs::read_to_string(skill_path.join("skill.yaml")).await {
            if let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(&yaml_content) {
                value.get("version")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            } else {
                "unknown".to_string()
            }
        } else {
            "unknown".to_string()
        };
        
        Ok(Some(SkillHubSkill {
            name,
            description,
            version,
            author: None,
            download_url: String::new(),
            source_url: None,
            downloads: None,
            rating: None,
            category: None,
            tags: vec![],
        }))
    }

    /// 检查 skill 是否有可用更新
    pub async fn check_for_updates(&self, skill_path: &Path) -> Result<Option<UpdateInfo>> {
        let local_info = self.get_local_skill_info(skill_path).await?;
        
        if let Some(local) = local_info {
            // 尝试获取远程版本
            if let Ok(remote) = self.get_skill_detail(&local.name).await {
                let local_version = semver::Version::parse(&local.version).ok();
                let remote_version = semver::Version::parse(&remote.version).ok();
                
                if let (Some(local_v), Some(remote_v)) = (local_version, remote_version) {
                    if remote_v > local_v {
                        return Ok(Some(UpdateInfo {
                            skill_name: local.name.clone(),
                            current_version: local.version,
                            latest_version: remote.version,
                            release_notes: remote.description,
                            download_url: remote.download_url,
                        }));
                    }
                }
            }
        }
        
        Ok(None)
    }

    /// 导出 skill 为压缩包
    pub async fn export_skill(&self, skill_path: &Path, output_dir: &Path) -> Result<PathBuf> {
        let skill_name = skill_path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "skill".to_string());
        
        let output_path = output_dir.join(format!("{}.tar.gz", skill_name));
        
        // 使用 tar 打包
        let output = Command::new("tar")
            .args([
                "-czf",
                &output_path.to_string_lossy(),
                "-C",
                &skill_path.parent().unwrap_or(skill_path).to_string_lossy(),
                &skill_name,
            ])
            .output()
            .context("Failed to create skill archive")?;
        
        if !output.status.success() {
            anyhow::bail!("Failed to create archive: {}", String::from_utf8_lossy(&output.stderr));
        }
        
        Ok(output_path)
    }

    /// 列出推荐的 skills
    pub async fn get_recommended(&self) -> Result<Vec<SkillHubSkill>> {
        // 获取精选榜单作为推荐
        let featured = self.get_featured().await?;
        
        Ok(featured.into_iter().map(|f| f.skill).collect())
    }

    /// 按分类获取 skills
    pub async fn get_skills_by_category(&self, category: &str) -> Result<Vec<SkillHubSkill>> {
        let result = self.search_via_api(category, 1, 20).await?;
        Ok(result.skills)
    }

    /// 获取默认安装目录
    pub fn get_default_install_dir(&self) -> PathBuf {
        self.config.default_install_dir.clone()
            .unwrap_or_else(|| PathBuf::from("~/.workbuddy/skills"))
            .expand_user().unwrap_or_else(|_| PathBuf::from("./skills"))
    }

    /// 列出已安装的 skills
    pub async fn list_installed(&self, install_dir: &Path) -> Result<Vec<String>> {
        let mut skills = Vec::new();
        
        if !install_dir.exists() {
            return Ok(skills);
        }
        
        let mut entries = fs::read_dir(install_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                // 检查是否包含 SKILL.md
                if path.join("SKILL.md").exists() {
                    if let Some(name) = path.file_name() {
                        skills.push(name.to_string_lossy().to_string());
                    }
                }
            }
        }
        
        Ok(skills)
    }

    /// 卸载 skill
    pub async fn uninstall(&self, skill_name: &str, install_dir: &Path) -> Result<()> {
        let skill_path = install_dir.join(skill_name);
        
        if !skill_path.exists() {
            anyhow::bail!("Skill '{}' not found at {:?}", skill_name, skill_path);
        }
        
        // 如果是 CLI 安装的，尝试使用 CLI 卸载
        if Self::is_cli_installed() {
            let output = Command::new("skillhub")
                .arg("uninstall")
                .arg(skill_name)
                .output();
            
            if let Ok(output) = output {
                if output.status.success() {
                    info!("Successfully uninstalled '{}' via CLI", skill_name);
                    return Ok(());
                }
            }
        }
        
        // 回退到直接删除目录
        fs::remove_dir_all(&skill_path).await
            .context("Failed to remove skill directory")?;
        
        info!("Successfully uninstalled '{}'", skill_name);
        Ok(())
    }

    /// 运行单个测试文件
    async fn run_test_file(&self, path: &Path, test_name: &str) -> Result<SkillTestResult> {
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
                // 尝试使用 ts-node 或 deno
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
                return Ok(SkillTestResult {
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
                
                Ok(SkillTestResult {
                    passed: output.status.success(),
                    test_name: test_name.to_string(),
                    output: stdout,
                    error: if output.status.success() { None } else { Some(stderr) },
                    duration_ms,
                })
            },
            Err(e) => {
                Ok(SkillTestResult {
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

/// 更新信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub skill_name: String,
    pub current_version: String,
    pub latest_version: String,
    pub release_notes: String,
    pub download_url: String,
}

// 辅助函数: 展开用户路径
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
    fn test_skillhub_config_default() {
        let config = SkillHubConfig::default();
        assert!(config.mirror_url.is_some());
        assert!(config.api_base.is_some());
    }

    #[test]
    fn test_skillhub_skill_serde() {
        let json = r#"{
            "name": "test-skill",
            "description": "A test skill",
            "version": "1.0.0",
            "author": "tester",
            "download_url": "https://example.com/test.tar.gz",
            "tags": ["test", "example"]
        }"#;
        
        let skill: SkillHubSkill = serde_json::from_str(json).unwrap();
        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.version, "1.0.0");
        assert_eq!(skill.tags.len(), 2);
    }
}