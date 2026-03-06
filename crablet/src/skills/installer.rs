use anyhow::{Result, Context};
use std::path::Path;
use tokio::process::Command;
use tracing::info;

pub struct SkillInstaller;

impl SkillInstaller {
    pub async fn install_from_git(url: &str, skills_dir: &Path) -> Result<()> {
        info!("Installing skill from {}", url);

        // check if git is installed
        if which::which("git").is_err() {
            return Err(anyhow::anyhow!("Git is not installed or not in PATH"));
        }

        // Parse name from URL (simple heuristic)
        let repo_name = url.split('/').next_back().unwrap_or("unknown_skill")
            .trim_end_matches(".git");
            
        let target_path = skills_dir.join(repo_name);
        
        if target_path.exists() {
            return Err(anyhow::anyhow!("Skill directory already exists: {:?}", target_path));
        }

        // git clone
        let status = Command::new("git")
            .arg("clone")
            .arg("--depth")
            .arg("1") // Shallow clone
            .arg(url)
            .arg(&target_path)
            .status()
            .await
            .context("Failed to execute git clone")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Git clone failed"));
        }

        info!("Skill installed to {:?}", target_path);
        
        // Validation: Check if it's a valid skill
        // We can use SkillRegistry logic, but for now just check for manifest
        let has_manifest = target_path.join("skill.yaml").exists() || 
                           target_path.join("skill.json").exists() ||
                           target_path.join("SKILL.md").exists();
                           
        if !has_manifest {
            // Rollback
            let _ = tokio::fs::remove_dir_all(&target_path).await;
            return Err(anyhow::anyhow!("Installed repository is not a valid Crablet skill (missing skill.yaml/json/SKILL.md)"));
        }

        Ok(())
    }
}
