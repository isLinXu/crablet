use async_trait::async_trait;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum Language {
    Python,
    JavaScript,
    Shell,
    Lua,
}

#[async_trait]
pub trait Sandbox: Send + Sync {
    /// Initialize the sandbox (e.g. pull images)
    async fn init(&self) -> Result<()>;
    
    /// Execute code in the sandbox
    async fn execute(&self, language: Language, code: &str) -> Result<ExecutionResult>;
    
    /// Cleanup resources (e.g. stop containers)
    async fn cleanup(&self) -> Result<()>;
}

pub mod docker;
pub mod local;
