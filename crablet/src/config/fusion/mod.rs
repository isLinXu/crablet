//! Crablet + OpenClaw Fusion Configuration Module
//! 
//! This module provides bidirectional configuration management between
//! Markdown-based OpenClaw configs and Crablet's runtime state.

pub mod parser;

use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Fusion configuration combining OpenClaw style with Crablet features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionConfig {
    /// Configuration metadata
    pub metadata: ConfigMetadata,
    
    /// Agent identity (from AGENTS.md)
    pub agent: AgentConfig,
    
    /// Soul/Personality (from SOUL.md)
    pub soul: SoulConfig,
    
    /// User profile (from USER.md)
    pub user: UserConfig,
    
    /// Memory settings (from MEMORY.md)
    pub memory: MemoryConfig,
    
    /// Tools/Skills (from TOOLS.md)
    pub tools: ToolsConfig,
    
    /// Heartbeat/Scheduling (from HEARTBEAT.md)
    pub heartbeat: HeartbeatConfig,
    
    /// Engine settings (Crablet specific)
    pub engine: EngineConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMetadata {
    pub name: String,
    pub version: String,
    pub edition: String, // "openclaw", "crablet", "fusion"
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub identity: IdentityConfig,
    pub capabilities: CapabilitiesConfig,
    pub behavior: BehaviorConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityConfig {
    pub name: String,
    pub description: String,
    pub role: String,
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesConfig {
    pub rag: RagCapability,
    pub memory: MemoryCapability,
    pub cognitive: CognitiveCapability,
    pub skills: SkillsCapability,
    pub channels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagCapability {
    pub enabled: bool,
    pub backend: String, // "hybrid", "vector", "graph"
    pub vector_store: String,
    pub graph_store: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCapability {
    pub layers: u8, // 4 for fusion
    pub daily_logs: bool,
    pub consolidation: bool,
    pub cross_session: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitiveCapability {
    pub router: String, // "adaptive", "fixed", "random"
    pub system1: bool,
    pub system2: bool,
    pub system3: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsCapability {
    pub local: bool,
    pub mcp: bool,
    pub openclaw: bool,
    pub plugin: bool,
    pub hot_reload: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorConfig {
    pub proactivity: ProactivityLevel,
    pub response_style: ResponseStyle,
    pub safety_level: SafetyLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProactivityLevel {
    Passive,    // Wait for user input
    Balanced,   // Suggest when appropriate
    Active,     // Proactively offer help
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStyle {
    Concise,
    Balanced,
    Detailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafetyLevel {
    Strict,
    Balanced,
    Permissive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulConfig {
    pub personality: PersonalityConfig,
    pub values: Vec<ValueConfig>,
    pub principles: Vec<PrincipleConfig>,
    pub cognitive_profile: CognitiveProfileConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityConfig {
    pub name: String,
    pub traits: Vec<String>,
    pub communication_style: String,
    pub thinking_pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueConfig {
    pub name: String,
    pub priority: u8, // 1-10
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrincipleConfig {
    pub name: String,
    pub description: String,
    pub immutable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CognitiveProfileConfig {
    pub system1: System1Profile,
    pub system2: System2Profile,
    pub system3: System3Profile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct System1Profile {
    pub enabled: bool,
    pub intent_trie: String,
    pub fuzzy_matching: bool,
    pub openclaw_prompts: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct System2Profile {
    pub enabled: bool,
    pub react_engine: String,
    pub middleware_chain: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct System3Profile {
    pub enabled: bool,
    pub swarm_coordinator: String,
    pub max_agents: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    pub profile: UserProfileConfig,
    pub preferences: UserPreferencesConfig,
    pub privacy: PrivacyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfileConfig {
    pub user_id: String,
    pub name: Option<String>,
    pub role: Option<String>,
    pub expertise: Vec<String>,
    pub goals: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferencesConfig {
    pub language: String,
    pub response_length: ResponseLength,
    pub format_preferences: FormatPreferencesConfig,
    pub proactive_behavior: ProactiveBehaviorConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseLength {
    Minimal,
    Moderate,
    Detailed,
    Deep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatPreferencesConfig {
    pub use_markdown: bool,
    pub use_tables: bool,
    pub use_code_blocks: bool,
    pub use_emoji: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProactiveBehaviorConfig {
    pub suggest_related: bool,
    pub ask_clarification: bool,
    pub summarize_conversation: bool,
    pub recommend_next: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyConfig {
    pub store_history: bool,
    pub learn_preferences: bool,
    pub share_anonymous_data: bool,
    pub cross_session_identification: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub working: WorkingMemoryConfig,
    pub episodic: EpisodicMemoryConfig,
    pub semantic: SemanticMemoryConfig,
    pub daily_logs: DailyLogsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemoryConfig {
    pub capacity_messages: usize,
    pub max_tokens: usize,
    pub compression_strategy: String, // "remove", "summarize", "hybrid"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodicMemoryConfig {
    pub backend: String, // "sqlite", "postgres"
    pub database_url: String,
    pub retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMemoryConfig {
    pub backend: String, // "hybrid", "vector_only", "graph_only"
    pub vector_store: VectorStoreConfig,
    pub graph_store: Option<GraphStoreConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStoreConfig {
    pub provider: String, // "qdrant", "milvus", "pgvector"
    pub url: String,
    pub dimension: usize,
    pub distance: String, // "cosine", "euclidean", "dot"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStoreConfig {
    pub provider: String, // "neo4j", "sqlite"
    pub url: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyLogsConfig {
    pub enabled: bool,
    pub log_dir: PathBuf,
    pub format: String, // "markdown", "json"
    pub retention_days: u32,
    pub auto_extract_memories: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsConfig {
    pub registry: ToolRegistryConfig,
    pub permissions: ToolPermissionsConfig,
    pub orchestration: ToolOrchestrationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRegistryConfig {
    pub auto_load: bool,
    pub hot_reload: bool,
    pub scan_interval_secs: u64,
    pub skill_dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPermissionsConfig {
    pub default_allow: Vec<String>,
    pub default_deny: Vec<String>,
    pub require_confirmation: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOrchestrationConfig {
    pub max_parallel: usize,
    pub timeout_secs: u64,
    pub retry_attempts: u32,
    pub enable_chaining: bool,
    pub enable_composition: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    pub enabled: bool,
    pub timezone: String,
    pub tasks: HeartbeatTasksConfig,
    pub health_checks: HealthChecksConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatTasksConfig {
    pub daily: Vec<ScheduledTask>,
    pub weekly: Vec<ScheduledTask>,
    pub monthly: Vec<ScheduledTask>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub name: String,
    pub schedule: String, // cron expression
    pub action: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthChecksConfig {
    pub enabled: bool,
    pub interval_secs: u64,
    pub checks: Vec<HealthCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheck {
    pub name: String,
    pub check_type: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub alert_on_failure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    pub performance: PerformanceConfig,
    pub safety: SafetyConfig,
    pub observability: ObservabilityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub max_concurrent_sessions: usize,
    pub message_queue_size: usize,
    pub worker_threads: usize,
    pub enable_zero_copy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConfig {
    pub oracle_enabled: bool,
    pub sandbox_type: String, // "docker", "gvisor", "none"
    pub rate_limiting: RateLimitingConfig,
    pub content_filter: ContentFilterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    pub enabled: bool,
    pub requests_per_minute: u32,
    pub tokens_per_day: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentFilterConfig {
    pub enabled: bool,
    pub filter_level: String, // "strict", "moderate", "permissive"
    pub custom_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    pub logging: LoggingConfig,
    pub metrics: MetricsConfig,
    pub tracing: TracingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub endpoint: Option<String>,
    pub interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    pub enabled: bool,
    pub jaeger_endpoint: Option<String>,
    pub sampling_rate: f64,
}

impl FusionConfig {
    /// Create default fusion configuration
    pub fn default_fusion() -> Self {
        Self {
            metadata: ConfigMetadata {
                name: "Crablet".to_string(),
                version: "2.0.0".to_string(),
                edition: "fusion".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                updated_at: chrono::Utc::now().to_rfc3339(),
            },
            agent: AgentConfig {
                identity: IdentityConfig {
                    name: "Crablet".to_string(),
                    description: "Your intelligent assistant companion".to_string(),
                    role: "multimodal_ai_assistant".to_string(),
                    avatar: None,
                },
                capabilities: CapabilitiesConfig {
                    rag: RagCapability {
                        enabled: true,
                        backend: "hybrid".to_string(),
                        vector_store: "qdrant".to_string(),
                        graph_store: Some("neo4j".to_string()),
                    },
                    memory: MemoryCapability {
                        layers: 4,
                        daily_logs: true,
                        consolidation: true,
                        cross_session: true,
                    },
                    cognitive: CognitiveCapability {
                        router: "adaptive".to_string(),
                        system1: true,
                        system2: true,
                        system3: true,
                    },
                    skills: SkillsCapability {
                        local: true,
                        mcp: true,
                        openclaw: true,
                        plugin: true,
                        hot_reload: true,
                    },
                    channels: vec![
                        "web".to_string(),
                        "telegram".to_string(),
                    ],
                },
                behavior: BehaviorConfig {
                    proactivity: ProactivityLevel::Balanced,
                    response_style: ResponseStyle::Balanced,
                    safety_level: SafetyLevel::Balanced,
                },
            },
            soul: SoulConfig {
                personality: PersonalityConfig {
                    name: "小螃蟹".to_string(),
                    traits: vec![
                        "friendly".to_string(),
                        "professional".to_string(),
                        "curious".to_string(),
                    ],
                    communication_style: "adaptive".to_string(),
                    thinking_pattern: "analytical".to_string(),
                },
                values: vec![
                    ValueConfig {
                        name: "user_first".to_string(),
                        priority: 10,
                        description: "用户至上".to_string(),
                    },
                    ValueConfig {
                        name: "honesty".to_string(),
                        priority: 9,
                        description: "诚实透明".to_string(),
                    },
                ],
                principles: vec![
                    PrincipleConfig {
                        name: "do_no_harm".to_string(),
                        description: "绝不伤害".to_string(),
                        immutable: true,
                    },
                    PrincipleConfig {
                        name: "privacy_protection".to_string(),
                        description: "保护隐私".to_string(),
                        immutable: true,
                    },
                ],
                cognitive_profile: CognitiveProfileConfig {
                    system1: System1Profile {
                        enabled: true,
                        intent_trie: "builtin".to_string(),
                        fuzzy_matching: true,
                        openclaw_prompts: true,
                    },
                    system2: System2Profile {
                        enabled: true,
                        react_engine: "enhanced".to_string(),
                        middleware_chain: vec![
                            "safety".to_string(),
                            "cost_guard".to_string(),
                            "semantic_cache".to_string(),
                            "planning".to_string(),
                            "rag".to_string(),
                        ],
                    },
                    system3: System3Profile {
                        enabled: true,
                        swarm_coordinator: "default".to_string(),
                        max_agents: 100,
                    },
                },
            },
            user: UserConfig {
                profile: UserProfileConfig {
                    user_id: "default".to_string(),
                    name: None,
                    role: None,
                    expertise: vec![],
                    goals: vec![],
                },
                preferences: UserPreferencesConfig {
                    language: "zh-CN".to_string(),
                    response_length: ResponseLength::Moderate,
                    format_preferences: FormatPreferencesConfig {
                        use_markdown: true,
                        use_tables: true,
                        use_code_blocks: true,
                        use_emoji: false,
                    },
                    proactive_behavior: ProactiveBehaviorConfig {
                        suggest_related: true,
                        ask_clarification: true,
                        summarize_conversation: false,
                        recommend_next: false,
                    },
                },
                privacy: PrivacyConfig {
                    store_history: true,
                    learn_preferences: true,
                    share_anonymous_data: false,
                    cross_session_identification: false,
                },
            },
            memory: MemoryConfig {
                working: WorkingMemoryConfig {
                    capacity_messages: 20,
                    max_tokens: 8000,
                    compression_strategy: "hybrid".to_string(),
                },
                episodic: EpisodicMemoryConfig {
                    backend: "sqlite".to_string(),
                    database_url: "sqlite://./data/episodic.db".to_string(),
                    retention_days: 365,
                },
                semantic: SemanticMemoryConfig {
                    backend: "hybrid".to_string(),
                    vector_store: VectorStoreConfig {
                        provider: "qdrant".to_string(),
                        url: "http://localhost:6333".to_string(),
                        dimension: 1536,
                        distance: "cosine".to_string(),
                    },
                    graph_store: Some(GraphStoreConfig {
                        provider: "neo4j".to_string(),
                        url: "bolt://localhost:7687".to_string(),
                        username: Some("neo4j".to_string()),
                        password: Some("password".to_string()),
                    }),
                },
                daily_logs: DailyLogsConfig {
                    enabled: true,
                    log_dir: PathBuf::from("agent-workspace/memory"),
                    format: "markdown".to_string(),
                    retention_days: 90,
                    auto_extract_memories: true,
                },
            },
            tools: ToolsConfig {
                registry: ToolRegistryConfig {
                    auto_load: true,
                    hot_reload: true,
                    scan_interval_secs: 30,
                    skill_dirs: vec![
                        PathBuf::from("agent-workspace/skills"),
                        PathBuf::from("skills"),
                    ],
                },
                permissions: ToolPermissionsConfig {
                    default_allow: vec!["read".to_string(), "search".to_string()],
                    default_deny: vec!["execute".to_string(), "delete".to_string()],
                    require_confirmation: vec!["write".to_string(), "network".to_string()],
                },
                orchestration: ToolOrchestrationConfig {
                    max_parallel: 5,
                    timeout_secs: 30,
                    retry_attempts: 3,
                    enable_chaining: true,
                    enable_composition: true,
                },
            },
            heartbeat: HeartbeatConfig {
                enabled: true,
                timezone: "Asia/Shanghai".to_string(),
                tasks: HeartbeatTasksConfig {
                    daily: vec![
                        ScheduledTask {
                            name: "archive_logs".to_string(),
                            schedule: "0 0 * * *".to_string(),
                            action: "archive_daily_logs".to_string(),
                            enabled: true,
                        },
                        ScheduledTask {
                            name: "extract_memories".to_string(),
                            schedule: "0 2 * * *".to_string(),
                            action: "extract_memories".to_string(),
                            enabled: true,
                        },
                    ],
                    weekly: vec![
                        ScheduledTask {
                            name: "consolidate_memories".to_string(),
                            schedule: "0 1 * * 0".to_string(),
                            action: "consolidate_memories".to_string(),
                            enabled: true,
                        },
                    ],
                    monthly: vec![],
                },
                health_checks: HealthChecksConfig {
                    enabled: true,
                    interval_secs: 60,
                    checks: vec![
                        HealthCheck {
                            name: "database".to_string(),
                            check_type: "ping".to_string(),
                            parameters: HashMap::new(),
                            alert_on_failure: true,
                        },
                    ],
                },
            },
            engine: EngineConfig {
                performance: PerformanceConfig {
                    max_concurrent_sessions: 1000,
                    message_queue_size: 10000,
                    worker_threads: 8,
                    enable_zero_copy: true,
                },
                safety: SafetyConfig {
                    oracle_enabled: true,
                    sandbox_type: "docker".to_string(),
                    rate_limiting: RateLimitingConfig {
                        enabled: true,
                        requests_per_minute: 60,
                        tokens_per_day: 1000000,
                    },
                    content_filter: ContentFilterConfig {
                        enabled: true,
                        filter_level: "moderate".to_string(),
                        custom_rules: vec![],
                    },
                },
                observability: ObservabilityConfig {
                    logging: LoggingConfig {
                        level: "info".to_string(),
                        format: "json".to_string(),
                        output: "stdout".to_string(),
                    },
                    metrics: MetricsConfig {
                        enabled: true,
                        endpoint: Some("http://localhost:9090".to_string()),
                        interval_secs: 15,
                    },
                    tracing: TracingConfig {
                        enabled: true,
                        jaeger_endpoint: Some("http://localhost:14268".to_string()),
                        sampling_rate: 0.1,
                    },
                },
            },
        }
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ConfigValidationError> {
        // TODO: Implement comprehensive validation
        Ok(())
    }
    
    /// Merge with another config (for hot reload)
    pub fn merge(&mut self, other: FusionConfig) {
        // TODO: Implement smart merge logic
        *self = other;
    }
}

#[derive(Debug)]
pub struct ConfigValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ConfigValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Config validation error in {}: {}", self.field, self.message)
    }
}

impl std::error::Error for ConfigValidationError {}
