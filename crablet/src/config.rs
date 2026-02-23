use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use directories::ProjectDirs;
use anyhow::Result;
use std::collections::HashMap;
use keyring::Entry;
use tracing::{info, warn};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub database_url: String,
    pub skills_dir: PathBuf,
    pub model_name: String,
    pub log_level: String,
    pub mcp_servers: HashMap<String, McpServerConfig>,
    pub channels: Vec<String>,
    #[serde(skip)]
    pub openai_api_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Deserialize)]
struct ConfigFile {
    database_url: Option<String>,
    skills_dir: Option<PathBuf>,
    model_name: Option<String>,
    log_level: Option<String>,
    mcp_servers: Option<HashMap<String, McpServerConfig>>,
    channels: Option<Vec<String>>,
}

impl Config {
    pub fn load() -> Result<Self> {
        // Defaults
        let mut database_url = "sqlite:crablet.db?mode=rwc".to_string();
        let mut skills_dir = PathBuf::from("skills");
        let mut model_name = "gpt-4o-mini".to_string();
        let mut log_level = "info".to_string();
        let mut mcp_servers = HashMap::new();
        let mut channels = Vec::new();
        let mut openai_api_key = None;

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
            }
        }

        // 2. Override with Env Vars
        if let Ok(env_db) = std::env::var("DATABASE_URL") { database_url = env_db; }
        if let Ok(env_skills) = std::env::var("CRABLET_SKILLS_DIR") { skills_dir = PathBuf::from(env_skills); }
        if let Ok(env_model) = std::env::var("OPENAI_MODEL_NAME") { model_name = env_model; }
        if let Ok(env_log) = std::env::var("RUST_LOG") { log_level = env_log; }
        
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
            openai_api_key,
        })
    }
}
