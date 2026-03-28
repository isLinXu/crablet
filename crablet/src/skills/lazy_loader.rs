//! Lazy Skill Loading Module
//!
//! Provides lazy loading mechanism for skills to improve startup time.
//! Skills are only loaded when first accessed, not at application startup.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use anyhow::Result;
use tracing::{info, warn};
use tokio::sync::RwLock;
use tokio::fs;

use super::{Skill, SkillType, SkillManifest, openclaw};

/// Lazy-loaded skill registry wrapper
/// Provides on-demand skill loading to reduce startup time
pub struct LazySkillRegistry {
    /// Pre-indexed skill manifests (loaded quickly, without full initialization)
    skill_index: RwLock<HashMap<String, SkillIndexEntry>>,
    /// Fully loaded skills cache (populated on first access)
    loaded_skills: Arc<RwLock<HashMap<String, SkillType>>>,
    /// Skills directory path
    skills_dir: PathBuf,
    /// Registry URL for fetching remote skills
    registry_url: String,
}

/// Lightweight skill index entry (without loading full skill)
#[derive(Clone, Debug)]
pub struct SkillIndexEntry {
    pub name: String,
    pub description: String,
    pub version: String,
    pub manifest_path: Option<PathBuf>,  // Local path if loaded from disk
    pub openclaw_md_path: Option<PathBuf>,  // OpenClaw skill path
    pub is_remote: bool,
    pub remote_url: Option<String>,
}

impl LazySkillRegistry {
    /// Create a new lazy skill registry
    pub fn new(skills_dir: PathBuf) -> Self {
        Self {
            skill_index: RwLock::new(HashMap::new()),
            loaded_skills: Arc::new(RwLock::new(HashMap::new())),
            skills_dir,
            registry_url: "https://raw.githubusercontent.com/crablet/skill-registry/main/index.json".to_string(),
        }
    }

    /// Quick index scan - loads only metadata without full skill initialization
    /// This should be called at startup for fast UI population
    pub async fn quick_index(&self) -> Result<Vec<SkillIndexEntry>> {
        let mut index = HashMap::new();

        // Scan skills directory
        if self.skills_dir.exists() {
            let mut entries = fs::read_dir(&self.skills_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let skill_dir = entry.path();
                if skill_dir.is_dir() {
                    self.index_skill_dir(&skill_dir, &mut index).await?;
                }
            }
        }

        // Store index
        let mut idx = self.skill_index.write().await;
        *idx = index.clone();

        Ok(index.into_values().collect())
    }

    /// Index a single skill directory (fast, no full loading)
    async fn index_skill_dir(&self, skill_dir: &Path, index: &mut HashMap<String, SkillIndexEntry>) -> Result<()> {
        let skill_name = skill_dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let yaml_path = skill_dir.join("skill.yaml");
        let json_path = skill_dir.join("skill.json");
        let md_path = skill_dir.join("SKILL.md");

        let entry = if yaml_path.exists() {
            // Read lightweight manifest info
            match self.read_manifest_summary(&yaml_path).await {
                Ok((desc, ver)) => SkillIndexEntry {
                    name: skill_name.clone(),
                    description: desc,
                    version: ver,
                    manifest_path: Some(yaml_path),
                    openclaw_md_path: None,
                    is_remote: false,
                    remote_url: None,
                },
                Err(_) => SkillIndexEntry {
                    name: skill_name.clone(),
                    description: String::new(),
                    version: "unknown".to_string(),
                    manifest_path: Some(yaml_path),
                    openclaw_md_path: None,
                    is_remote: false,
                    remote_url: None,
                }
            }
        } else if json_path.exists() {
            match self.read_manifest_summary(&json_path).await {
                Ok((desc, ver)) => SkillIndexEntry {
                    name: skill_name.clone(),
                    description: desc,
                    version: ver,
                    manifest_path: Some(json_path),
                    openclaw_md_path: None,
                    is_remote: false,
                    remote_url: None,
                },
                Err(_) => SkillIndexEntry {
                    name: skill_name.clone(),
                    description: String::new(),
                    version: "unknown".to_string(),
                    manifest_path: Some(json_path),
                    openclaw_md_path: None,
                    is_remote: false,
                    remote_url: None,
                }
            }
        } else if md_path.exists() {
            SkillIndexEntry {
                name: skill_name.clone(),
                description: "OpenClaw skill".to_string(),
                version: "1.0.0".to_string(),
                manifest_path: None,
                openclaw_md_path: Some(md_path),
                is_remote: false,
                remote_url: None,
            }
        } else {
            return Ok(());
        };

        index.insert(skill_name, entry);
        Ok(())
    }

