use std::path::Path;
use anyhow::{Result, Context, anyhow};
use crate::plugins::Plugin;
use crate::safety::oracle::{SafetyOracle, SafetyDecision};
use async_trait::async_trait;
use serde_json::Value;
use tokio::fs;

pub struct FilePlugin {
    oracle: SafetyOracle,
}

impl FilePlugin {
    pub fn new(oracle: SafetyOracle) -> Self {
        Self { oracle }
    }
}

#[async_trait]
impl Plugin for FilePlugin {
    fn name(&self) -> &str {
        "file"
    }

    fn description(&self) -> &str {
        "Read/Write files. Args: {\"action\": \"read\"|\"write\", \"path\": \"...\", \"content\": \"...\"}"
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let action = args.get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("read");
            
        let path = args.get("path")
            .and_then(|v| v.as_str())
            .context("Missing 'path' argument")?;

        // Safety Check
        if let SafetyDecision::Blocked(reason) = self.oracle.check_file_access(path) {
            return Ok(format!("🚫 Safety Oracle Blocked: {}", reason));
        }

        // Define a sandbox root (e.g., current working directory for now, or a specific data dir)
        let sandbox_root = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());

        match action {
            "read" => FileTool::read_safe(path, &sandbox_root).await,
            "write" => {
                let content = args.get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                
                // Check write access against sandbox
                let p = Path::new(path);
                let canonical = if p.exists() {
                    fs::canonicalize(p).await?
                } else {
                    let parent = p.parent().unwrap_or(Path::new("."));
                    if parent.exists() {
                        fs::canonicalize(parent).await?.join(p.file_name().unwrap_or_default())
                    } else {
                        return Err(anyhow!("Parent directory does not exist"));
                    }
                };
                
                if !canonical.starts_with(&sandbox_root) {
                    return Err(anyhow!("Access denied: path outside sandbox"));
                }
                
                FileTool::write(path, content).await.map(|_| "File written successfully".to_string())
            },
            _ => Err(anyhow!("Unknown action: {}", action))
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct FileTool;

impl FileTool {
    pub async fn read_safe(path: &str, sandbox_root: &Path) -> Result<String> {
        let canonical = fs::canonicalize(path).await
            .context("Failed to resolve path")?;
        
        // Ensure inside sandbox
        if !canonical.starts_with(sandbox_root) {
            return Err(anyhow!("Access denied: path outside sandbox"));
        }
        
        // Check file size limit (e.g., 10MB)
        let max_file_size = 10 * 1024 * 1024;
        let metadata = fs::metadata(&canonical).await?;
        if metadata.len() > max_file_size {
            return Err(anyhow!("File too large (max 10MB)"));
        }
        
        fs::read_to_string(&canonical).await.context("Failed to read file")
    }

    pub async fn read(path: &str) -> Result<String> {
        // Fallback for non-sandboxed internal use if needed, but prefer safe version
        // For now, redirect to safe with CWD
        let root = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
        Self::read_safe(path, &root).await
    }

    pub async fn write(path: &str, content: &str) -> Result<()> {
        fs::write(path, content).await.context("Failed to write file")
    }

    pub async fn list(path: &str) -> Result<Vec<String>> {
        let mut entries = fs::read_dir(path).await?;
        let mut results = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            results.push(entry.path().to_string_lossy().into_owned());
        }
        Ok(results)
    }
}
