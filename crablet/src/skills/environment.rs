//! 技能执行环境管理
//!
//! 提供依赖隔离和虚拟环境管理，确保技能依赖不污染系统环境。

use anyhow::{Result, Context, anyhow};
use tracing::{info, warn};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::fs;
use serde::{Deserialize, Serialize};

/// 虚拟环境类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnvironmentType {
    /// Python 虚拟环境
    Python {
        python_path: PathBuf,
        pip_path: PathBuf,
    },
    /// Node.js 环境
    Node {
        node_path: PathBuf,
        npm_path: PathBuf,
    },
    /// 容器环境
    Container {
        image: String,
    },
}

/// 技能执行环境
pub struct SkillEnvironment {
    pub env_type: EnvironmentType,
    pub skill_dir: PathBuf,
    pub env_dir: PathBuf,
}

impl SkillEnvironment {
    /// 为技能创建隔离环境
    pub async fn create(skill_dir: &Path, runtime: Option<&str>) -> Result<Self> {
        let env_dir = skill_dir.join(".env");
        
        let env_type = match runtime {
            Some("python3") | Some("python") => {
                Self::create_python_env(skill_dir, &env_dir).await?
            }
            Some("node") | Some("nodejs") => {
                Self::create_node_env(skill_dir, &env_dir).await?
            }
            Some("docker") => {
                EnvironmentType::Container {
                    image: "alpine:latest".to_string(),
                }
            }
            _ => {
                // 默认尝试自动检测
                if skill_dir.join("requirements.txt").exists() {
                    Self::create_python_env(skill_dir, &env_dir).await?
                } else if skill_dir.join("package.json").exists() {
                    Self::create_node_env(skill_dir, &env_dir).await?
                } else {
                    return Err(anyhow!("Cannot determine runtime environment"));
                }
            }
        };

        Ok(Self {
            env_type,
            skill_dir: skill_dir.to_path_buf(),
            env_dir,
        })
    }

