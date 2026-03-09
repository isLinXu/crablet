use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result;
use tracing::{info, warn, error};
use std::path::{Path, PathBuf};
use super::{Skill, SkillType, SkillManifest, openclaw, dependency};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct RegistryIndexItem {
    pub name: String,
    pub description: String,
    pub version: String,
    pub url: String, // Git URL or tarball URL
    pub author: Option<String>,
    pub rating: Option<f32>,
    pub downloads: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RegistryIndex {
    pub skills: Vec<RegistryIndexItem>,
    pub last_updated: String,
}

pub struct SkillRegistry {
    pub(crate) skills: HashMap<String, SkillType>,
    resources: HashMap<String, (crate::tools::mcp::McpResource, Arc<crate::tools::mcp::McpClient>)>,
    prompts: HashMap<String, (crate::tools::mcp::McpPrompt, Arc<crate::tools::mcp::McpClient>)>,
    registry_url: String,
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
            resources: HashMap::new(),
            prompts: HashMap::new(),
            registry_url: "https://raw.githubusercontent.com/crablet/skill-registry/main/index.json".to_string(),
        }
    }
    
    pub fn with_registry_url(mut self, url: String) -> Self {
        self.registry_url = url;
        self
    }
    
    // Add insert_skill method for testing or manual registration
    pub fn insert_skill(&mut self, name: String, skill_type: SkillType) {
        self.skills.insert(name, skill_type);
    }

    pub fn unregister(&mut self, name: &str) -> Result<()> {
        if self.skills.remove(name).is_some() {
            info!("Unregistered skill: {}", name);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Skill not found: {}", name))
        }
    }

    pub fn clear(&mut self) {
        self.skills.clear();
        self.resources.clear();
        self.prompts.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn backup(&self) -> RegistryBackup {
        RegistryBackup {
            skills: self.skills.clone(),
            resources: self.resources.clone(),
            prompts: self.prompts.clone(),
        }
    }

    pub fn restore(&mut self, backup: RegistryBackup) {
        self.skills = backup.skills;
        self.resources = backup.resources;
        self.prompts = backup.prompts;
    }

    pub fn register_plugin(&mut self, plugin: Box<dyn crate::plugins::Plugin>) {
        let manifest = SkillManifest {
            name: plugin.name().to_string(),
            description: plugin.description().to_string(),
            version: "1.0.0 (Native)".to_string(),
            parameters: serde_json::json!({ "type": "object", "additionalProperties": true }), // TODO: Add schema to Plugin trait
            entrypoint: "native".to_string(),
            env: HashMap::new(),
            requires: vec![],
            runtime: None,
            dependencies: None,
            resources: None,
            permissions: vec![],
            conflicts: vec![],
            min_crablet_version: None,
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
            requires: vec![],
            runtime: Some("mcp".to_string()),
            dependencies: None,
            resources: None,
            permissions: vec![],
            conflicts: vec![],
            min_crablet_version: None,
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
        
        // Dependency check
        dependency::check_dependencies(self, &manifest).await?;

        Ok(Skill {
            manifest,
            path: path.parent().ok_or_else(|| anyhow::anyhow!("Invalid skill path: has no parent"))?.to_path_buf(),
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
        let mut tools: Vec<serde_json::Value> = self.list_skills().iter()
            .map(|s| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": s.name,
                    "description": s.description,
                    "parameters": s.parameters
                }
            })
        }).collect();

        // Inject MCP Resource Tools if any resources exist
        if !self.resources.is_empty() {
            tools.push(serde_json::json!({
                "type": "function",
                "function": {
                    "name": "list_resources",
                    "description": "List available MCP resources (data sources)",
                    "parameters": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                }
            }));

            tools.push(serde_json::json!({
                "type": "function",
                "function": {
                    "name": "read_resource",
                    "description": "Read content of an MCP resource",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "uri": {
                                "type": "string",
                                "description": "The URI of the resource to read"
                            }
                        },
                        "required": ["uri"]
                    }
                }
            }));
        }

        // Inject MCP Prompt Tools if any prompts exist
        if !self.prompts.is_empty() {
             tools.push(serde_json::json!({
                "type": "function",
                "function": {
                    "name": "list_prompts",
                    "description": "List available MCP prompts",
                    "parameters": {
                        "type": "object",
                        "properties": {},
                        "required": []
                    }
                }
            }));
            
            tools.push(serde_json::json!({
                "type": "function",
                "function": {
                    "name": "get_prompt",
                    "description": "Get an MCP prompt template",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "arguments": { "type": "object" }
                        },
                        "required": ["name"]
                    }
                }
            }));
        }

        tools
    }
    
    // Delegate execution to executor
    pub async fn execute(&self, name: &str, args: serde_json::Value) -> Result<String> {
        // Intercept built-in MCP tools
        match name {
            "list_resources" => {
                let resources = self.list_resources();
                return Ok(serde_json::to_string_pretty(&resources)?);
            },
            "read_resource" => {
                let uri = args.get("uri").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'uri' parameter"))?;
                return self.read_resource(uri).await;
            },
            "list_prompts" => {
                let prompts = self.list_prompts();
                return Ok(serde_json::to_string_pretty(&prompts)?);
            },
            "get_prompt" => {
                let prompt_name = args.get("name").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing 'name' parameter"))?;
                let prompt_args = args.get("arguments").cloned();
                return self.get_prompt(prompt_name, prompt_args).await;
            },
            _ => {}
        }

        super::executor::SkillExecutor::execute(self, name, args).await
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

    pub async fn fetch_registry(&self) -> Result<RegistryIndex> {
        let client = reqwest::Client::new();
        let resp = client.get(&self.registry_url).send().await?;
        let index: RegistryIndex = resp.json().await?;
        Ok(index)
    }

    pub async fn search(&self, query: &str) -> Result<Vec<RegistryIndexItem>> {
        let index = self.fetch_registry().await?;
        let query = query.to_lowercase();
        
        let results = index.skills.into_iter()
            .filter(|s| s.name.to_lowercase().contains(&query) || s.description.to_lowercase().contains(&query))
            .collect();
            
        Ok(results)
    }

    pub async fn install(&mut self, name: &str, target_dir: PathBuf) -> Result<()> {
        let index = self.fetch_registry().await?;
        let skill = index.skills.iter()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow::anyhow!("Skill '{}' not found in registry", name))?;
            
        info!("Installing skill '{}' from {}", name, skill.url);
        
        // Ensure target directory exists
        if !target_dir.exists() {
            tokio::fs::create_dir_all(&target_dir).await?;
        }
        
        let install_path = target_dir.join(name);
        if install_path.exists() {
            anyhow::bail!("Skill directory already exists: {:?}", install_path);
        }
        
        // Clone repo
        info!("Cloning {} to {:?}", skill.url, install_path);
        
        // Simple git clone for now
        let status = std::process::Command::new("git")
            .arg("clone")
            .arg(&skill.url)
            .arg(&install_path)
            .status()?;
            
        if !status.success() {
            anyhow::bail!("Failed to clone skill repository");
        }
        
        // Load the installed skill
        self.load_from_dir(&target_dir).await?;
        
        Ok(())
    }
}

#[derive(Clone)]
pub struct RegistryBackup {
    pub(crate) skills: HashMap<String, SkillType>,
    pub(crate) resources: HashMap<String, (crate::tools::mcp::McpResource, Arc<crate::tools::mcp::McpClient>)>,
    pub(crate) prompts: HashMap<String, (crate::tools::mcp::McpPrompt, Arc<crate::tools::mcp::McpClient>)>,
}
