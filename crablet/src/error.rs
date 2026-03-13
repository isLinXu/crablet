use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrabletError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Memory error: {0}")]
    Memory(String),

    #[error("Cognitive system error: {0}")]
    Cognitive(String),

    #[error("LLM client error: {0}")]
    Llm(String),

    #[error("Agent execution error: {0}")]
    Agent(String),

    #[error("Swarm orchestration error: {0}")]
    Swarm(String),

    #[error("Tool execution error: {0}")]
    Tool(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("RPA error: {0}")]
    RpaError(String),

    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Watcher error: {0}")]
    Watcher(#[from] notify::Error),

    #[error("Parse error: {0}")]
    Parse(#[from] chrono::ParseError),

    #[error("Search error: {0}")]
    SearchError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, CrabletError>;