    /// 创建 Python 虚拟环境
    async fn create_python_env(skill_dir: &Path, env_dir: &Path) -> Result<EnvironmentType> {
        info!("Creating Python virtual environment at {:?}", env_dir);

        // 检查 Python 是否可用
        let python_check = Command::new("which")
            .arg("python3")
            .output()
            .await;

        if python_check.is_err() || !python_check.unwrap().status.success() {
            return Err(anyhow!("Python3 is not installed or not in PATH"));
        }

        // 创建虚拟环境
        if !env_dir.exists() {
            let output = Command::new("python3")
                .args(&["-m", "venv", env_dir.to_str().unwrap()])
                .current_dir(skill_dir)
                .output()
                .await
                .context("Failed to create Python virtual environment")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("Failed to create venv: {}", stderr));
            }
        }

        // 确定路径
        let (python_path, pip_path) = if cfg!(windows) {
            (
                env_dir.join("Scripts").join("python.exe"),
                env_dir.join("Scripts").join("pip.exe"),
            )
        } else {
            (
                env_dir.join("bin").join("python"),
                env_dir.join("bin").join("pip"),
            )
        };

        // 安装依赖
        let requirements_file = skill_dir.join("requirements.txt");
        if requirements_file.exists() {
            info!("Installing Python dependencies from requirements.txt");
            let output = Command::new(&pip_path)
                .args(&["install", "-r", "requirements.txt"])
                .current_dir(skill_dir)
                .output()
                .await
                .context("Failed to install pip dependencies")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to install some dependencies: {}", stderr);
            }
        }

        Ok(EnvironmentType::Python { python_path, pip_path })
    }

    /// 创建 Node.js 环境
    async fn create_node_env(skill_dir: &Path, env_dir: &Path) -> Result<EnvironmentType> {
        info!("Setting up Node.js environment at {:?}", env_dir);

        // 检查 Node 是否可用
        let node_check = Command::new("which")
            .arg("node")
            .output()
            .await;

        if node_check.is_err() || !node_check.unwrap().status.success() {
            return Err(anyhow!("Node.js is not installed or not in PATH"));
        }

        // 获取 node 和 npm 路径
        let node_path = PathBuf::from("node");
        let npm_path = PathBuf::from("npm");

        // 安装依赖
        let package_json = skill_dir.join("package.json");
        if package_json.exists() {
            info!("Installing Node.js dependencies");
            let output = Command::new(&npm_path)
                .arg("install")
                .current_dir(skill_dir)
                .output()
                .await
                .context("Failed to install npm dependencies")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to install some dependencies: {}", stderr);
            }
        }

        Ok(EnvironmentType::Node { node_path, npm_path })
    }

    /// 安装额外依赖
    pub async fn install_dependencies(&self, dependencies: &SkillDependencies) -> Result<()> {
        match &self.env_type {
            EnvironmentType::Python { pip_path, .. } => {
                for dep in &dependencies.pip {
                    info!("Installing Python dependency: {}", dep);
                    let output = Command::new(pip_path)
                        .args(&["install", dep])
                        .output()
                        .await?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(anyhow!("Failed to install {}: {}", dep, stderr));
                    }
                }
            }
            EnvironmentType::Node { npm_path, .. } => {
                for dep in &dependencies.npm {
                    info!("Installing Node.js dependency: {}", dep);
                    let output = Command::new(npm_path)
                        .args(&["install", dep])
                        .current_dir(&self.skill_dir)
                        .output()
                        .await?;

                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        return Err(anyhow!("Failed to install {}: {}", dep, stderr));
                    }
                }
            }
            EnvironmentType::Container { .. } => {
                warn!("Container environment dependency installation not implemented");
            }
        }

        Ok(())
    }

    /// 执行技能
    pub async fn execute(&self, entrypoint: &str, args: serde_json::Value) -> Result<String> {
        let args_str = serde_json::to_string(&args)?;

        match &self.env_type {
            EnvironmentType::Python { python_path, .. } => {
                self.execute_python(python_path, entrypoint, &args_str).await
            }
            EnvironmentType::Node { node_path, .. } => {
                self.execute_node(node_path, entrypoint, &args_str).await
            }
            EnvironmentType::Container { image } => {
                self.execute_container(image, entrypoint, &args_str).await
            }
        }
    }

    /// 执行 Python 脚本
    async fn execute_python(
        &self,
        python_path: &Path,
        entrypoint: &str,
        args: &str,
    ) -> Result<String> {
        let script_path = self.skill_dir.join(entrypoint);
        
        let output = Command::new(python_path)
            .arg(&script_path)
            .arg(args)
            .current_dir(&self.skill_dir)
            .output()
            .await
            .context("Failed to execute Python script")?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Python execution failed: {}", stderr))
        }
    }

    /// 执行 Node.js 脚本
    async fn execute_node(
        &self,
        node_path: &Path,
        entrypoint: &str,
        args: &str,
    ) -> Result<String> {
        let script_path = self.skill_dir.join(entrypoint);
        
        let output = Command::new(node_path)
            .arg(&script_path)
            .arg(args)
            .current_dir(&self.skill_dir)
            .output()
            .await
            .context("Failed to execute Node.js script")?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Node.js execution failed: {}", stderr))
        }
    }

    /// 在容器中执行
    async fn execute_container(
        &self,
        image: &str,
        entrypoint: &str,
        args: &str,
    ) -> Result<String> {
        let output = Command::new("docker")
            .args(&[
                "run",
                "--rm",
                "-v",
                &format!("{}:/skill", self.skill_dir.to_string_lossy()),
                "-w",
                "/skill",
                image,
                entrypoint,
                args,
            ])
            .output()
            .await
            .context("Failed to execute in container")?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(anyhow!("Container execution failed: {}", stderr))
        }
    }

    /// 清理环境
    pub async fn cleanup(&self) -> Result<()> {
        if self.env_dir.exists() {
            info!("Cleaning up environment at {:?}", self.env_dir);
            fs::remove_dir_all(&self.env_dir).await?;
        }
        Ok(())
    }
}

/// 技能依赖
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillDependencies {
    #[serde(default)]
    pub pip: Vec<String>,
    #[serde(default)]
    pub npm: Vec<String>,
    #[serde(default)]
    pub system: Vec<String>,
}

/// 虚拟环境管理器
pub struct VirtualEnv;

impl VirtualEnv {
    /// 检查并安装系统依赖
    pub async fn check_system_dependencies(deps: &[String]) -> Result<()> {
        for dep in deps {
            info!("Checking system dependency: {}", dep);
            
            let check = Command::new("which")
                .arg(dep)
                .output()
                .await;

            match check {
                Ok(output) if output.status.success() => {
                    info!("System dependency '{}' is available", dep);
                }
                _ => {
                    return Err(anyhow!(
                        "Missing system dependency: '{}'. Please install it manually.",
                        dep
                    ));
                }
            }
        }

        Ok(())
    }

    /// 获取 Python 版本
    pub async fn get_python_version() -> Result<String> {
        let output = Command::new("python3")
            .arg("--version")
            .output()
            .await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(anyhow!("Failed to get Python version"))
        }
    }

    /// 获取 Node.js 版本
    pub async fn get_node_version() -> Result<String> {
        let output = Command::new("node")
            .arg("--version")
            .output()
            .await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(anyhow!("Failed to get Node.js version"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_dependencies_default() {
        let deps = SkillDependencies::default();
        assert!(deps.pip.is_empty());
        assert!(deps.npm.is_empty());
        assert!(deps.system.is_empty());
    }
}
