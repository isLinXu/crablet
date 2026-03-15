use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use directories::ProjectDirs;
use anyhow::Result;
use std::collections::HashMap;
use keyring::Entry;
use tracing::{info, warn};
use std::sync::Arc;
use parking_lot::RwLock;
use notify::{Watcher, RecursiveMode, RecommendedWatcher};

use validator::Validate;

// Fusion configuration module (OpenClaw-style)
pub mod fusion;

#[derive(Clone, Debug, Serialize, Deserialize, Validate)]
pub struct Config {
    #[validate(url)]
    pub database_url: String,
    pub skills_dir: PathBuf,
    #[validate(length(min = 1))]
    pub model_name: String,
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
    
    // Auth Config
    #[validate(url)]
    pub oidc_issuer: Option<String>,
    pub oidc_client_id: Option<String>,
    pub oidc_client_secret: Option<String>,
    #[validate(length(min = 8))]
    pub jwt_secret: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: "sqlite:crablet.db?mode=rwc".to_string(),
            skills_dir: PathBuf::from("skills"),
            model_name: "gpt-4o-mini".to_string(),
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
            oidc_issuer: None,
            oidc_client_id: None,
            oidc_client_secret: None,
            jwt_secret: None,
        }
    }
}

fn default_ollama_model() -> String {
    "qwen2.5:14b".to_string()
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
}

#[derive(Deserialize)]
struct ConfigFile {
    database_url: Option<String>,
    skills_dir: Option<PathBuf>,
    model_name: Option<String>,
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
        // Defaults
        let mut database_url = "sqlite:crablet.db?mode=rwc".to_string();
        let mut skills_dir = PathBuf::from("skills");
        let mut model_name = "gpt-4o-mini".to_string();
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
        let mut oidc_issuer = None;
        let mut oidc_client_id = None;
        let mut oidc_client_secret = None;
        let mut jwt_secret = None;

        // 1. Try to load from XDG config
        if let Some(proj_dirs) = ProjectDirs::from("com", "crablet", "crablet") {
            let config_dir = proj_dirs.config_dir();
            let config_path = config_dir.join("config.toml");
            
            if config_path.exists() {
                let content = std::fs::read_to_string(&config_path)?;
                let toml_config: ConfigFile = toml::from_str(&content)?;
                
                if let Some(db) = toml_config.database_url { database_url = db; }
                if let Some(skills) = toml_config.skills_dir { skills_dir = skills; }
                if let Some(model) = toml_config.model_name { model_name = model; }
                if let Some(log) = toml_config.log_level { log_level = log; }
                if let Some(mcp) = toml_config.mcp_servers { mcp_servers = mcp; }
                if let Some(chans) = toml_config.channels { channels = chans; }
                if let Some(thresh) = toml_config.semantic_cache_threshold { semantic_cache_threshold = thresh; }
                if let Some(val) = toml_config.system2_threshold { system2_threshold = val; }
                if let Some(val) = toml_config.system3_threshold { system3_threshold = val; }
                if let Some(val) = toml_config.enable_adaptive_routing { enable_adaptive_routing = val; }
                if let Some(val) = toml_config.bandit_exploration { bandit_exploration = val; }
                if let Some(val) = toml_config.enable_hierarchical_reasoning { enable_hierarchical_reasoning = val; }
                if let Some(val) = toml_config.deliberate_threshold { deliberate_threshold = val; }
                if let Some(val) = toml_config.meta_reasoning_threshold { meta_reasoning_threshold = val; }
                if let Some(val) = toml_config.mcts_simulations { mcts_simulations = val; }
                if let Some(val) = toml_config.mcts_exploration_weight { mcts_exploration_weight = val; }
                if let Some(val) = toml_config.graph_rag_entity_mode { graph_rag_entity_mode = val; }
                if let Some(provs) = toml_config.providers { providers = provs; }
                if let Some(p) = toml_config.port { port = p; }
                if let Some(val) = toml_config.oidc_issuer { oidc_issuer = Some(val); }
                if let Some(val) = toml_config.oidc_client_id { oidc_client_id = Some(val); }
                if let Some(val) = toml_config.oidc_client_secret { oidc_client_secret = Some(val); }
                if let Some(val) = toml_config.jwt_secret { jwt_secret = Some(val); }
            }
        }

        // 1.5. Try to load from local config (overrides XDG)
        let local_config_path = PathBuf::from("config/config.toml");
        if local_config_path.exists() {
            info!("Loading local config from {:?}", local_config_path);
            let content = std::fs::read_to_string(&local_config_path)?;
            let toml_config: ConfigFile = toml::from_str(&content)?;
            
            if let Some(db) = toml_config.database_url { database_url = db; }
            if let Some(skills) = toml_config.skills_dir { skills_dir = skills; }
            if let Some(model) = toml_config.model_name { model_name = model; }
            if let Some(log) = toml_config.log_level { log_level = log; }
            if let Some(mcp) = toml_config.mcp_servers { mcp_servers = mcp; }
            if let Some(chans) = toml_config.channels { channels = chans; }
            if let Some(thresh) = toml_config.semantic_cache_threshold { semantic_cache_threshold = thresh; }
            if let Some(val) = toml_config.system2_threshold { system2_threshold = val; }
            if let Some(val) = toml_config.system3_threshold { system3_threshold = val; }
            if let Some(val) = toml_config.enable_adaptive_routing { enable_adaptive_routing = val; }
            if let Some(val) = toml_config.bandit_exploration { bandit_exploration = val; }
            if let Some(val) = toml_config.enable_hierarchical_reasoning { enable_hierarchical_reasoning = val; }
            if let Some(val) = toml_config.deliberate_threshold { deliberate_threshold = val; }
            if let Some(val) = toml_config.meta_reasoning_threshold { meta_reasoning_threshold = val; }
            if let Some(val) = toml_config.mcts_simulations { mcts_simulations = val; }
            if let Some(val) = toml_config.mcts_exploration_weight { mcts_exploration_weight = val; }
            if let Some(val) = toml_config.graph_rag_entity_mode { graph_rag_entity_mode = val; }
            if let Some(provs) = toml_config.providers { providers = provs; }
            if let Some(p) = toml_config.port { port = p; }
            if let Some(val) = toml_config.oidc_issuer { oidc_issuer = Some(val); }
            if let Some(val) = toml_config.oidc_client_id { oidc_client_id = Some(val); }
            if let Some(val) = toml_config.oidc_client_secret { oidc_client_secret = Some(val); }
            if let Some(val) = toml_config.jwt_secret { jwt_secret = Some(val); }
        }

