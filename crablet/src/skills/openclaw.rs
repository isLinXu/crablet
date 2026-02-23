use anyhow::{Result, Context};
use crate::skills::{Skill, SkillManifest};
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
}

impl OpenClawSkillLoader {
    pub async fn load(path: &Path) -> Result<Skill> {
        let content = tokio::fs::read_to_string(path).await?;
        
        // Parse Frontmatter
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() < 3 {
            return Err(anyhow::anyhow!("Invalid SKILL.md format: missing frontmatter"));
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
        };
        
        Ok(Skill {
            manifest,
            path: path.parent().unwrap().to_path_buf(),
        })
    }
    
    pub async fn get_instruction(path: &Path) -> Result<String> {
        let content = tokio::fs::read_to_string(path).await?;
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() < 3 {
            return Ok(content); // Return whole content if no frontmatter
        }
        Ok(parts[2].trim().to_string())
    }
}
