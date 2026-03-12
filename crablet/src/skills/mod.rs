use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

pub mod registry;
pub mod executor;
pub mod dependency;
pub mod openclaw;
pub mod installer;
pub mod watcher;
pub mod composite;
pub mod chain;
pub mod orchestrator;
pub mod dsl;
pub mod visualization;
pub mod semantic_matcher;
pub mod hybrid_matcher;

// Re-export core types
pub use registry::SkillRegistry;
pub use executor::SkillExecutor;
pub use dependency::SkillDependencies;
pub use composite::{CompositeSkill, CompositeExecutor, CompositionType, SkillNode, ErrorPolicy, RetryPolicy};
pub use chain::{SkillChain, SkillChainEngine, ChainStep, StepType, StepConnection, ChainConfig, ChainErrorPolicy};
pub use dsl::{WorkflowDefinition, WorkflowCompiler};
pub use orchestrator::{SkillOrchestrator, OrchestratorConfig, ExecutionRequest, ExecutionResponse, ExecutionStatus};
pub use visualization::{GraphExporter, GraphFormat};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    pub version: String,
    pub parameters: serde_json::Value, // JSON Schema for arguments
    pub entrypoint: String, // Command to run (e.g., "python main.py")
    #[serde(default)]
    pub env: HashMap<String, String>, // Environment variables
    #[serde(default)]
    pub requires: Vec<String>, // System dependencies (e.g., "python3", "ffmpeg")
    #[serde(default)]
    pub runtime: Option<String>, // e.g., "python3", "node"
    #[serde(default)]
    pub dependencies: Option<SkillDependencies>, // Package dependencies
    #[serde(default)]
    pub resources: Option<SkillResources>, // Resource limits
    #[serde(default)]
    pub permissions: Vec<String>, // Permissions (e.g., "network:google.com")
    #[serde(default)]
    pub conflicts: Vec<String>, // Conflicting skills
    #[serde(default)]
    pub min_crablet_version: Option<String>, // Minimum Crablet version
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkillResources {
    #[serde(default)]
    pub timeout: Option<String>, // e.g. "30s"
    #[serde(default)]
    pub memory: Option<String>, // e.g. "128MB" (Not enforced yet)
    #[serde(default)]
    pub network: bool, // (Not enforced yet)
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
