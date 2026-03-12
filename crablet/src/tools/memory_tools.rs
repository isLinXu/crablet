//! Memory Functions - Tools for Agent to manage memory autonomously
//!
//! These tools implement the MemGPT-style memory management functions:
//! - `core_memory_append`: Append content to Core Memory blocks
//! - `core_memory_replace`: Replace content in Core Memory
//! - `conversation_search`: Search through conversation history
//! - `archival_memory_search`: Search through long-term memory
//! - `archival_memory_insert`: Insert new long-term memories
//!
//! # Usage
//!
//! These tools are automatically registered with SkillRegistry and can be
//! called by the Agent through Function Calling during ReAct execution.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::warn;
#[cfg(feature = "knowledge")]
use tracing::info;

use crate::plugins::Plugin;
use crate::memory::core::CoreMemoryBlock;
use crate::events::{AgentEvent, EventBus};

/// Plugin for appending content to Core Memory blocks
pub struct CoreMemoryAppendPlugin {
    memory_manager: Arc<crate::memory::manager::MemoryManager>,
    event_bus: Arc<EventBus>,
}

impl CoreMemoryAppendPlugin {
    pub fn new(
        memory_manager: Arc<crate::memory::manager::MemoryManager>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self { memory_manager, event_bus }
    }
}

#[async_trait]
impl Plugin for CoreMemoryAppendPlugin {
    fn name(&self) -> &str {
        "core_memory_append"
    }

