//! Core Memory - Persistent memory that is always visible to the LLM
//!
//! This struct implements the MemGPT-style Core Memory concept:
//! - **Persona**: Agent's identity, behavior guidelines, and and capabilities
//! - **Human**: User profile, preferences, and interaction history summary
//! - **Memory**: Important facts, decisions, and key information
//!
//! All content is automatically truncated to fit within limits.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Core memory block types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreMemoryBlock {
    /// Agent's persona and behavior guidelines (~500 chars)
    Persona,
    /// User profile and preferences (~1000 chars)
    Human,
    /// Important facts and memories (~2000 chars)
    Memory,
}

impl CoreMemoryBlock {
    /// Get the character limit for this block
    pub fn char_limit(&self) -> usize {
        match self {
            CoreMemoryBlock::Persona => 500,
            CoreMemoryBlock::Human => 1000,
            CoreMemoryBlock::Memory => 2000,
        }
    }

    /// Get the block name as string
    pub fn as_str(&self) -> &'static str {
        match self {
            CoreMemoryBlock::Persona => "persona",
                CoreMemoryBlock::Human => "human",
                CoreMemoryBlock::Memory => "memory",
            }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "persona" => Some(CoreMemoryBlock::Persona),
            "human" => Some(CoreMemoryBlock::Human),
            "memory" => Some(CoreMemoryBlock::Memory),
            _ => None,
        }
    }
}

/// Configuration for Core Memory limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreMemoryLimits {
    /// Maximum characters for Persona block
    pub persona_limit: usize,
    /// Maximum characters for Human block  
    pub human_limit: usize,
    /// Maximum characters for Memory block
    pub memory_limit: usize,
}

impl Default for CoreMemoryLimits {
    fn default() -> Self {
        Self {
            persona_limit: 500,
            human_limit: 1000,
            memory_limit: 2000,
        }
    }
}

/// Core Memory - Persistent memory that is always visible to the LLM
///
/// This struct implements the MemGPT-style Core Memory concept:
/// - **Persona**: Agent's identity, behavior guidelines, and capabilities
/// - **Human**: User profile, preferences, and interaction history summary
/// - **Memory**: Important facts, decisions, and key information
///
/// All content is automatically truncated to fit within limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreMemory {
    /// Agent persona and behavior guidelines
    pub persona: String,
    /// User profile and preferences
    pub human: String,
    /// Important facts and memories
    pub memory: String,
    /// Character limits for each block
    #[serde(default)]
    pub limits: CoreMemoryLimits,
    /// Last modification timestamp
    pub last_modified: chrono::DateTime<chrono::Utc>,
    /// Version number for conflict detection
    pub version: u64,
}

impl Default for CoreMemory {
    fn default() -> Self {
        Self {
            persona: String::new(),
            human: String::new(),
            memory: String::new(),
            limits: CoreMemoryLimits::default(),
            last_modified: chrono::Utc::now(),
            version: 1,
        }
    }
}

impl CoreMemory {
    /// Create a new CoreMemory with default limits
    pub fn new() -> Self {
        Self::default()
    }

    /// Create CoreMemory with custom limits
    pub fn with_limits(limits: CoreMemoryLimits) -> Self {
        Self {
            limits,
            ..Self::default()
        }
    }

    /// Get the content of a specific block
    pub fn get(&self, block: CoreMemoryBlock) -> &str {
        match block {
            CoreMemoryBlock::Persona => &self.persona,
            CoreMemoryBlock::Human => &self.human,
            CoreMemoryBlock::Memory => &self.memory,
        }
    }

    /// Get mutable reference to a specific block
    fn get_mut(&mut self, block: CoreMemoryBlock) -> &mut String {
        self.touch();
        match block {
            CoreMemoryBlock::Persona => &mut self.persona,
            CoreMemoryBlock::Human => &mut self.human,
            CoreMemoryBlock::Memory => &mut self.memory,
        }
    }

    /// Get the character limit for a block
    pub fn get_limit(&self, block: CoreMemoryBlock) -> usize {
        match block {
            CoreMemoryBlock::Persona => self.limits.persona_limit,
            CoreMemoryBlock::Human => self.limits.human_limit,
            CoreMemoryBlock::Memory => self.limits.memory_limit,
        }
    }

    /// Update the last_modified timestamp and increment version
    fn touch(&mut self) {
        self.last_modified = chrono::Utc::now();
        self.version += 1;
    }

