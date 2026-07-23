use anyhow::Result;
use directories::ProjectDirs;
use keyring::Entry;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

use validator::Validate;

// Fusion configuration module (OpenClaw-style)
pub mod fusion;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DistributedHarnessSettings {
    #[serde(default)]
    pub enabled: bool,
    pub node_id: Option<String>,
    pub node_address: Option<String>,
    pub node_port: Option<u16>,
    pub backend: Option<String>,
    pub backend_uri: Option<String>,
    #[serde(default = "default_distributed_lock_ttl_secs")]
    pub lock_ttl_secs: u64,
    #[serde(default = "default_distributed_heartbeat_interval_secs")]
    pub heartbeat_interval_secs: u64,
    #[serde(default = "default_distributed_node_timeout_secs")]
    pub node_timeout_secs: u64,
    #[serde(default = "default_distributed_rpc_path")]
    pub rpc_path: String,
    pub rpc_bearer_token: Option<String>,
}

impl Default for DistributedHarnessSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            node_id: None,
            node_address: None,
            node_port: None,
            backend: None,
            backend_uri: None,
            lock_ttl_secs: default_distributed_lock_ttl_secs(),
            heartbeat_interval_secs: default_distributed_heartbeat_interval_secs(),
            node_timeout_secs: default_distributed_node_timeout_secs(),
            rpc_path: default_distributed_rpc_path(),
            rpc_bearer_token: None,
        }
    }
}

impl DistributedHarnessSettings {
    pub fn is_enabled(&self) -> bool {
        self.enabled
            || self
                .node_id
                .as_deref()
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
pub struct Config {
    #[validate(url)]
    pub database_url: String,
    pub skills_dir: PathBuf,
    #[validate(length(min = 1))]
    pub model_name: String,
    pub llm_vendor: Option<String>,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
    pub log_level: String,
    pub mcp_servers: HashMap<String, McpServerConfig>,
    pub channels: Vec<String>,
    #[serde(default = "default_semantic_threshold")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub semantic_cache_threshold: f32,
    #[serde(default = "default_system2_threshold")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub system2_threshold: f32,
    #[serde(default = "default_system3_threshold")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub system3_threshold: f32,
    #[serde(default)]
    pub enable_adaptive_routing: bool,
    #[serde(default = "default_bandit_exploration")]
    #[validate(range(min = 0.05, max = 2.0))]
    pub bandit_exploration: f32,
    #[serde(default = "default_enable_hierarchical_reasoning")]
    pub enable_hierarchical_reasoning: bool,
    #[serde(default = "default_deliberate_threshold")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub deliberate_threshold: f32,
    #[serde(default = "default_meta_reasoning_threshold")]
    #[validate(range(min = 0.0, max = 1.0))]
    pub meta_reasoning_threshold: f32,
    #[serde(default = "default_mcts_simulations")]
    #[validate(range(min = 1, max = 512))]
    pub mcts_simulations: u32,
    #[serde(default = "default_mcts_exploration_weight")]
    #[validate(range(min = 0.1, max = 3.0))]
    pub mcts_exploration_weight: f32,
    #[serde(default = "default_graph_rag_entity_mode")]
    pub graph_rag_entity_mode: String,
    #[serde(skip)]
    pub openai_api_key: Option<String>,
    #[serde(skip)]
    pub serper_api_key: Option<String>,
    #[serde(skip)]
    pub feishu_app_id: Option<String>,
    #[serde(skip)]
    pub feishu_app_secret: Option<String>,
    #[serde(skip)]
    pub wecom_corp_id: Option<String>,
    #[serde(skip)]
    pub wecom_corp_secret: Option<String>,
    #[serde(skip)]
    pub wecom_agent_id: Option<String>,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default = "default_port")]
    #[validate(range(min = 1))]
    pub port: u16,
    #[serde(default)]
    pub distributed_harness: DistributedHarnessSettings,

