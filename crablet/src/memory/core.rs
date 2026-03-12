use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CoreMemoryBlock {
    Profile,
    Facts,
    Constraints,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CoreMemory {
    pub profile: Vec<String>,
    pub facts: Vec<String>,
    pub constraints: Vec<String>,
}

impl CoreMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let content = fs::read_to_string(path)?;
        let data = serde_json::from_str(&content)?;
        Ok(data)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn to_system_prompt(&self) -> String {
        format!(
            "Profile:\n{}\n\nFacts:\n{}\n\nConstraints:\n{}",
            self.profile.join("\n"),
            self.facts.join("\n"),
            self.constraints.join("\n")
        )
    }

    pub fn append(&mut self, block: CoreMemoryBlock, content: &str) -> Result<usize> {
        let target = match block {
            CoreMemoryBlock::Profile => &mut self.profile,
            CoreMemoryBlock::Facts => &mut self.facts,
            CoreMemoryBlock::Constraints => &mut self.constraints,
        };
        target.push(content.to_string());
        Ok(content.len())
    }

    pub fn replace(&mut self, block: CoreMemoryBlock, old_content: &str, new_content: &str) -> Result<bool> {
        let target = match block {
            CoreMemoryBlock::Profile => &mut self.profile,
            CoreMemoryBlock::Facts => &mut self.facts,
            CoreMemoryBlock::Constraints => &mut self.constraints,
        };
        if let Some(item) = target.iter_mut().find(|v| v.as_str() == old_content) {
            *item = new_content.to_string();
            return Ok(true);
        }
        Ok(false)
    }

    pub fn clear(&mut self, block: CoreMemoryBlock) {
        match block {
            CoreMemoryBlock::Profile => self.profile.clear(),
            CoreMemoryBlock::Facts => self.facts.clear(),
            CoreMemoryBlock::Constraints => self.constraints.clear(),
        }
    }
}
