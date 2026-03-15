use anyhow::{Result, Context, anyhow};
use crate::skills::{Skill, SkillManifest, SkillDependencies, SkillResources, SkillTrigger};
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
    /// Triggers for automatic skill activation
    #[serde(default)]
    triggers: Vec<SkillTrigger>,
    /// Author of the skill
    #[serde(default)]
    author: Option<String>,
    /// Minimum Crablet version required
    #[serde(default)]
    min_crablet_version: Option<String>,
    /// Permissions required by the skill
    #[serde(default)]
    permissions: Vec<String>,
    /// Conflicting skills
    #[serde(default)]
    conflicts: Vec<String>,
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
        
        // Auto-generate triggers if none defined
        let triggers = if frontmatter.triggers.is_empty() {
            Self::generate_triggers(&frontmatter)
        } else {
            frontmatter.triggers
        };
            
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
            permissions: frontmatter.permissions,
            conflicts: frontmatter.conflicts,
            min_crablet_version: frontmatter.min_crablet_version,
            author: frontmatter.author,
            triggers,
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
    
    /// Generate default triggers for an OpenClaw skill
    fn generate_triggers(frontmatter: &FrontMatter) -> Vec<SkillTrigger> {
        let mut triggers = Vec::new();
        
        // 1. Command trigger from skill name
        triggers.push(SkillTrigger::Command {
            prefix: format!("/{}", frontmatter.name.to_lowercase()),
            args_schema: frontmatter.parameters.clone(),
        });
        
        // 2. Keyword trigger from description
        let keywords: Vec<String> = frontmatter.description
            .split_whitespace()
            .filter(|w| w.len() > 3)
            .take(5)
            .map(|w| w.to_lowercase().trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| !w.is_empty())
            .collect();
        
        if !keywords.is_empty() {
            triggers.push(SkillTrigger::Keyword {
                keywords,
                case_sensitive: false,
            });
        }
        
        // 3. Semantic trigger
        triggers.push(SkillTrigger::Semantic {
            description: frontmatter.description.clone(),
            threshold: 0.75,
        });
        
        triggers
    }
    
    /// Parse SKILL.md content without file I/O (for testing)
    pub fn parse_content(content: &str) -> Result<(SkillManifest, String)> {
        let parts: Vec<&str> = content.split("---").collect();
        if parts.len() < 3 {
            return Err(anyhow!("Invalid SKILL.md format: missing frontmatter"));
        }
        
        let yaml_str = parts[1];
        let frontmatter: FrontMatter = serde_yaml::from_str(yaml_str)
            .context("Failed to parse SKILL.md frontmatter")?;
        
        let instruction = parts[2..].join("---").trim().to_string();
        
        // Auto-generate triggers if none defined
        let triggers = if frontmatter.triggers.is_empty() {
            Self::generate_triggers(&frontmatter)
        } else {
            frontmatter.triggers
        };
        
        let manifest = SkillManifest {
            name: frontmatter.name,
            description: frontmatter.description,
            version: frontmatter.version.unwrap_or_else(|| "1.0.0".to_string()),
            parameters: frontmatter.parameters.unwrap_or_else(|| serde_json::json!({
                "type": "object", 
                "properties": {},
                "additionalProperties": true
            })),
            entrypoint: "openclaw".to_string(),
            env: HashMap::new(),
            requires: vec![],
            runtime: None,
            dependencies: frontmatter.dependencies,
            resources: frontmatter.resources,
            permissions: frontmatter.permissions,
            conflicts: frontmatter.conflicts,
            min_crablet_version: frontmatter.min_crablet_version,
            author: frontmatter.author,
            triggers,
        };
        
        Ok((manifest, instruction))
    }
}