    /// Read manifest summary (description and version only)
    async fn read_manifest_summary(&self, path: &Path) -> Result<(String, String)> {
        let content = fs::read_to_string(path).await?;
        let json: serde_json::Value = serde_json::from_str(&content)
            .or_else(|_| serde_yaml::from_str(&content).map_err(|e| anyhow::anyhow!("{}", e)))?;

        let description = json.get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let version = json.get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        Ok((description, version))
    }

    /// Get a skill by name (loads on first access)
    pub async fn get_skill(&self, name: &str) -> Result<Option<SkillType>> {
        // Check if already loaded
        {
            let loaded = self.loaded_skills.read().await;
            if let Some(skill) = loaded.get(name) {
                return Ok(Some(skill.clone()));
            }
        }

        // Load on first access
        let entry = {
            let index = self.skill_index.read().await;
            index.get(name).cloned()
        };

        if let Some(entry) = entry {
            let skill = if let Some(manifest_path) = &entry.manifest_path {
                self.load_local_skill(manifest_path).await.ok().map(SkillType::Local)
            } else if let Some(md_path) = &entry.openclaw_md_path {
                self.load_openclaw_skill(md_path).await.ok().map(|(s, i)| SkillType::OpenClaw(s, i))
            } else {
                None
            };

            if let Some(skill) = skill {
                // Cache the loaded skill
                let mut loaded = self.loaded_skills.write().await;
                loaded.insert(name.to_string(), skill.clone());
                return Ok(Some(skill));
            }
        }

        Ok(None)
    }

    /// Load a local skill from manifest
    async fn load_local_skill(&self, path: &Path) -> Result<Skill> {
        let content = tokio::fs::read_to_string(path).await?;
        let manifest: SkillManifest = if path.extension().and_then(|s| s.to_str()) == Some("json") {
            serde_json::from_str(&content)?
        } else {
            serde_yaml::from_str(&content)?
        };

        // Note: For lazy loading, we skip dependency check here for performance
        // Dependency check should happen at execution time

        Ok(Skill {
            manifest,
            path: path.parent().ok_or_else(|| anyhow::anyhow!("Invalid path"))?.to_path_buf(),
        })
    }

    /// Load an OpenClaw skill
    async fn load_openclaw_skill(&self, path: &Path) -> Result<(Skill, String)> {
        let skill = openclaw::OpenClawSkillLoader::load(path).await?;
        let instruction = openclaw::OpenClawSkillLoader::get_instruction(path).await?;
        Ok((skill, instruction))
    }

    /// Get list of indexed skills (fast, doesn't load full data)
    pub async fn list_indexed(&self) -> Vec<SkillIndexEntry> {
        let index = self.skill_index.read().await;
        index.values().cloned().collect()
    }

    /// Get count of indexed vs loaded skills (for monitoring)
    pub async fn stats(&self) -> LazyLoadStats {
        let indexed = self.skill_index.read().await.len();
        let loaded = self.loaded_skills.read().await.len();
        LazyLoadStats {
            indexed_count: indexed,
            loaded_count: loaded,
            not_loaded: indexed.saturating_sub(loaded),
        }
    }

    /// Preload specific skills (useful for predictive loading)
    pub async fn preload_skills(&self, names: &[String]) -> Result<()> {
        for name in names {
            if self.get_skill(name).await?.is_none() {
                warn!("Failed to preload skill: {}", name);
            }
        }
        Ok(())
    }

    /// Clear loaded skills cache (frees memory but keeps index)
    pub async fn clear_cache(&self) {
        let mut loaded = self.loaded_skills.write().await;
        let count = loaded.len();
        loaded.clear();
        info!("Cleared {} skills from lazy load cache", count);
    }
}

/// Statistics for lazy loading
#[derive(Debug, Clone)]
pub struct LazyLoadStats {
    pub indexed_count: usize,
    pub loaded_count: usize,
    pub not_loaded: usize,
}

impl LazyLoadStats {
    pub fn load_percentage(&self) -> f64 {
        if self.indexed_count == 0 {
            100.0
        } else {
            (self.loaded_count as f64 / self.indexed_count as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_lazy_registry_creation() {
        let temp_dir = env::temp_dir().join("crablet_test_skills");
        let registry = LazySkillRegistry::new(temp_dir);

        let stats = registry.stats().await;
        assert_eq!(stats.indexed_count, 0);
        assert_eq!(stats.loaded_count, 0);

        println!("Lazy registry created successfully");
    }
}