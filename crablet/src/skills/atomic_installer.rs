//! 原子性技能安装器
//! 
//! 提供原子性安装、回滚和验证机制，确保安装过程的一致性。

use anyhow::{Result, Context, anyhow};
use tracing::{info, warn, error};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::process::Command;
use uuid::Uuid;

/// 安装事务
pub struct InstallTransaction {
    pub transaction_id: String,
    pub temp_dir: PathBuf,
    pub target_dir: PathBuf,
    pub source_url: String,
    pub state: TransactionState,
}

#[derive(Debug, Clone)]
pub enum TransactionState {
    Pending,
    Downloading,
    Validating,
    Installing,
    Completed,
    RolledBack,
    Failed(String),
}

/// 原子性安装器
pub struct AtomicInstaller;

impl AtomicInstaller {
    /// 从 Git URL 原子性安装技能
    pub async fn install_from_git(
        url: &str,
        skills_dir: &Path,
        name_override: Option<&str>,
    ) -> Result<InstallResult> {
        let transaction_id = Uuid::new_v4().to_string();
        info!("[{}] Starting atomic installation from {}", transaction_id, url);

        // 1. 解析技能名称
        let skill_name = name_override.map(|s| s.to_string()).unwrap_or_else(|| {
            url.split('/')
                .next_back()
                .unwrap_or("unknown_skill")
                .trim_end_matches(".git")
                .to_string()
        });

        let target_dir = skills_dir.join(&skill_name);
        
        // 2. 检查目标目录是否已存在
        if target_dir.exists() {
            return Err(anyhow!(
                "Skill '{}' already exists at {:?}. Use 'update' to upgrade.",
                skill_name,
                target_dir
            ));
        }

        // 3. 创建临时目录
        let temp_dir = Self::create_temp_dir(&transaction_id).await?;
        let _temp_skill_dir = temp_dir.join(&skill_name);

        // 4. 执行安装事务
        let transaction = InstallTransaction {
            transaction_id: transaction_id.clone(),
            temp_dir: temp_dir.clone(),
            target_dir: target_dir.clone(),
            source_url: url.to_string(),
            state: TransactionState::Pending,
        };

        match Self::execute_transaction(transaction).await {
            Ok(result) => {
                info!("[{}] Installation completed successfully", transaction_id);
                Ok(result)
            }
            Err(e) => {
                error!("[{}] Installation failed: {}", transaction_id, e);
                // 尝试清理
                let _ = fs::remove_dir_all(&temp_dir).await;
                Err(e)
            }
        }
    }

    /// 执行安装事务
    async fn execute_transaction(mut transaction: InstallTransaction) -> Result<InstallResult> {
        let tx_id = &transaction.transaction_id;

        // Phase 1: 下载
        transaction.state = TransactionState::Downloading;
        info!("[{}] Phase 1: Downloading...", tx_id);
        
        Self::download_skill(
            &transaction.source_url,
            &transaction.temp_dir,
        ).await?;

        // Phase 2: 验证
        transaction.state = TransactionState::Validating;
        info!("[{}] Phase 2: Validating...", tx_id);
        
        let validation = Self::validate_skill(&transaction.temp_dir).await?;
        if !validation.is_valid {
            return Err(anyhow!(
                "Skill validation failed: {}",
                validation.errors.join(", ")
            ));
        }

        // Phase 3: 原子性安装（移动）
        transaction.state = TransactionState::Installing;
        info!("[{}] Phase 3: Installing...", tx_id);
        
        // 找到实际的技能目录（可能在 temp_dir/skill_name/ 下）
        let actual_skill_dir = Self::find_skill_dir(&transaction.temp_dir).await?;
        
        // 原子性移动
        match Self::atomic_move(&actual_skill_dir, &transaction.target_dir).await {
            Ok(_) => {
                transaction.state = TransactionState::Completed;
                
                // 清理临时目录
                let _ = fs::remove_dir_all(&transaction.temp_dir).await;
                
                Ok(InstallResult {
                    skill_name: validation.manifest.as_ref().map(|m| m.name.clone()).unwrap_or_default(),
                    version: validation.manifest.as_ref().map(|m| m.version.clone()).unwrap_or_default(),
                    install_path: transaction.target_dir,
                    manifest: validation.manifest,
                    transaction_id: tx_id.clone(),
                    dependencies_installed: vec![],
                    environment_created: false,
                    signature_valid: false,
                })
            }
            Err(e) => {
                transaction.state = TransactionState::Failed(e.to_string());
                
                // 尝试回滚
                let _ = Self::rollback(&transaction).await;
                
                Err(anyhow!("Atomic move failed: {}", e))
            }
        }
    }

