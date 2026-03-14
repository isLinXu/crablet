use anyhow::{Result, anyhow};
use std::process::Command;
use std::path::{Path, PathBuf};
use std::fs;
use tracing::info;
use serde_json::Value;
use async_trait::async_trait;

/// 工具管理器
pub struct ToolManager {
    // 工具列表
    tools: Vec<Box<dyn Tool>>,
}

/// 工具 trait
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(&self, command: &str, args: Value) -> Result<String>;
}

impl ToolManager {
    /// 创建新的工具管理器
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
        }
    }

    /// 添加工具
    pub fn add_tool(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    /// 获取工具列表
    pub fn list_tools(&self) -> &[Box<dyn Tool>] {
        &self.tools
    }

    /// 执行工具
    pub async fn execute(&self, name: &str, args: Value) -> Result<String> {
        for tool in &self.tools {
            if tool.name() == name {
                return tool.execute(name, args).await;
            }
        }
        Err(anyhow!("Tool not found: {}", name))
    }

    /// 检查是否为空
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolManager {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SkillManagerTool {
    skills_dir: PathBuf,
}

impl SkillManagerTool {
    pub fn new(skills_dir: &Path) -> Self {
        Self {
            skills_dir: skills_dir.to_path_buf(),
        }
    }

    /// Install a skill from a Git repository
    pub fn install_from_git(&self, url: &str, name: Option<&str>) -> Result<String> {
        // Determine directory name
        let dir_name = if let Some(n) = name {
            n.to_string()
        } else {
            url.split('/').next_back()
                .ok_or_else(|| anyhow!("Invalid URL"))?
                .trim_end_matches(".git")
                .to_string()
        };

        let target_path = self.skills_dir.join(&dir_name);

        if target_path.exists() {
            return Err(anyhow!("Skill directory already exists: {:?}", target_path));
        }

        info!("Cloning skill from {} to {:?}", url, target_path);

        let status = Command::new("git")
            .arg("clone")
            .arg(url)
            .arg(&target_path)
            .status()?;

        if status.success() {
            Ok(format!("Successfully installed skill '{}' to {:?}", dir_name, target_path))
        } else {
            Err(anyhow!("Failed to clone repository"))
        }
    }

    /// Create a new simple Python skill from scratch (Self-Evolution)
    pub fn create_python_skill(&self, name: &str, description: &str, code: &str, params_json: &str) -> Result<String> {
        let skill_dir = self.skills_dir.join(name);
        if skill_dir.exists() {
             return Err(anyhow!("Skill '{}' already exists", name));
        }

        fs::create_dir_all(&skill_dir)?;

        // Write script
        let script_path = skill_dir.join(format!("{}.py", name));
        fs::write(&script_path, code)?;

        // Write manifest
        let manifest = format!(
            r#"
name: {}
description: {}
version: "1.0.0"
entrypoint: "python3 {}.py"
parameters: {}
env: {{}}
"#,
            name, description, name, params_json
        );
        
        fs::write(skill_dir.join("skill.yaml"), manifest)?;

        Ok(format!("Successfully created skill '{}' in {:?}", name, skill_dir))
    }
}