    // Auth Config
    #[validate(url)]
    pub oidc_issuer: Option<String>,
    pub oidc_client_id: Option<String>,
    pub oidc_client_secret: Option<String>,
    #[validate(length(min = 8))]
    pub jwt_secret: Option<String>,
}

impl Config {
    /// Create a deterministic test configuration without touching environment variables.
    /// This avoids test race conditions caused by `std::env::set_var`.
    pub fn for_test() -> Self {
        Self {
            openai_api_key: Some("sk-test".to_string()),
            ..Self::default()
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: "sqlite:crablet.db?mode=rwc".to_string(),
            skills_dir: PathBuf::from("skills"),
            model_name: "gpt-4o-mini".to_string(),
            llm_vendor: None,
            ollama_model: default_ollama_model(),
            log_level: "info".to_string(),
            mcp_servers: HashMap::new(),
            channels: Vec::new(),
            semantic_cache_threshold: default_semantic_threshold(),
            system2_threshold: default_system2_threshold(),
            system3_threshold: default_system3_threshold(),
            enable_adaptive_routing: false,
            bandit_exploration: default_bandit_exploration(),
            enable_hierarchical_reasoning: default_enable_hierarchical_reasoning(),
            deliberate_threshold: default_deliberate_threshold(),
            meta_reasoning_threshold: default_meta_reasoning_threshold(),
            mcts_simulations: default_mcts_simulations(),
            mcts_exploration_weight: default_mcts_exploration_weight(),
            graph_rag_entity_mode: default_graph_rag_entity_mode(),
            openai_api_key: None,
            serper_api_key: None,
            feishu_app_id: None,
            feishu_app_secret: None,
            wecom_corp_id: None,
            wecom_corp_secret: None,
            wecom_agent_id: None,
            providers: HashMap::new(),
            port: default_port(),
            distributed_harness: DistributedHarnessSettings::default(),
            oidc_issuer: None,
            oidc_client_id: None,
            oidc_client_secret: None,
            jwt_secret: None,
        }
    }
}

fn default_ollama_model() -> String {
    std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen3.6:latest".to_string())
}

fn default_semantic_threshold() -> f32 {
    0.92
}

fn default_system2_threshold() -> f32 {
    0.3
}

fn default_system3_threshold() -> f32 {
    0.7
}

fn default_bandit_exploration() -> f32 {
    0.55
}

fn default_enable_hierarchical_reasoning() -> bool {
    true
}

fn default_deliberate_threshold() -> f32 {
    0.58
}

fn default_meta_reasoning_threshold() -> f32 {
    0.82
}

fn default_mcts_simulations() -> u32 {
    24
}

fn default_mcts_exploration_weight() -> f32 {
    1.2
}