    fn description(&self) -> &str {
        "Append content to a core memory block (persona/human/memory). \
         Core memory is always visible to you. Use this to store important information \
         that should be remembered across conversations."
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let block_str = args.get("block")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'block' parameter"))?;
        
        let content = args.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'content' parameter"))?;

        let block = CoreMemoryBlock::from_str(block_str)
            .ok_or_else(|| anyhow!("Invalid block '{}'. Must be 'persona', 'human', or 'memory'", block_str))?;

        match self.memory_manager.core_memory_append(block, content).await {
            Ok(added) => {
                // Publish event
                self.event_bus.publish(AgentEvent::CoreMemoryUpdated {
                    block: block_str.to_string(),
                    operation: "append".to_string(),
                    timestamp: chrono::Utc::now(),
                });

                let core = self.memory_manager.get_core_memory().await;
                Ok(format!(
                    "Successfully appended {} chars to '{}' block. Block now has {}/{} chars used.",
                    added,
                    block_str,
                    core.get(block).len(),
                    core.get_limit(block)
                ))
            }
            Err(e) => {
                warn!("Core memory append failed: {}", e);
                Ok(format!("Failed to append to core memory: {}", e))
            }
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Plugin for replacing content in Core Memory blocks
pub struct CoreMemoryReplacePlugin {
    memory_manager: Arc<crate::memory::manager::MemoryManager>,
    event_bus: Arc<EventBus>,
}

impl CoreMemoryReplacePlugin {
    pub fn new(
        memory_manager: Arc<crate::memory::manager::MemoryManager>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self { memory_manager, event_bus }
    }
}

#[async_trait]
impl Plugin for CoreMemoryReplacePlugin {
    fn name(&self) -> &str {
        "core_memory_replace"
    }

    fn description(&self) -> &str {
        "Replace content in a core memory block. If old_content is empty, replaces the entire block. \
         Use this to update or correct existing information in your core memory."
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let block_str = args.get("block")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'block' parameter"))?;
        
        let old_content = args.get("old_content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        
        let new_content = args.get("new_content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'new_content' parameter"))?;

        let block = CoreMemoryBlock::from_str(block_str)
            .ok_or_else(|| anyhow!("Invalid block '{}'. Must be 'persona', 'human', or 'memory'", block_str))?;

        match self.memory_manager.core_memory_replace(block, old_content, new_content).await {
            Ok(replaced) => {
                if replaced {
                    self.event_bus.publish(AgentEvent::CoreMemoryUpdated {
                        block: block_str.to_string(),
                        operation: "replace".to_string(),
                        timestamp: chrono::Utc::now(),
                    });

                    let core = self.memory_manager.get_core_memory().await;
                    Ok(format!(
                        "Successfully replaced content in '{}' block. Block now has {}/{} chars used.",
                        block_str,
                        core.get(block).len(),
                        core.get_limit(block)
                    ))
                } else {
                    Ok(format!("Content not found in '{}' block. No changes made.", block_str))
                }
            }
            Err(e) => {
                warn!("Core memory replace failed: {}", e);
                Ok(format!("Failed to replace in core memory: {}", e))
            }
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Plugin for searching conversation history
pub struct ConversationSearchPlugin {
    memory_manager: Arc<crate::memory::manager::MemoryManager>,
}

impl ConversationSearchPlugin {
    pub fn new(memory_manager: Arc<crate::memory::manager::MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[async_trait]
impl Plugin for ConversationSearchPlugin {
    fn name(&self) -> &str {
        "conversation_search"
    }

    fn description(&self) -> &str {
        "Search through past conversation history for relevant information. \
         Returns matching messages from previous conversations."
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let query = args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;

        let page = args.get("page")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        // Search through episodic memory if available
        if let Some(ref episodic) = self.memory_manager.episodic {
            // Use string search on messages
            match episodic.search_messages(query, 10, page).await {
                Ok(results) => {
                    if results.is_empty() {
                        Ok(format!("No conversations found matching '{}'", query))
                    } else {
                        let formatted: Vec<String> = results.iter()
                            .map(|(_session_id, role, content, ts)| {
                                format!("[{}] {}: {}", ts.format("%Y-%m-%d %H:%M"), role, content)
                            })
                            .collect();
                        
                        Ok(format!(
                            "Found {} conversations matching '{}':\n{}",
                            results.len(),
                            query,
                            formatted.join("\n")
                        ))
                    }
                }
                Err(e) => {
                    warn!("Conversation search failed: {}", e);
                    Ok(format!("Search failed: {}", e))
                }
            }
        } else {
            Ok("Episodic memory not available. Cannot search conversations.".to_string())
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Plugin for searching archival (long-term) memory
pub struct ArchivalMemorySearchPlugin {
    #[cfg(feature = "knowledge")]
    vector_store: Option<Arc<crate::knowledge::vector_store::VectorStore>>,
    memory_manager: Arc<crate::memory::manager::MemoryManager>,
}

impl ArchivalMemorySearchPlugin {
    #[cfg(feature = "knowledge")]
    pub fn new(
        vector_store: Option<Arc<crate::knowledge::vector_store::VectorStore>>,
        memory_manager: Arc<crate::memory::manager::MemoryManager>,
    ) -> Self {
        Self { vector_store, memory_manager }
    }

    #[cfg(not(feature = "knowledge"))]
    pub fn new(memory_manager: Arc<crate::memory::manager::MemoryManager>) -> Self {
        Self { memory_manager }
    }
}

#[async_trait]
impl Plugin for ArchivalMemorySearchPlugin {
    fn name(&self) -> &str {
        "archival_memory_search"
    }

    fn description(&self) -> &str {
        "Search through archival (long-term) memory using semantic search. \
         This searches through summarized memories and knowledge stored for long-term retention."
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let query = args.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'query' parameter"))?;

        let count = args.get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        #[cfg(feature = "knowledge")]
        {
            if let Some(ref vs) = self.vector_store {
                match vs.search(query, count).await {
                    Ok(results) => {
                        if results.is_empty() {
                            Ok(format!("No archival memories found matching '{}'", query))
                        } else {
                            let formatted: Vec<String> = results.iter()
                                .enumerate()
                                .map(|(i, doc)| {
                                    format!("{}. {} (score: {:.3})", i + 1, doc.0, doc.1)
                                })
                                .collect();
                            
                            Ok(format!(
                                "Found {} archival memories matching '{}':\n{}",
                                results.len(),
                                query,
                                formatted.join("\n")
                            ))
                        }
                    }
                    Err(e) => {
                        warn!("Archival memory search failed: {}", e);
                        Ok(format!("Search failed: {}", e))
                    }
                }
            } else {
                Ok("Archival memory (VectorStore) not available.".to_string())
            }
        }

        #[cfg(not(feature = "knowledge"))]
        {
            Ok("Archival memory search requires 'knowledge' feature to be enabled.".to_string())
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Plugin for inserting new archival memories
pub struct ArchivalMemoryInsertPlugin {
    #[cfg(feature = "knowledge")]
    vector_store: Option<Arc<crate::knowledge::vector_store::VectorStore>>,
    event_bus: Arc<EventBus>,
}

impl ArchivalMemoryInsertPlugin {
    #[cfg(feature = "knowledge")]
    pub fn new(
        vector_store: Option<Arc<crate::knowledge::vector_store::VectorStore>>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self { vector_store, event_bus }
    }

    #[cfg(not(feature = "knowledge"))]
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self { event_bus }
    }
}

#[async_trait]
impl Plugin for ArchivalMemoryInsertPlugin {
    fn name(&self) -> &str {
        "archival_memory_insert"
    }

    fn description(&self) -> &str {
        "Insert a new memory into archival (long-term) storage. \
         Use this to store important facts, insights, or information for long-term retention."
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let content = args.get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing 'content' parameter"))?;

        let importance = args.get("importance")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0) as f32;

        #[cfg(feature = "knowledge")]
        {
            if let Some(ref vs) = self.vector_store {
                let metadata = json!({
                    "type": "user_inserted",
                    "importance": importance,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                });

                match vs.add_document(content, Some(metadata)).await {
                    Ok(_) => {
                        info!("Inserted new archival memory");
                        Ok(format!("Successfully inserted memory into archival storage: '{}'", 
                            if content.len() > 100 { &content[..100] } else { content }))
                    }
                    Err(e) => {
                        warn!("Failed to insert archival memory: {}", e);
                        Ok(format!("Failed to insert memory: {}", e))
                    }
                }
            } else {
                Ok("Archival memory (VectorStore) not available.".to_string())
            }
        }

        #[cfg(not(feature = "knowledge"))]
        {
            Ok("Archival memory insert requires 'knowledge' feature to be enabled.".to_string())
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Helper function to get tool definitions for all memory tools
pub fn get_memory_tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "core_memory_append",
                "description": "Append content to a core memory block (persona/human/memory). Core memory is always visible to you.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "block": {
                            "type": "string",
                            "enum": ["persona", "human", "memory"],
                            "description": "Which memory block to append to"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to append"
                        }
                    },
                    "required": ["block", "content"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "core_memory_replace",
                "description": "Replace content in a core memory block. If old_content is empty, replaces entire block.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "block": {
                            "type": "string",
                            "enum": ["persona", "human", "memory"],
                            "description": "Which memory block to modify"
                        },
                        "old_content": {
                            "type": "string",
                            "description": "Content to replace (empty = replace all)"
                        },
                        "new_content": {
                            "type": "string",
                            "description": "New content"
                        }
                    },
                    "required": ["block", "new_content"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "conversation_search",
                "description": "Search through past conversation history for relevant information.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query"
                        },
                        "page": {
                            "type": "integer",
                            "description": "Page number for pagination (default: 0)"
                        }
                    },
                    "required": ["query"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "archival_memory_search",
                "description": "Search through archival (long-term) memory using semantic search.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query"
                        },
                        "count": {
                            "type": "integer",
                            "description": "Number of results to return (default: 5)"
                        }
                    },
                    "required": ["query"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "archival_memory_insert",
                "description": "Insert a new memory into archival (long-term) storage.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "content": {
                            "type": "string",
                            "description": "Memory content to store"
                        },
                        "importance": {
                            "type": "number",
                            "description": "Importance score 0.0-1.0 (default: 1.0)"
                        }
                    },
                    "required": ["content"]
                }
            }
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definitions() {
        let defs = get_memory_tool_definitions();
        assert_eq!(defs.len(), 5);
        
        // Check each tool has required fields
        for def in defs {
            assert!(def.get("type").is_some());
            assert!(def.get("function").is_some());
            let func = def.get("function").unwrap();
            assert!(func.get("name").is_some());
            assert!(func.get("description").is_some());
            assert!(func.get("parameters").is_some());
        }
    }
}