        // 2. Override with Env Vars
        if let Ok(env_db) = std::env::var("DATABASE_URL") { database_url = env_db; }
        if let Ok(env_skills) = std::env::var("CRABLET_SKILLS_DIR") { skills_dir = PathBuf::from(env_skills); }
        if let Ok(env_model) = std::env::var("OPENAI_MODEL_NAME") { model_name = env_model; }
        if let Ok(env_ollama) = std::env::var("OLLAMA_MODEL") { ollama_model = env_ollama; }
        if let Ok(env_log) = std::env::var("RUST_LOG") { log_level = env_log; }
        if let Ok(env_serper) = std::env::var("SERPER_API_KEY") { serper_api_key = Some(env_serper); }
        if let Ok(env_feishu_id) = std::env::var("FEISHU_APP_ID") { feishu_app_id = Some(env_feishu_id); }
        if let Ok(env_feishu_secret) = std::env::var("FEISHU_APP_SECRET") { feishu_app_secret = Some(env_feishu_secret); }
        if let Ok(env_wecom_id) = std::env::var("WECOM_CORP_ID") { wecom_corp_id = Some(env_wecom_id); }
        if let Ok(env_wecom_secret) = std::env::var("WECOM_CORP_SECRET") { wecom_corp_secret = Some(env_wecom_secret); }
        if let Ok(env_wecom_agent) = std::env::var("WECOM_AGENT_ID") { wecom_agent_id = Some(env_wecom_agent); }
        
        if let Ok(val) = std::env::var("OIDC_ISSUER") { oidc_issuer = Some(val); }
        if let Ok(val) = std::env::var("OIDC_CLIENT_ID") { oidc_client_id = Some(val); }
        if let Ok(val) = std::env::var("OIDC_CLIENT_SECRET") { oidc_client_secret = Some(val); }
        if let Ok(val) = std::env::var("JWT_SECRET") { jwt_secret = Some(val); }
        
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
            let lower = val.to_lowercase();
            enable_adaptive_routing = matches!(lower.as_str(), "1" | "true" | "yes" | "on");
        }
        if let Ok(val) = std::env::var("BANDIT_EXPLORATION") {
            if let Ok(t) = val.parse::<f32>() {
                bandit_exploration = t;
            }
        }
        if let Ok(val) = std::env::var("ENABLE_HIERARCHICAL_REASONING") {
            let lower = val.to_lowercase();
            enable_hierarchical_reasoning = matches!(lower.as_str(), "1" | "true" | "yes" | "on");
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
                    },
                    Err(_) => {
                         // Not found in keyring, check env var
                         if let Ok(env_key) = std::env::var("OPENAI_API_KEY") {
                             openai_api_key = Some(env_key);
                         } else if let Ok(env_key) = std::env::var("DASHSCOPE_API_KEY") {
                             openai_api_key = Some(env_key);
                         }
                    }
                }
            },
            Err(e) => {
                warn!("Failed to access keyring: {}. Falling back to env vars.", e);
                if let Ok(env_key) = std::env::var("OPENAI_API_KEY") {
                    openai_api_key = Some(env_key);
                } else if let Ok(env_key) = std::env::var("DASHSCOPE_API_KEY") {
                    openai_api_key = Some(env_key);
                }
            }
        }

        // 4. Fallback for skills: if local "skills" dir doesn't exist, check XDG data dir
        if !skills_dir.exists() {
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
            ollama_model,
            oidc_issuer,
            oidc_client_id,
            oidc_client_secret,
            jwt_secret,
        })
    }

    pub fn validate(&self) -> Result<()> {
        Validate::validate(self).map_err(|e| anyhow::anyhow!("Configuration validation failed: {}", e))
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
                
                let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                    match res {
                       Ok(_) => {
                           // Reload config
                           info!("Config file changed. Reloading...");
                           match Config::load() {
                               Ok(new_config) => {
                                   let mut w = config_clone.write();
                                   *w = new_config;
                                   info!("Config reloaded successfully.");
                               },
                               Err(e) => {
                                   warn!("Failed to reload config: {}", e);
                               }
                           }
                       },
                       Err(e) => warn!("Watch error: {:?}", e),
                    }
                })?;
                
                watcher.watch(&config_path, RecursiveMode::NonRecursive)?;
                self._watcher = Some(watcher);
            }
        }
        Ok(())
    }
}