    /// Append content to a specific block
    ///
    /// The content will be truncated if it exceeds the block's character limit.
    /// A newline is automatically added between existing content and new content.
    ///
    /// # Arguments
    /// * `block` - The memory block to append to
    /// * `content` - The content to append
    ///
    /// # Returns
    /// * `Ok(usize)` - The number of characters actually appended
    /// * `Err` - If the block is already at capacity
    pub fn append(&mut self, block: CoreMemoryBlock, content: &str) -> Result<usize> {
        let limit = self.get_limit(block);
        let current = self.get(block);

        if current.len() >= limit {
            return Err(anyhow!(
                "Core memory block '{}' is at capacity ({}/{} chars)",
                block.as_str(),
                current.len(),
                limit
            ));
        }

        let available = limit.saturating_sub(current.len());
        let content_to_add = if content.len() > available {
            warn!(
                "Core memory append truncated: {} chars requested, {} available",
                content.len(),
                available
            );
            &content[..available]
        } else {
            content
        };

        let block_ref = self.get_mut(block);
        
        // Add separator if block is not empty
        if !block_ref.is_empty() && !content_to_add.is_empty() {
            block_ref.push('\n');
        }
        block_ref.push_str(content_to_add);

        info!(
            "Core memory '{}' appended {} chars (total: {}/{})",
            block.as_str(),
            content_to_add.len(),
            block_ref.len(),
            limit
        );

        Ok(content_to_add.len())
    }

    /// Replace content in a specific block
    ///
    /// Searches for `old_content` and replaces it with `new_content`.
    /// If `old_content` is empty, replaces the entire block.
    ///
    /// # Arguments
    /// * `block` - The memory block to modify
    /// * `old_content` - The content to replace (empty = replace all)
    /// * `new_content` - The new content
    ///
    /// # Returns
    /// * `Ok(bool)` - True if replacement was made
    /// * `Err` - If new content exceeds limit
    pub fn replace(
        &mut self,
        block: CoreMemoryBlock,
        old_content: &str,
        new_content: &str,
    ) -> Result<bool> {
        let limit = self.get_limit(block);

        if old_content.is_empty() {
            // Replace entire block
            if new_content.len() > limit {
                return Err(anyhow!(
                    "New content exceeds limit: {} > {}",
                    new_content.len(),
                    limit
                ));
            }
            let block_ref = self.get_mut(block);
            let old_len = block_ref.len();
            block_ref.clear();
            block_ref.push_str(new_content);
            
            info!(
                "Core memory '{}' replaced entirely ({} -> {} chars)",
                block.as_str(),
                old_len,
                new_content.len()
            );
            
            return Ok(true);
        }

        // Replace specific content
        let block_ref = self.get_mut(block);
        
        if !block_ref.contains(old_content) {
            warn!(
                "Core memory '{}' does not contain the content to replace",
                block.as_str()
            );
            return Ok(false);
        }

        // Calculate new length
        let new_len = block_ref.len().saturating_sub(old_content.len()) + new_content.len();
        if new_len > limit {
            return Err(anyhow!(
                "Replacement would exceed limit: {} > {}",
                new_len,
                limit
            ));
        }

        *block_ref = block_ref.replace(old_content, new_content);
        
        info!(
            "Core memory '{}' replaced content (total: {}/{})",
            block.as_str(),
            block_ref.len(),
            limit
        );

        Ok(true)
    }

    /// Clear a specific block
    pub fn clear(&mut self, block: CoreMemoryBlock) {
        let block_ref = self.get_mut(block);
        let old_len = block_ref.len();
        block_ref.clear();
        
        info!(
            "Core memory '{}' cleared ({} chars removed)",
            block.as_str(),
            old_len
        );
    }

    /// Format Core Memory as a system prompt for LLM injection
    ///
    /// This creates a formatted string that can be prepended to the
    /// LLM's context window as a system message.
    pub fn to_system_prompt(&self) -> String {
        let mut prompt = String::from("## Core Memory (Always Visible)\n\n");
        prompt.push_str("This is your persistent memory that you can read and modify using the memory tools.\n\n");

        if !self.persona.is_empty() {
            prompt.push_str("### Persona (Your Identity)\n");
            prompt.push_str(&self.persona);
            prompt.push_str("\n\n");
        }

        if !self.human.is_empty() {
            prompt.push_str("### Human Profile (User Information)\n");
            prompt.push_str(&self.human);
            prompt.push_str("\n\n");
        }

        if !self.memory.is_empty() {
            prompt.push_str("### Memory (Important Facts)\n");
            prompt.push_str(&self.memory);
            prompt.push_str("\n\n");
        }

        prompt.push_str("---\n");
        prompt.push_str("You can use `core_memory_append` to add new information and `core_memory_replace` to update existing information.\n");

        prompt
    }

    /// Get total character count across all blocks
    pub fn total_chars(&self) -> usize {
        self.persona.len() + self.human.len() + self.memory.len()
    }

    /// Get total capacity across all blocks
    pub fn total_capacity(&self) -> usize {
        self.limits.persona_limit + self.limits.human_limit + self.limits.memory_limit
    }

    /// Get usage percentage (0.0 - 1.0)
    pub fn usage_ratio(&self) -> f32 {
        self.total_chars() as f32 / self.total_capacity() as f32
    }

    /// Check if any block is near capacity (>90%)
    pub fn is_near_capacity(&self) -> bool {
        self.persona.len() as f32 / self.limits.persona_limit as f32 > 0.9
            || self.human.len() as f32 / self.limits.human_limit as f32 > 0.9
            || self.memory.len() as f32 / self.limits.memory_limit as f32 > 0.9
    }

