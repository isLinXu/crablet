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
pub mod openclaw_executor;
pub mod atomic_installer;
pub mod signature;
pub mod environment;
pub mod installer_ui;
pub mod semantic_search;
pub mod version_manager;
pub mod interactive_wizard;
pub mod dev_tools;
pub mod trigger;
pub mod context;
pub mod discovery;

// Re-export core types
pub use registry::SkillRegistry;
pub use executor::SkillExecutor;
pub use dependency::SkillDependencies;
pub use composite::{CompositeSkill, CompositeExecutor, CompositionType, SkillNode, ErrorPolicy, RetryPolicy};
pub use chain::{SkillChain, SkillChainEngine, ChainStep, StepType, StepConnection, ChainConfig, ChainErrorPolicy};
pub use dsl::{WorkflowDefinition, WorkflowCompiler};
pub use orchestrator::{SkillOrchestrator, OrchestratorConfig, ExecutionRequest, ExecutionResponse, ExecutionStatus};
pub use visualization::{GraphExporter, GraphFormat};
pub use openclaw_executor::{OpenClawEngine, OpenClawResult, ExecutionContext};
pub use atomic_installer::{AtomicInstaller, InstallResult};
pub use signature::{SkillSignatureVerifier, VerificationResult};
pub use environment::{SkillEnvironment, VirtualEnv};
pub use installer_ui::{InstallProgress, SkillInfoDisplay, UserPrompt, ErrorDisplay, LogDisplay};
pub use semantic_search::{SkillSearchManager, SkillSearchResult, SkillSearchMetadata, SkillSearchIndex, SkillCategory, SearchQuery, SearchFilters, MatchType};
pub use semantic_matcher::{SemanticMatcher, SemanticMatch, SkillMetadata};
pub use version_manager::{VersionManager, SemVer, VersionConstraint, VersionDiff, UpdateInfo, UpdateStats};
pub use interactive_wizard::{InteractiveWizard, QuickInstallWizard, WizardStep, WizardState, SkillConfiguration, InstallOptions};
pub use dev_tools::{DevTools, InitResult, ValidationResult, TestResult, BuildResult, PublishResult, DocsResult};
pub use trigger::{SkillTrigger, SkillTriggerEngine, TriggerMatch};
pub use context::{SkillContext, ExecutionRecord, MemoryContext};

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
