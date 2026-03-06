use anyhow::Result;
use std::process::Stdio;
use tokio::process::Command;
use tracing::warn;
use serde::{Deserialize, Serialize};
use super::{SkillRegistry, SkillManifest};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillDependencies {
    #[serde(default)]
    pub pip: Vec<String>,
    #[serde(default)]
    pub npm: Vec<String>,
}

pub async fn check_dependencies(registry: &SkillRegistry, manifest: &SkillManifest) -> Result<()> {
    // Check Crablet version
    if let Some(min_ver) = &manifest.min_crablet_version {
        let current_ver = env!("CARGO_PKG_VERSION");
        // Simple string comparison for now, or use semver crate if available
        // Assuming semver format "x.y.z"
        if min_ver.as_str() > current_ver {
            return Err(anyhow::anyhow!("Skill requires Crablet version >= {}, but current is {}", min_ver, current_ver));
        }
    }

    // Check conflicts
    for conflict in &manifest.conflicts {
        if registry.skills.contains_key(conflict) {
            return Err(anyhow::anyhow!("Skill conflicts with existing skill: {}", conflict));
        }
    }

    // Check system dependencies (requires)
    for req in &manifest.requires {
        // Check if command exists in PATH
        if which::which(req).is_err() {
            return Err(anyhow::anyhow!("Missing system dependency: {}", req));
        }
    }

    // Check package dependencies
    if let Some(deps) = &manifest.dependencies {
        for pkg in &deps.pip {
            // Simple parsing: take everything before first non-alphanumeric (except - and _)
            let pkg_name = pkg.split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_').next().unwrap_or(pkg);
            
            // Run pip show
            let status = Command::new("pip")
                .arg("show")
                .arg(pkg_name)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
                
            if let Ok(s) = status {
                if !s.success() {
                    warn!("Missing pip dependency: {}. Attempting to install...", pkg);
                    let install_status = Command::new("pip")
                        .arg("install")
                        .arg(pkg)
                        .status()
                        .await?;
                        
                    if !install_status.success() {
                         return Err(anyhow::anyhow!("Failed to install pip dependency: {}", pkg));
                    }
                }
            }
        }
    }

    Ok(())
}
