use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{info, warn, error};
use std::sync::Arc;
use tokio::time::Duration;

pub mod watcher;
pub mod openclaw;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    pub version: String,
    pub parameters: serde_json::Value, // JSON Schema for arguments
    pub entrypoint: String, // Command to run (e.g., "python main.py")
    #[serde(default)]
    pub env: HashMap<String, String>, // Environment variables
}

#[derive(Clone)]
pub struct Skill {
    pub manifest: SkillManifest,
    pub path: std::path::PathBuf, // Directory containing the skill
}

// Enum to support different types of skills
#[derive(Clone)]
pub enum SkillType {
    Local(Skill),
    // Stores manifest, client, and tool name
    Mcp(SkillManifest, Arc<crate::tools::mcp::McpClient>, String),
    // Native Rust Plugin
    Plugin(SkillManifest, Arc<Box<dyn crate::plugins::Plugin>>),
    // OpenClaw Prompt Skill
    OpenClaw(Skill, String), // Skill + Instructions
}

pub struct SkillRegistry {
    skills: HashMap<String, SkillType>,
    resources: HashMap<String, (crate::tools::mcp::McpResource, Arc<crate::tools::mcp::McpClient>)>,
    prompts: HashMap<String, (crate::tools::mcp::McpPrompt, Arc<crate::tools::mcp::McpClient>)>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            resources: HashMap::new(),
            prompts: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.skills.clear();
        self.resources.clear();
        self.prompts.clear();
    }

    pub fn register_plugin(&mut self, plugin: Box<dyn crate::plugins::Plugin>) {
        let manifest = SkillManifest {
            name: plugin.name().to_string(),
            description: plugin.description().to_string(),
            version: "1.0.0 (Native)".to_string(),
            parameters: serde_json::json!({ "type": "object", "additionalProperties": true }), // TODO: Add schema to Plugin trait
            entrypoint: "native".to_string(),
            env: HashMap::new(),
        };
        self.skills.insert(plugin.name().to_string(), SkillType::Plugin(manifest, Arc::new(plugin)));
    }

    pub fn register_mcp_tool(&mut self, tool_name: String, client: Arc<crate::tools::mcp::McpClient>, description: Option<String>, input_schema: serde_json::Value) {
        let manifest = SkillManifest {
            name: tool_name.clone(),
            description: description.unwrap_or_default(),
            version: "1.0.0 (MCP)".to_string(),
            parameters: input_schema,
            entrypoint: "mcp".to_string(),
            env: HashMap::new(),
        };
        
        self.skills.insert(tool_name.clone(), SkillType::Mcp(manifest, client, tool_name));
    }

    pub fn register_mcp_resource(&mut self, resource: crate::tools::mcp::McpResource, client: Arc<crate::tools::mcp::McpClient>) {
        self.resources.insert(resource.uri.clone(), (resource, client));
    }

    pub fn register_mcp_prompt(&mut self, prompt: crate::tools::mcp::McpPrompt, client: Arc<crate::tools::mcp::McpClient>) {
        self.prompts.insert(prompt.name.clone(), (prompt, client));
    }

    /// Load skills from a directory (e.g., "./skills")
    pub async fn load_from_dir<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        if !path.exists() {
            warn!("Skills directory not found: {:?}", path);
            return Ok(());
        }

        let mut entries = tokio::fs::read_dir(path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let skill_dir = entry.path();
            if skill_dir.is_dir() {
                // Try to find skill.yaml or skill.json
                let yaml_path = skill_dir.join("skill.yaml");
                let json_path = skill_dir.join("skill.json");

                let manifest_path = if yaml_path.exists() {
                    Some(yaml_path)
                } else if json_path.exists() {
                    Some(json_path)
                } else {
                    None
                };

                if let Some(manifest_path) = manifest_path {
                    match self.load_skill(&manifest_path).await {
                        Ok(skill) => {
                            info!("Loaded skill: {} (v{})", skill.manifest.name, skill.manifest.version);
                            self.skills.insert(skill.manifest.name.clone(), SkillType::Local(skill));
                        }
                        Err(e) => {
                            error!("Failed to load skill from {:?}: {}", manifest_path, e);
                        }
                    }
                } else {
                    // Check for OpenClaw SKILL.md
                    let md_path = skill_dir.join("SKILL.md");
                    if md_path.exists() {
                        match openclaw::OpenClawSkillLoader::load(&md_path).await {
                            Ok(skill) => {
                                info!("Loaded OpenClaw skill: {}", skill.manifest.name);
                                let instruction = openclaw::OpenClawSkillLoader::get_instruction(&md_path).await.unwrap_or_default();
                                self.skills.insert(skill.manifest.name.clone(), SkillType::OpenClaw(skill, instruction));
                            }
                            Err(e) => {
                                error!("Failed to load OpenClaw skill from {:?}: {}", md_path, e);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn load_skill(&self, path: &Path) -> Result<Skill> {
        let content = tokio::fs::read_to_string(path).await?;
        let manifest: SkillManifest = if path.extension().and_then(|s| s.to_str()) == Some("json") {
            serde_json::from_str(&content)?
        } else {
            serde_yaml::from_str(&content)?
        };

        Ok(Skill {
            manifest,
            path: path.parent().unwrap().to_path_buf(),
        })
    }

    pub fn get_skill(&self, name: &str) -> Option<&SkillManifest> {
        match self.skills.get(name) {
            Some(SkillType::Local(s)) => Some(&s.manifest),
            Some(SkillType::Mcp(m, _, _)) => Some(m),
            Some(SkillType::Plugin(m, _)) => Some(m),
            Some(SkillType::OpenClaw(s, _)) => Some(&s.manifest),
            None => None,
        }
    }

    pub fn list_skills(&self) -> Vec<SkillManifest> {
        // Return owned SkillManifests to support dynamic generation
        let mut manifests = Vec::new();
        for skill_type in self.skills.values() {
             match skill_type {
                 SkillType::Local(s) => manifests.push(s.manifest.clone()),
                 SkillType::Mcp(m, _, _) => manifests.push(m.clone()),
                 SkillType::Plugin(m, _) => manifests.push(m.clone()),
                 SkillType::OpenClaw(s, _) => manifests.push(s.manifest.clone()),
             }
        }
        manifests
    }

    pub fn to_tool_definitions(&self) -> Vec<serde_json::Value> {
        self.list_skills().iter()
            .map(|s| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": s.name,
                    "description": s.description,
                    "parameters": s.parameters
                }
            })
        }).collect()
    }

    pub fn list_resources(&self) -> Vec<crate::tools::mcp::McpResource> {
        self.resources.values().map(|(r, _)| r.clone()).collect()
    }

    pub fn list_prompts(&self) -> Vec<crate::tools::mcp::McpPrompt> {
        self.prompts.values().map(|(p, _)| p.clone()).collect()
    }

    pub async fn read_resource(&self, uri: &str) -> Result<String> {
        if let Some((_, client)) = self.resources.get(uri) {
            client.read_resource(uri).await
        } else {
            Err(anyhow::anyhow!("Resource not found: {}", uri))
        }
    }

    pub async fn get_prompt(&self, name: &str, args: Option<serde_json::Value>) -> Result<String> {
        if let Some((_, client)) = self.prompts.get(name) {
            client.get_prompt(name, args).await
        } else {
            Err(anyhow::anyhow!("Prompt not found: {}", name))
        }
    }

    /// Execute a skill with arguments
    pub async fn execute(&self, name: &str, args: serde_json::Value) -> Result<String> {
        let skill_type = self.skills.get(name).context(format!("Skill not found: {}", name))?;
        
        // Timeout for execution (30 seconds)
        let timeout_duration = Duration::from_secs(30);

        let execution_future = async move {
            match skill_type {
                SkillType::Local(skill) => {
                    // Prepare command
                    let parts: Vec<&str> = skill.manifest.entrypoint.split_whitespace().collect();
                    if parts.is_empty() {
                        return Err(anyhow::anyhow!("Invalid entrypoint for skill {}", name));
                    }

                    let cmd = parts[0];
                    let cmd_args = &parts[1..];

                    let mut command = Command::new(cmd);
                    command.args(cmd_args);
                    command.current_dir(&skill.path);
                    
                    let args_json = serde_json::to_string(&args)?;
                    command.arg(&args_json);

                    for (k, v) in &skill.manifest.env {
                        command.env(k, v);
                    }

                    command.stdout(Stdio::piped());
                    command.stderr(Stdio::piped());

                    info!("Executing skill {}: {} {}", name, skill.manifest.entrypoint, args_json);

                    let output = command.spawn()?.wait_with_output().await?;

                    if output.status.success() {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        Ok(stdout)
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        Err(anyhow::anyhow!("Skill execution failed: {}", stderr))
                    }
                },
                SkillType::Mcp(_, client, tool_name) => {
                    info!("Executing MCP tool {}: {}", tool_name, args);
                    client.call_tool(tool_name, args).await
                },
                SkillType::Plugin(_, plugin) => {
                    info!("Executing Plugin {}: {}", name, args);
                    plugin.execute(name, args).await
                },
                SkillType::OpenClaw(_skill, instruction) => {
                    info!("Executing OpenClaw skill: {}", name);
                    
                    // Simple interpolation: Replace {{arg}} with value
                    let mut prompt = instruction.clone();
                    
                    // Check if instruction contains python code block ```python
                    if prompt.contains("```python") {
                        // Extract python code
                        if let Some(start) = prompt.find("```python") {
                            // Wait, end needs to be after start + len
                            let code_start = start + 9; // len("```python")
                            let code_block = &prompt[code_start..];
                            if let Some(code_end) = code_block.find("```") {
                                // No special handling for 'see' here anymore.
                                // If the skill exists, it executes as a prompt skill.
                                // If we want to disable 'see', we must ensure it's not loaded into the registry.
                            }
                        }
                    }
                    
                    if let Some(obj) = args.as_object() {
                        for (k, v) in obj {
                            let key = format!("{{{{{}}}}}", k); // {{key}}
                            let val = v.as_str().unwrap_or(&v.to_string()).to_string();
                            prompt = prompt.replace(&key, &val);
                        }
                    }
                    
                    // Let's just return a generic success message if it's not a prompt skill
                    if prompt.len() > 500 {
                        Ok(format!("Executed skill '{}'. (Output suppressed as it seems to be documentation)", name))
                    } else {
                         Ok(format!("### INSTRUCTION FROM SKILL\n{}", prompt))
                    }
                }
            }
        };

        match tokio::time::timeout(timeout_duration, execution_future).await {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!("Skill execution timed out after 30s")),
        }
    }
}