    /// Save Core Memory to a file
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        info!("Core memory saved to {:?}", path);
        Ok(())
    }

    /// Load Core Memory from a file
    pub fn load(path: &std::path::Path) -> Result<Self> {
        if !path.exists() {
            info!("Core memory file not found, creating new one");
            return Ok(Self::default());
        }

        let json = std::fs::read_to_string(path)?;
        let core: CoreMemory = serde_json::from_str(&json)?;
        info!(
            "Core memory loaded from {:?} (version: {}, {} chars)",
            path,
            core.version,
            core.total_chars()
        );
        Ok(core)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_core_memory_default() {
        let core = CoreMemory::default();
        assert!(core.persona.is_empty());
        assert!(core.human.is_empty());
        assert!(core.memory.is_empty());
        assert_eq!(core.version, 1);
    }

    #[test]
    fn test_core_memory_append() {
        let mut core = CoreMemory::default();
        
        let added = core.append(CoreMemoryBlock::Persona, "I am a helpful assistant.").unwrap();
        assert_eq!(added, 24);
        assert_eq!(core.persona, "I am a helpful assistant.");
        
        core.append(CoreMemoryBlock::Persona, "I like to be concise.").unwrap();
        assert!(core.persona.contains('\n'));
        assert!(core.persona.ends_with("I like to be concise."));
    }

    #[test]
    fn test_core_memory_append_truncation() {
        let mut core = CoreMemory::default();
        
        // Try to add content exceeding limit
        let long_content = "x".repeat(1000);
        let added = core.append(CoreMemoryBlock::Persona, &long_content).unwrap();
        
        // Should be truncated to limit (500)
        assert_eq!(added, 500);
        assert_eq!(core.persona.len(), 500);
    }

    #[test]
    fn test_core_memory_replace_entire() {
        let mut core = CoreMemory::default();
        core.append(CoreMemoryBlock::Memory, "Old memory").unwrap();
        
        core.replace(CoreMemoryBlock::Memory, "", "New memory").unwrap();
        assert_eq!(core.memory, "New memory");
    }

    #[test]
    fn test_core_memory_replace_partial() {
        let mut core = CoreMemory::default();
        core.append(CoreMemoryBlock::Memory, "User likes Python").unwrap();
        
        let replaced = core.replace(CoreMemoryBlock::Memory, "Python", "Rust").unwrap();
        assert!(replaced);
        assert_eq!(core.memory, "User likes Rust");
    }

    #[test]
    fn test_core_memory_replace_not_found() {
        let mut core = CoreMemory::default();
        core.append(CoreMemoryBlock::Memory, "Some content").unwrap();
        
        let replaced = core.replace(CoreMemoryBlock::Memory, "nonexistent", "new").unwrap();
        assert!(!replaced);
        assert_eq!(core.memory, "Some content");
    }

    #[test]
    fn test_core_memory_to_system_prompt() {
        let mut core = CoreMemory::default();
        core.append(CoreMemoryBlock::Persona, "I am helpful.").unwrap();
        core.append(CoreMemoryBlock::Human, "User is friendly.").unwrap();
        core.append(CoreMemoryBlock::Memory, "Important fact.").unwrap();
        
        let prompt = core.to_system_prompt();
        
        assert!(prompt.contains("Core Memory"));
        assert!(prompt.contains("I am helpful."));
        assert!(prompt.contains("User is friendly."));
        assert!(prompt.contains("Important fact."));
        assert!(prompt.contains("core_memory_append"));
    }

    #[test]
    fn test_core_memory_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("core_memory.json");
        
        let mut core = CoreMemory::default();
        core.append(CoreMemoryBlock::Persona, "Test persona").unwrap();
        core.append(CoreMemoryBlock::Human, "Test human").unwrap();
        
        core.save(&path).unwrap();
        
        let loaded = CoreMemory::load(&path).unwrap();
        assert_eq!(loaded.persona, "Test persona");
        assert_eq!(loaded.human, "Test human");
    }

    #[test]
    fn test_core_memory_usage() {
        let mut core = CoreMemory::default();
        assert_eq!(core.usage_ratio(), 0.0);
        
        // Fill persona to 50%
        let content = "x".repeat(250);
        core.append(CoreMemoryBlock::Persona, &content).unwrap();
        
        // Total capacity is 500 + 1000 + 2000 = 3500
        // 250 / 3500 ≈ 0.071
        assert!((core.usage_ratio() - 0.071).abs() < 0.01);
    }

    #[test]
    fn test_core_memory_block_limits() {
        assert_eq!(CoreMemoryBlock::Persona.char_limit(), 500);
        assert_eq!(CoreMemoryBlock::Human.char_limit(), 1000);
        assert_eq!(CoreMemoryBlock::Memory.char_limit(), 2000);
    }

    #[test]
    fn test_core_memory_block_from_str() {
        assert_eq!(CoreMemoryBlock::from_str("persona"), Some(CoreMemoryBlock::Persona));
        assert_eq!(CoreMemoryBlock::from_str("HUMAN"), Some(CoreMemoryBlock::Human));
        assert_eq!(CoreMemoryBlock::from_str("Memory"), Some(CoreMemoryBlock::Memory));
        assert_eq!(CoreMemoryBlock::from_str("invalid"), None);
    }
}
