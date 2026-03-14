use anyhow::{Result, Context, anyhow};
use crate::skills::{Skill, SkillManifest, SkillDependencies, SkillResources};
use std::path::Path;
use std::collections::HashMap;
use serde::Deserialize;

pub struct OpenClawSkillLoader;

#[derive(Deserialize)]
struct FrontMatter {
    name: String,
    description: String,
    version: Option<String>,
    #[serde(default)]
    parameters: Option<serde_json::Value>,
    #[serde(default)]
    dependencies: Option<SkillDependencies>,
    #[serde(default)]
    resources: Option<SkillResources>,
}

impl OpenClawSkillLoader {
    pub async fn load(path: &Path) -> Result<Skill> {
        let content = tokio::fs::read_to_string(path).await?;
        
        // Parse Frontmatter
        let parts: Vec<&str> = content.split("---").collect();
        // split("---") on a file starting with --- gives ["", "yaml content", "markdown content"]
        // So parts[1] is yaml.
        if parts.len() < 3 {
            return Err(anyhow!("Invalid SKILL.md format: missing frontmatter"));
        }
        
        let yaml_str = parts[1];
        let frontmatter: FrontMatter = serde_yaml::from_str(yaml_str)
            .context("Failed to parse SKILL.md frontmatter")?;
            
        let manifest = SkillManifest {
            name: frontmatter.name,
            description: frontmatter.description,
            version: frontmatter.version.unwrap_or_else(|| "1.0.0".to_string()),
            parameters: frontmatter.parameters.unwrap_or_else(|| serde_json::json!({
                "type": "object", 
                "properties": {},
                "additionalProperties": true
            })),
            entrypoint: "openclaw".to_string(), // Virtual entrypoint
            env: HashMap::new(),
            requires: vec![],
            runtime: None,
            dependencies: frontmatter.dependencies,
            resources: frontmatter.resources,
            permissions: vec![],
            conflicts: vec![],
            min_crablet_version: None,
            author: None,
        };
        
        Ok(Skill {
            manifest,
            path: path.parent().ok_or_else(|| anyhow!("Invalid skill path: has no parent"))?.to_path_buf(),
        })
    }
    
    pub async fn get_instruction(path: &Path) -> Result<String> {
        let content = tokio::fs::read_to_string(path).await?;
        let parts: Vec<&str> = content.split("---").collect();
        if parts.len() < 3 {
            return Ok(content); // Return whole content if no frontmatter
        }
        // Join the rest back, in case body has ---
        Ok(parts[2..].join("---").trim().to_string())
    }
}