fn default_graph_rag_entity_mode() -> String {
    "hybrid".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_distributed_lock_ttl_secs() -> u64 {
    300
}

fn default_distributed_heartbeat_interval_secs() -> u64 {
    30
}

fn default_distributed_node_timeout_secs() -> u64 {
    60
}

fn default_distributed_rpc_path() -> String {
    "/rpc".to_string()
}

fn parse_bool_flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn sqlite_url(path: &Path) -> String {
    format!("sqlite://{}?mode=rwc", path.display())
}

fn migrate_if_absent(source: &Path, destination: &Path) -> Result<bool> {
    if !source.is_file() || destination.exists() {
        return Ok(false);
    }
    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::rename(source, destination) {
        Ok(()) => Ok(true),
        Err(_) => {
            std::fs::copy(source, destination)?;
            let copied = std::fs::metadata(destination)?.len();
            let original = std::fs::metadata(source)?.len();
            if copied != original {
                let _ = std::fs::remove_file(destination);
                anyhow::bail!("legacy database copy verification failed");
            }
            std::fs::remove_file(source)?;
            Ok(true)
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProviderConfig {
    pub api_key_env: Option<String>,
    pub base_url: Option<String>,
    pub models: Vec<String>,
    /// Per-model capability overrides. No provider capability is inferred.
    #[serde(default)]
    pub capabilities: HashMap<String, crate::cognitive::llm::capability::CapabilityDescriptor>,
    /// Deterministic model fallback order within this provider.
    #[serde(default)]
    pub fallback_order: Vec<String>,
}

#[derive(Deserialize)]
struct ConfigFile {
    database_url: Option<String>,
    skills_dir: Option<PathBuf>,
    model_name: Option<String>,
    llm_vendor: Option<String>,
    log_level: Option<String>,
    mcp_servers: Option<HashMap<String, McpServerConfig>>,
    channels: Option<Vec<String>>,
    semantic_cache_threshold: Option<f32>,
    system2_threshold: Option<f32>,
    system3_threshold: Option<f32>,
    enable_adaptive_routing: Option<bool>,
    bandit_exploration: Option<f32>,
    enable_hierarchical_reasoning: Option<bool>,
    deliberate_threshold: Option<f32>,
    meta_reasoning_threshold: Option<f32>,
    mcts_simulations: Option<u32>,
    mcts_exploration_weight: Option<f32>,
    graph_rag_entity_mode: Option<String>,
    providers: Option<HashMap<String, ProviderConfig>>,
    port: Option<u16>,
    distributed_harness: Option<DistributedHarnessSettings>,
    oidc_issuer: Option<String>,
    oidc_client_id: Option<String>,
    oidc_client_secret: Option<String>,
    jwt_secret: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config = Self::load_raw()?;
        config.validate()?;

        // Ensure skills dir exists
        std::fs::create_dir_all(&config.skills_dir).ok();

        Ok(config)
    }

    fn load_raw() -> Result<Self> {
        // 桌面包装器显式注入 CRABLET_DATA_DIR；CLI/服务模式保留相对路径默认值。
        // 这避免 sidecar 将持久数据写入只读 .app、DMG 卷或不确定 CWD。
        let desktop_data_dir = std::env::var_os("CRABLET_DATA_DIR").map(PathBuf::from);
        let mut database_url = desktop_data_dir
            .as_ref()
            .map(|root| sqlite_url(&root.join("db/crablet.db")))
            .unwrap_or_else(|| "sqlite:crablet.db?mode=rwc".to_string());
        let mut skills_dir = desktop_data_dir
            .as_ref()
            .map(|root| root.join("skills"))
            .unwrap_or_else(|| PathBuf::from("skills"));
        let mut model_name = "gpt-4o-mini".to_string();
        let mut llm_vendor = None;
        let mut ollama_model = default_ollama_model();
        let mut log_level = "info".to_string();
        let mut mcp_servers = HashMap::new();
        let mut channels = Vec::new();
        let mut semantic_cache_threshold = 0.92;
        let mut system2_threshold = default_system2_threshold();
        let mut system3_threshold = default_system3_threshold();
        let mut enable_adaptive_routing = false;
        let mut bandit_exploration = default_bandit_exploration();
        let mut enable_hierarchical_reasoning = default_enable_hierarchical_reasoning();
        let mut deliberate_threshold = default_deliberate_threshold();
        let mut meta_reasoning_threshold = default_meta_reasoning_threshold();
        let mut mcts_simulations = default_mcts_simulations();
        let mut mcts_exploration_weight = default_mcts_exploration_weight();
        let mut graph_rag_entity_mode = default_graph_rag_entity_mode();
        let mut openai_api_key = None;
        let mut serper_api_key = None;
        let mut feishu_app_id = None;
        let mut feishu_app_secret = None;
        let mut wecom_corp_id = None;
        let mut wecom_corp_secret = None;
        let mut wecom_agent_id = None;
        let mut providers = HashMap::new();
        let mut port = 3000;
        let mut distributed_harness = DistributedHarnessSettings::default();
        let mut oidc_issuer = None;
        let mut oidc_client_id = None;
        let mut oidc_client_secret = None;
        let mut jwt_secret = None;

        /// Apply a parsed ConfigFile onto mutable config variables.
        /// Called for each config source in priority order (lower → higher priority).
        macro_rules! apply_config_file {
            ($toml_config:expr) => {{
                let c = $toml_config;
                if let Some(v) = c.database_url {
                    database_url = v;
                }
                if let Some(v) = c.skills_dir {
                    skills_dir = v;
                }
                if let Some(v) = c.model_name {
                    model_name = v;
                }
                if let Some(v) = c.llm_vendor {
                    llm_vendor = Some(v);
                }
                if let Some(v) = c.log_level {
                    log_level = v;
                }
                if let Some(v) = c.mcp_servers {
                    mcp_servers = v;
                }
                if let Some(v) = c.channels {
                    channels = v;
                }
                if let Some(v) = c.semantic_cache_threshold {
                    semantic_cache_threshold = v;
                }
                if let Some(v) = c.system2_threshold {
                    system2_threshold = v;
                }
                if let Some(v) = c.system3_threshold {
                    system3_threshold = v;
                }
                if let Some(v) = c.enable_adaptive_routing {
                    enable_adaptive_routing = v;
                }
                if let Some(v) = c.bandit_exploration {
                    bandit_exploration = v;
                }
                if let Some(v) = c.enable_hierarchical_reasoning {
                    enable_hierarchical_reasoning = v;
                }
                if let Some(v) = c.deliberate_threshold {
                    deliberate_threshold = v;
                }
                if let Some(v) = c.meta_reasoning_threshold {
                    meta_reasoning_threshold = v;
                }
                if let Some(v) = c.mcts_simulations {
                    mcts_simulations = v;
                }
                if let Some(v) = c.mcts_exploration_weight {
                    mcts_exploration_weight = v;
                }
                if let Some(v) = c.graph_rag_entity_mode {
                    graph_rag_entity_mode = v;
                }
                if let Some(v) = c.providers {
                    providers = v;
                }
                if let Some(v) = c.port {
                    port = v;
                }
                if let Some(v) = c.distributed_harness {
                    distributed_harness = v;
                }
                if let Some(v) = c.oidc_issuer {
                    oidc_issuer = Some(v);
                }
                if let Some(v) = c.oidc_client_id {
                    oidc_client_id = Some(v);
                }
                if let Some(v) = c.oidc_client_secret {
                    oidc_client_secret = Some(v);
                }
                if let Some(v) = c.jwt_secret {
                    jwt_secret = Some(v);
                }
            }};
        }

        // Helper: load and parse a TOML config file from the given path.
        let load_toml = |path: &std::path::Path| -> Result<Option<ConfigFile>> {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                Ok(Some(toml::from_str(&content)?))
            } else {
                Ok(None)
            }
        };

        // 1. Try to load from XDG config
        if let Some(proj_dirs) = ProjectDirs::from("com", "crablet", "crablet") {
            let config_path = proj_dirs.config_dir().join("config.toml");
            if let Some(toml_config) = load_toml(&config_path)? {
                apply_config_file!(toml_config);
            }
        }

        // 1.5. Desktop config lives under the injected data root; CLI keeps local compatibility.
        let local_config_path = desktop_data_dir
            .as_ref()
            .map(|root| root.join("config/config.toml"))
            .unwrap_or_else(|| PathBuf::from("config/config.toml"));
        if let Some(toml_config) = load_toml(&local_config_path)? {
            info!("Loading local config from {:?}", local_config_path);
            apply_config_file!(toml_config);
        }

        // 2. Override with Env Vars
        if let Ok(env_db) = std::env::var("DATABASE_URL") {
            database_url = env_db;
        }
        if let Ok(env_skills) = std::env::var("CRABLET_SKILLS_DIR") {
            skills_dir = PathBuf::from(env_skills);
        }
        if let Ok(env_model) = std::env::var("OPENAI_MODEL_NAME") {
            model_name = env_model;
        }
        if let Ok(env_vendor) = std::env::var("LLM_VENDOR") {
            llm_vendor = Some(env_vendor);
        }
        if let Ok(env_ollama) = std::env::var("OLLAMA_MODEL") {
            ollama_model = env_ollama;
        }
        if let Ok(env_log) = std::env::var("RUST_LOG") {
            log_level = env_log;
        }
        if let Ok(env_serper) = std::env::var("SERPER_API_KEY") {
            serper_api_key = Some(env_serper);
        }
        if let Ok(env_feishu_id) = std::env::var("FEISHU_APP_ID") {
            feishu_app_id = Some(env_feishu_id);
        }
        if let Ok(env_feishu_secret) = std::env::var("FEISHU_APP_SECRET") {
            feishu_app_secret = Some(env_feishu_secret);
        }
        if let Ok(env_wecom_id) = std::env::var("WECOM_CORP_ID") {
            wecom_corp_id = Some(env_wecom_id);
        }
        if let Ok(env_wecom_secret) = std::env::var("WECOM_CORP_SECRET") {
            wecom_corp_secret = Some(env_wecom_secret);
        }
        if let Ok(env_wecom_agent) = std::env::var("WECOM_AGENT_ID") {
            wecom_agent_id = Some(env_wecom_agent);
        }

        if let Ok(val) = std::env::var("OIDC_ISSUER") {
            oidc_issuer = Some(val);
        }
        if let Ok(val) = std::env::var("OIDC_CLIENT_ID") {
            oidc_client_id = Some(val);
        }
        if let Ok(val) = std::env::var("OIDC_CLIENT_SECRET") {
            oidc_client_secret = Some(val);
        }
        if let Ok(val) = std::env::var("JWT_SECRET") {
            jwt_secret = Some(val);
        }
        if let Ok(val) = std::env::var("CRABLET_DISTRIBUTED_ENABLED") {
            distributed_harness.enabled = parse_bool_flag(&val);
        }
        if let Ok(val) = std::env::var("CRABLET_DISTRIBUTED_NODE_ID") {
            distributed_harness.node_id = Some(val);
        }
        if let Ok(val) = std::env::var("CRABLET_DISTRIBUTED_NODE_ADDRESS") {
            distributed_harness.node_address = Some(val);
        }
        if let Ok(val) = std::env::var("CRABLET_DISTRIBUTED_NODE_PORT") {
            if let Ok(port_value) = val.parse::<u16>() {
                distributed_harness.node_port = Some(port_value);
            }
        }
        if let Ok(val) = std::env::var("CRABLET_DISTRIBUTED_BACKEND") {
            distributed_harness.backend = Some(val);
        }
        if let Ok(val) = std::env::var("CRABLET_DISTRIBUTED_BACKEND_URI") {
            distributed_harness.backend_uri = Some(val);
        }
        if let Ok(val) = std::env::var("CRABLET_DISTRIBUTED_LOCK_TTL_SECS") {
            if let Ok(ttl) = val.parse::<u64>() {
                distributed_harness.lock_ttl_secs = ttl;
            }
        }
        if let Ok(val) = std::env::var("CRABLET_DISTRIBUTED_HEARTBEAT_INTERVAL_SECS") {
            if let Ok(interval) = val.parse::<u64>() {
                distributed_harness.heartbeat_interval_secs = interval;
            }
        }
        if let Ok(val) = std::env::var("CRABLET_DISTRIBUTED_NODE_TIMEOUT_SECS") {
            if let Ok(timeout) = val.parse::<u64>() {
                distributed_harness.node_timeout_secs = timeout;
            }
        }
        if let Ok(val) = std::env::var("CRABLET_DISTRIBUTED_RPC_PATH") {
            distributed_harness.rpc_path = val;
        }
        if let Ok(val) = std::env::var("CRABLET_DISTRIBUTED_RPC_BEARER_TOKEN") {
            distributed_harness.rpc_bearer_token = Some(val);
        }

        if let Ok(env_thresh) = std::env::var("SEMANTIC_CACHE_THRESHOLD") {
            if let Ok(t) = env_thresh.parse::<f32>() {
                semantic_cache_threshold = t;
            }
        }
        if let Ok(val) = std::env::var("SYSTEM2_THRESHOLD") {
            if let Ok(t) = val.parse::<f32>() {
                system2_threshold = t;
            }
        }
        if let Ok(val) = std::env::var("SYSTEM3_THRESHOLD") {
            if let Ok(t) = val.parse::<f32>() {
                system3_threshold = t;
            }
        }
        if let Ok(val) = std::env::var("ENABLE_ADAPTIVE_ROUTING") {
            enable_adaptive_routing = parse_bool_flag(&val);
        }
        if let Ok(val) = std::env::var("BANDIT_EXPLORATION") {
            if let Ok(t) = val.parse::<f32>() {
                bandit_exploration = t;
            }
        }
        if let Ok(val) = std::env::var("ENABLE_HIERARCHICAL_REASONING") {
            enable_hierarchical_reasoning = parse_bool_flag(&val);
        }
        if let Ok(val) = std::env::var("DELIBERATE_THRESHOLD") {
            if let Ok(t) = val.parse::<f32>() {
                deliberate_threshold = t;
            }
        }
        if let Ok(val) = std::env::var("META_REASONING_THRESHOLD") {
            if let Ok(t) = val.parse::<f32>() {
                meta_reasoning_threshold = t;
            }
        }
        if let Ok(val) = std::env::var("MCTS_SIMULATIONS") {
            if let Ok(t) = val.parse::<u32>() {
                mcts_simulations = t;
            }
        }
        if let Ok(val) = std::env::var("MCTS_EXPLORATION_WEIGHT") {
            if let Ok(t) = val.parse::<f32>() {
                mcts_exploration_weight = t;
            }
        }
        if let Ok(val) = std::env::var("GRAPH_RAG_ENTITY_MODE") {
            graph_rag_entity_mode = val;
        }

        // 3. Try Keyring for API Key
        // Try to get API key from secure storage
        match Entry::new("crablet", "openai_api_key") {
            Ok(entry) => {
                match entry.get_password() {
                    Ok(pwd) => {
                        info!("Loaded OpenAI API key from system keyring");
                        openai_api_key = Some(pwd);
                    }
                    Err(_) => {
                        // Not found in keyring, check env var
                        if let Ok(env_key) = std::env::var("OPENAI_API_KEY") {
                            openai_api_key = Some(env_key);
                        } else if let Ok(env_key) = std::env::var("DASHSCOPE_API_KEY") {
                            openai_api_key = Some(env_key);
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to access keyring: {}. Falling back to env vars.", e);
                if let Ok(env_key) = std::env::var("OPENAI_API_KEY") {
                    openai_api_key = Some(env_key);
                } else if let Ok(env_key) = std::env::var("DASHSCOPE_API_KEY") {
                    openai_api_key = Some(env_key);
                }
            }
        }

        if let Some(root) = desktop_data_dir.as_ref() {
            for subdir in ["db", "config", "skills", "uploads", "logs"] {
                std::fs::create_dir_all(root.join(subdir))?;
            }
            if std::env::var_os("DATABASE_URL").is_none() {
                let destination = root.join("db/crablet.db");
                for source in [PathBuf::from("crablet.db"), root.join("crablet.db")] {
                    if migrate_if_absent(&source, &destination)? {
                        info!("Migrated legacy database {:?} to {:?}", source, destination);
                        break;
                    }
                }
                database_url = sqlite_url(&destination);
            }
        } else if !skills_dir.exists() {
            // CLI compatibility: retain the existing XDG fallback.
            if let Some(proj_dirs) = ProjectDirs::from("com", "crablet", "crablet") {
                let data_dir = proj_dirs.data_dir().join("skills");
                if data_dir.exists() {
                    skills_dir = data_dir;
                }
            }
        }

        Ok(Self {
            database_url,
            skills_dir,
            model_name,
            llm_vendor,
            log_level,
            mcp_servers,
            channels,
            semantic_cache_threshold,
            system2_threshold,
            system3_threshold,
            enable_adaptive_routing,
            bandit_exploration,
            enable_hierarchical_reasoning,
            deliberate_threshold,
            meta_reasoning_threshold,
            mcts_simulations,
            mcts_exploration_weight,
            graph_rag_entity_mode,
            openai_api_key,
            serper_api_key,
            feishu_app_id,
            feishu_app_secret,
            wecom_corp_id,
            wecom_corp_secret,
            wecom_agent_id,
            providers,
            port,
            distributed_harness,
            ollama_model,
            oidc_issuer,
            oidc_client_id,
            oidc_client_secret,
            jwt_secret,
        })
    }

    pub fn validate(&self) -> Result<()> {
        Validate::validate(self)
            .map_err(|e| anyhow::anyhow!("Configuration validation failed: {}", e))?;

        if self.distributed_harness.enabled
            && self
                .distributed_harness
                .node_id
                .as_deref()
                .map(|value| value.trim().is_empty())
                .unwrap_or(true)
        {
            return Err(anyhow::anyhow!(
                "Configuration validation failed: distributed_harness.node_id is required when distributed_harness.enabled is true"
            ));
        }

        Ok(())
    }
}

// Hot Reload Support
pub struct ConfigManager {
    config: Arc<RwLock<Config>>,
    _watcher: Option<RecommendedWatcher>,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        let config_arc = Arc::new(RwLock::new(config));

        let mut manager = Self {
            config: config_arc,
            _watcher: None,
        };

        manager.setup_watcher()?;
        Ok(manager)
    }

    pub fn get_config(&self) -> Config {
        self.config.read().clone()
    }

    fn setup_watcher(&mut self) -> Result<()> {
        if let Some(proj_dirs) = ProjectDirs::from("com", "crablet", "crablet") {
            let config_path = proj_dirs.config_dir().join("config.toml");
            if config_path.exists() {
                let config_clone = self.config.clone();

                let mut watcher = notify::recommended_watcher(
                    move |res: Result<notify::Event, notify::Error>| {
                        match res {
                            Ok(_) => {
                                // Reload config
                                info!("Config file changed. Reloading...");
                                match Config::load() {
                                    Ok(new_config) => {
                                        let mut w = config_clone.write();
                                        *w = new_config;
                                        info!("Config reloaded successfully.");
                                    }
                                    Err(e) => {
                                        warn!("Failed to reload config: {}", e);
                                    }
                                }
                            }
                            Err(e) => warn!("Watch error: {:?}", e),
                        }
                    },
                )?;

                watcher.watch(&config_path, RecursiveMode::NonRecursive)?;
                self._watcher = Some(watcher);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod path_tests {
    use super::{migrate_if_absent, sqlite_url};
    use std::fs;

    #[test]
    fn sqlite_url_uses_absolute_path() {
        assert_eq!(
            sqlite_url(std::path::Path::new("/Users/alice/data/db/crablet.db")),
            "sqlite:///Users/alice/data/db/crablet.db?mode=rwc"
        );
    }

    #[test]
    fn legacy_database_migration_is_idempotent_and_never_overwrites() {
        let temp = std::env::temp_dir().join(format!(
            "crablet-path-test-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("migration")
        ));
        let source = temp.join("legacy/crablet.db");
        let destination = temp.join("db/crablet.db");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"legacy-data").unwrap();

        assert!(migrate_if_absent(&source, &destination).unwrap());
        assert_eq!(fs::read(&destination).unwrap(), b"legacy-data");
        assert!(!source.exists());
        assert!(!migrate_if_absent(&source, &destination).unwrap());

        fs::write(&source, b"must-not-overwrite").unwrap();
        assert!(!migrate_if_absent(&source, &destination).unwrap());
        assert_eq!(fs::read(&destination).unwrap(), b"legacy-data");
        fs::remove_dir_all(temp).unwrap();
    }
}
