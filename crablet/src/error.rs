use thiserror::Error;

#[derive(Error, Debug)]
pub enum CrabletError {
    #[error("LLM Provider Error: {0}")]
    LlmError(String),

    #[error("Tool Execution Error: {0}")]
    ToolError(String),

    #[error("Memory Error: {0}")]
    MemoryError(String),

    #[error("Database Error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Configuration Error: {0}")]
    ConfigError(String),

    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization Error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Unknown Error: {0}")]
    Unknown(String),
}
