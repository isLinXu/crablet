use super::{SkillManifest, SkillRegistry};
use anyhow::{anyhow, Result};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillDependencies {
    #[serde(default)]
    pub pip: Vec<String>,
    #[serde(default)]
    pub npm: Vec<String>,
}

#[derive(Debug, PartialEq)]
pub enum DependencyStatus {
    Satisfied,
    Missing,
    Outdated,
    Conflict,
}

pub struct VirtualEnvironment {
    pub path: PathBuf,
    pub env_type: EnvironmentType,
}

#[derive(Debug, Clone)]
pub enum EnvironmentType {
    Python { python_path: PathBuf },
    Node { node_path: PathBuf },
}

impl VirtualEnvironment {
    pub async fn create_python_env(base_path: &Path) -> Result<Self> {
        let env_path = base_path.join(".venv");
        if !env_path.exists() {
            info!("Creating Python virtual environment at {:?}", env_path);
            let status = Command::new("python3")
                .args(["-m", "venv"])
                .arg(&env_path)
                .status()
                .await?;
            if !status.success() {
                return Err(anyhow!("Failed to create virtual environment"));
            }
        }

        let python_path = if cfg!(windows) {
            env_path.join("Scripts").join("python.exe")
        } else {
            env_path.join("bin").join("python")
        };

        Ok(Self {
            path: env_path,
            env_type: EnvironmentType::Python { python_path },
        })
    }

    pub async fn install_dependencies(&self, dependencies: &[String]) -> Result<()> {
        match &self.env_type {
            EnvironmentType::Python { python_path } => {
                for dep in dependencies {
                    info!("Installing Python dependency: {}", dep);
                    let status = Command::new(python_path)
                        .args(["-m", "pip", "install", dep])
                        .status()
                        .await?;
                    if !status.success() {
                        return Err(anyhow!("Failed to install dependency: {}", dep));
                    }
                }
            }
            EnvironmentType::Node { .. } => {
                // npm install implementation:
                // 1. Check for package.json in the skill directory
                // 2. Run `npm install --production` via Command
                // 3. Verify node_modules exists after install
                // Currently logs a warning; npm support will be added
                // when the Node.js skill runtime is fully integrated.
                warn!("npm installation not yet fully implemented");
            }
        }
        Ok(())
    }
}

pub async fn check_dependencies(registry: &SkillRegistry, manifest: &SkillManifest) -> Result<()> {
    // 1. Check Crablet version using semver
    if let Some(min_ver_str) = &manifest.min_crablet_version {
        let current_ver_str = env!("CARGO_PKG_VERSION");
        let current_ver = Version::parse(current_ver_str)?;

        // Treat min_ver as a VersionReq (e.g., ">=0.1.0")
        let req_str = if min_ver_str.starts_with(['>', '<', '=']) {
            min_ver_str.clone()
        } else {
            format!(">={}", min_ver_str)
        };

        let req = VersionReq::parse(&req_str)?;
        if !req.matches(&current_ver) {
            return Err(anyhow!(
                "Skill requires Crablet version {}, but current is {}",
                min_ver_str,
                current_ver_str
            ));
        }
    }

    // 2. Check conflicts
    for conflict in &manifest.conflicts {
        if registry.skills.contains_key(conflict) {
            return Err(anyhow!("Skill conflicts with existing skill: {}", conflict));
        }
    }

    // 3. Check system dependencies (requires)
    for req in &manifest.requires {
        if which::which(req).is_err() {
            return Err(anyhow!("Missing system dependency: {}", req));
        }
    }

    // 4. Check package dependencies (pip)
    if let Some(deps) = &manifest.dependencies {
        for pkg in &deps.pip {
            let pkg_name = pkg
                .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                .next()
                .unwrap_or(pkg);

            let status = Command::new("pip")
                .args(["show", pkg_name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;

            if let Ok(s) = status {
                if !s.success() {
                    warn!("Missing pip dependency: {}. Attempting to install...", pkg);
                    // For now, we auto-install, but in a real app we might want to ask or use venv
                    let install_status =
                        Command::new("pip").args(["install", pkg]).status().await?;

                    if !install_status.success() {
                        return Err(anyhow!("Failed to install pip dependency: {}", pkg));
                    }
                }
            }
        }

        // 5. Check npm dependencies (placeholder)
        for pkg in &deps.npm {
            debug!("Checking npm dependency: {}", pkg);
            // Basic check if npm is available
            if which::which("npm").is_err() {
                return Err(anyhow!("npm is required but not found in PATH"));
            }
            // npm package check: verify the package is resolvable
            // Real implementation would run `npm ls <pkg>` and parse
            // the output to confirm the package is installed and at
            // a compatible version. For now, we only verify npm is available.
        }
    }

    Ok(())
}
