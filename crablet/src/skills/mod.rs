use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

pub mod atomic_installer;
pub mod chain;
pub mod composite;
pub mod context;
pub mod dependency;
pub mod dev_tools;
pub mod discovery;
pub mod dsl;
pub mod environment;
pub mod executor;
pub mod hybrid_matcher;
pub mod installer;
pub mod installer_ui;
pub mod interactive_wizard;
pub mod openclaw;
pub mod openclaw_executor;
pub mod orchestrator;
pub mod registry;
pub mod semantic_matcher;
pub mod semantic_search;
pub mod signature;
pub mod trigger;
pub mod version_manager;
pub mod visualization;
pub mod watcher;

// Re-export core types
pub use atomic_installer::{AtomicInstaller, InstallResult};
pub use chain::{
    ChainConfig, ChainErrorPolicy, ChainStep, SkillChain, SkillChainEngine, StepConnection,
    StepType,
};
pub use composite::{
    CompositeExecutor, CompositeSkill, CompositionType, ErrorPolicy, RetryPolicy, SkillNode,
};
pub use context::{ExecutionRecord, MemoryContext, SkillContext};
pub use dependency::SkillDependencies;
pub use dev_tools::{
    BuildResult, DevTools, DocsResult, InitResult, PublishResult, TestResult, ValidationResult,
};
pub use dsl::{WorkflowCompiler, WorkflowDefinition};
pub use environment::{SkillEnvironment, VirtualEnv};
pub use executor::SkillExecutor;
pub use installer_ui::{ErrorDisplay, InstallProgress, LogDisplay, SkillInfoDisplay, UserPrompt};
pub use interactive_wizard::{
    InstallOptions, InteractiveWizard, QuickInstallWizard, SkillConfiguration, WizardState,
    WizardStep,
};
pub use openclaw_executor::{ExecutionContext, OpenClawEngine, OpenClawResult};
pub use orchestrator::{
    ExecutionRequest, ExecutionResponse, ExecutionStatus, OrchestratorConfig, SkillOrchestrator,
};
pub use registry::SkillRegistry;
pub use semantic_matcher::{SemanticMatch, SemanticMatcher, SkillMetadata};
pub use semantic_search::{
    MatchType, SearchFilters, SearchQuery, SkillCategory, SkillSearchIndex, SkillSearchManager,
    SkillSearchMetadata, SkillSearchResult,
};
pub use signature::{SkillSignatureVerifier, VerificationResult};
pub use trigger::{SkillTrigger, SkillTriggerEngine, TriggerMatch};
pub use version_manager::{
    SemVer, UpdateInfo, UpdateStats, VersionConstraint, VersionDiff, VersionManager,
};
pub use visualization::{GraphExporter, GraphFormat};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillParameter {
    pub name: String,
    pub description: String,
    pub param_type: String,
    pub required: bool,
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Vec<SkillParameter>,
    pub handler: String,
    pub examples: Vec<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    pub version: String,
    pub parameters: serde_json::Value, // JSON Schema for arguments
    pub entrypoint: String,            // Command to run (e.g., "python main.py")
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
    #[serde(default)]
    pub author: Option<String>, // Author of the skill
    /// Triggers for automatic skill activation
    #[serde(default)]
    pub triggers: Vec<SkillTrigger>,
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

impl std::fmt::Debug for SkillType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillType::Local(_) => write!(f, "SkillType::Local"),
            SkillType::Mcp(manifest, _, tool_name) => {
                write!(f, "SkillType::Mcp({:?}, _, {:?})", manifest.name, tool_name)
            }
            SkillType::Plugin(manifest, _) => {
                write!(f, "SkillType::Plugin({:?}, _)", manifest.name)
            }
            SkillType::OpenClaw(skill, _) => {
                write!(f, "SkillType::OpenClaw({:?}, _)", skill.manifest.name)
            }
        }
    }
}