    /// 创建临时目录
    async fn create_temp_dir(transaction_id: &str) -> Result<PathBuf> {
        let temp_base = std::env::temp_dir().join("crablet_skill_install");
        let temp_dir = temp_base.join(format!("{}_{}", transaction_id, chrono::Utc::now().timestamp()));
        
        fs::create_dir_all(&temp_dir)
            .await
            .context("Failed to create temporary directory")?;
        
        Ok(temp_dir)
    }

    /// 下载技能
    async fn download_skill(url: &str, temp_dir: &Path) -> Result<()> {
        // 检查 git 是否可用
        let git_check = Command::new("which")
            .arg("git")
            .output()
            .await;
        
        if git_check.is_err() || !git_check.unwrap().status.success() {
            return Err(anyhow!("Git is not installed or not in PATH"));
        }

        // 执行浅克隆
        let output = Command::new("git")
            .arg("clone")
            .arg("--depth")
            .arg("1")
            .arg("--single-branch")
            .arg(url)
            .arg(temp_dir)
            .output()
            .await
            .context("Failed to execute git clone")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Git clone failed: {}", stderr));
        }

        info!("Successfully cloned from {}", url);
        Ok(())
    }

    /// 验证技能
    async fn validate_skill(temp_dir: &Path) -> Result<ValidationResult> {
        let mut errors = Vec::new();
        let mut manifest = None;

        // 检查可能的 manifest 文件
        let possible_manifests = [
            "skill.yaml",
            "skill.yml",
            "skill.json",
            "SKILL.md",
        ];

        let skill_dir = Self::find_skill_dir(temp_dir).await?;
        let mut found_manifest = false;

        for manifest_name in &possible_manifests {
            let manifest_path = skill_dir.join(manifest_name);
            if manifest_path.exists() {
                found_manifest = true;
                
                // 尝试解析 manifest
                match Self::parse_manifest(&manifest_path).await {
                    Ok(m) => {
                        manifest = Some(m);
                    }
                    Err(e) => {
                        errors.push(format!("Failed to parse {}: {}", manifest_name, e));
                    }
                }
                break;
            }
        }

        if !found_manifest {
            errors.push("No valid manifest found (skill.yaml/skill.json/SKILL.md)".to_string());
        }

        // 如果有 manifest，进行额外验证
        if let Some(ref m) = manifest {
            // 验证名称
            if m.name.is_empty() {
                errors.push("Skill name is empty".to_string());
            }

            // 验证版本格式
            if !Self::is_valid_version(&m.version) {
                errors.push(format!("Invalid version format: {}", m.version));
            }

            // 验证入口点（对于非 OpenClaw 技能）
            if m.entrypoint != "openclaw" && !skill_dir.join(&m.entrypoint).exists() {
                // 检查是否是相对路径
                let entrypoint_path = skill_dir.join(&m.entrypoint.split_whitespace().next().unwrap_or(""));
                if !entrypoint_path.exists() {
                    warn!("Entrypoint '{}' not found in skill directory", m.entrypoint);
                }
            }
        }

        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            manifest,
        })
    }

    /// 解析 manifest
    async fn parse_manifest(path: &Path) -> Result<super::SkillManifest> {
        let content = fs::read_to_string(path).await?;
        
        let manifest = if path.extension().and_then(|s| s.to_str()) == Some("json") {
            serde_json::from_str(&content)?
        } else if path.file_name().and_then(|s| s.to_str()) == Some("SKILL.md") {
            // OpenClaw 格式，使用专门的解析器
            super::openclaw::OpenClawSkillLoader::load(path).await?.manifest
        } else {
            serde_yaml::from_str(&content)?
        };
        
        Ok(manifest)
    }

    /// 查找技能目录
    async fn find_skill_dir(temp_dir: &Path) -> Result<PathBuf> {
        // 如果 temp_dir 直接包含 manifest，返回 temp_dir
        for manifest in &["skill.yaml", "skill.yml", "skill.json", "SKILL.md"] {
            if temp_dir.join(manifest).exists() {
                return Ok(temp_dir.to_path_buf());
            }
        }

        // 否则查找子目录
        let mut entries = fs::read_dir(temp_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                for manifest in &["skill.yaml", "skill.yml", "skill.json", "SKILL.md"] {
                    if path.join(manifest).exists() {
                        return Ok(path);
                    }
                }
            }
        }

        // 如果没找到，返回 temp_dir 本身
        Ok(temp_dir.to_path_buf())
    }

    /// 原子性移动
    async fn atomic_move(source: &Path, target: &Path) -> Result<()> {
        // 确保父目录存在
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).await?;
        }

        // 使用 tokio 的异步重命名（在 Unix 上是原子的）
        fs::rename(source, target)
            .await
            .context("Failed to move skill directory atomically")?;

        info!("Atomically moved {:?} to {:?}", source, target);
        Ok(())
    }

    /// 回滚事务
    async fn rollback(transaction: &InstallTransaction) -> Result<()> {
        warn!("[{}] Rolling back transaction...", transaction.transaction_id);

        // 清理临时目录
        if transaction.temp_dir.exists() {
            fs::remove_dir_all(&transaction.temp_dir).await?;
        }

        // 如果目标目录存在但安装未完成，也清理
        if transaction.target_dir.exists() {
            let metadata = fs::metadata(&transaction.target_dir).await?;
            if metadata.is_dir() {
                // 检查是否包含 .crablet_installing 标记文件
                if transaction.target_dir.join(".crablet_installing").exists() {
                    fs::remove_dir_all(&transaction.target_dir).await?;
                }
            }
        }

        info!("[{}] Rollback completed", transaction.transaction_id);
        Ok(())
    }

    /// 验证版本格式
    fn is_valid_version(version: &str) -> bool {
        // 简单的语义化版本检查
        version.split('.').count() >= 2 &&
        version.chars().all(|c| c.is_numeric() || c == '.' || c == '-' || c == '+')
    }
}

/// 安装结果
#[derive(Debug, Clone)]
pub struct InstallResult {
    pub skill_name: String,
    pub version: String,
    pub install_path: PathBuf,
    pub manifest: Option<super::SkillManifest>,
    pub transaction_id: String,
    pub dependencies_installed: Vec<String>,
    pub environment_created: bool,
    pub signature_valid: bool,
}

/// 验证结果
#[derive(Debug, Clone)]
struct ValidationResult {
    is_valid: bool,
    errors: Vec<String>,
    manifest: Option<super::SkillManifest>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_validation() {
        assert!(AtomicInstaller::is_valid_version("1.0.0"));
        assert!(AtomicInstaller::is_valid_version("2.1.3-beta"));
        assert!(AtomicInstaller::is_valid_version("1.0"));
        assert!(!AtomicInstaller::is_valid_version(""));
        assert!(!AtomicInstaller::is_valid_version("v1.0")); // 'v' 前缀不被允许
    }
}
