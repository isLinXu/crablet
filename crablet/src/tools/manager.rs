use super::Tool;
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;

/// Name-keyed tool registry.
///
/// Registration rejects empty and duplicate names. Lookup is O(1) on average,
/// while [`ToolManager::list_tools`] sorts names to provide deterministic output.
pub struct ToolManager {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolManager {
    /// Create an empty tool registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool under its declared name.
    pub fn add_tool(&mut self, tool: Box<dyn Tool>) -> Result<()> {
        let declared_name = tool.name();
        let name = declared_name.trim();
        if name.is_empty() {
            return Err(anyhow!("Tool name cannot be empty"));
        }
        if name != declared_name {
            return Err(anyhow!(
                "Tool name cannot have leading or trailing whitespace"
            ));
        }
        if self.tools.contains_key(name) {
            return Err(anyhow!("Tool already registered: {}", name));
        }

        self.tools.insert(name.to_owned(), tool);
        Ok(())
    }

    /// Find a tool by its exact registered name in O(1) average time.
    pub fn get_tool(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.get(name).map(Box::as_ref)
    }

    /// Return tools ordered by name for deterministic discovery output.
    pub fn list_tools(&self) -> Vec<&dyn Tool> {
        let mut tools: Vec<_> = self.tools.values().map(Box::as_ref).collect();
        tools.sort_unstable_by(|left, right| left.name().cmp(right.name()));
        tools
    }

    /// Return OpenAI-compatible function definitions for tool-aware clients.
    pub fn to_tool_definitions(&self) -> Vec<Value> {
        self.list_tools()
            .into_iter()
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": tool.parameters(),
                    }
                })
            })
            .collect()
    }

    /// Execute a registered tool.
    pub async fn execute(&self, name: &str, args: Value) -> Result<String> {
        let tool = self
            .get_tool(name)
            .ok_or_else(|| anyhow!("Tool not found: {}", name))?;
        tool.execute(name, args).await
    }

    /// Check whether the registry is empty.
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
            url.split('/')
                .next_back()
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
            Ok(format!(
                "Successfully installed skill '{}' to {:?}",
                dir_name, target_path
            ))
        } else {
            Err(anyhow!("Failed to clone repository"))
        }
    }

    /// Create a new simple Python skill from scratch (Self-Evolution)
    pub fn create_python_skill(
        &self,
        name: &str,
        description: &str,
        code: &str,
        params_json: &str,
    ) -> Result<String> {
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

        Ok(format!(
            "Successfully created skill '{}' in {:?}",
            name, skill_dir
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;

    struct TestTool {
        name: &'static str,
    }

    #[async_trait]
    impl Tool for TestTool {
        fn name(&self) -> &str {
            self.name
        }

        fn description(&self) -> &str {
            "test tool"
        }

        fn parameters(&self) -> Value {
            json!({"type": "object"})
        }

        async fn execute(&self, command: &str, args: Value) -> Result<String> {
            Ok(format!("{}:{}", command, args["value"]))
        }
    }

    #[test]
    fn registers_and_lists_tools_deterministically() {
        let mut manager = ToolManager::new();
        manager
            .add_tool(Box::new(TestTool { name: "zeta" }))
            .unwrap();
        manager
            .add_tool(Box::new(TestTool { name: "alpha" }))
            .unwrap();

        assert!(manager.get_tool("alpha").is_some());
        let names: Vec<_> = manager
            .list_tools()
            .iter()
            .map(|tool| tool.name())
            .collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
    }

    #[test]
    fn exposes_tool_definitions_with_declared_schema() {
        let mut manager = ToolManager::new();
        manager
            .add_tool(Box::new(TestTool { name: "echo" }))
            .unwrap();

        assert_eq!(
            manager.to_tool_definitions(),
            vec![json!({
                "type": "function",
                "function": {
                    "name": "echo",
                    "description": "test tool",
                    "parameters": {"type": "object"}
                }
            })]
        );
    }

    #[test]
    fn rejects_empty_and_duplicate_names() {
        let mut manager = ToolManager::new();
        assert!(manager.add_tool(Box::new(TestTool { name: "  " })).is_err());
        assert!(manager
            .add_tool(Box::new(TestTool { name: " echo " }))
            .is_err());
        manager
            .add_tool(Box::new(TestTool { name: "echo" }))
            .unwrap();
        let error = manager
            .add_tool(Box::new(TestTool { name: "echo" }))
            .unwrap_err();
        assert_eq!(error.to_string(), "Tool already registered: echo");
    }

    #[tokio::test]
    async fn preserves_unknown_tool_error() {
        let manager = ToolManager::new();
        let error = manager.execute("missing", json!({})).await.unwrap_err();
        assert_eq!(error.to_string(), "Tool not found: missing");
    }

    #[tokio::test]
    async fn executes_registered_tool() {
        let mut manager = ToolManager::new();
        manager
            .add_tool(Box::new(TestTool { name: "echo" }))
            .unwrap();

        let output = manager
            .execute("echo", json!({"value": "ok"}))
            .await
            .unwrap();
        assert_eq!(output, "echo:\"ok\"");
    }
}
