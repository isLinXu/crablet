use std::fs;
use std::path::Path;
use anyhow::{Result, Context, anyhow};
use crate::plugins::Plugin;
use crate::safety::oracle::{SafetyOracle, SafetyDecision};
use async_trait::async_trait;
use serde_json::Value;

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
        match self.oracle.check_file_access(path) {
            SafetyDecision::Blocked(reason) => return Ok(format!("🚫 Safety Oracle Blocked: {}", reason)),
            _ => {} // Allowed or Confirmation (File access usually allowed if not blocked)
        }

        match action {
            "read" => FileTool::read(path),
            "write" => {
                let content = args.get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                FileTool::write(path, content).map(|_| "File written successfully".to_string())
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
    pub fn read(path: &str) -> Result<String> {
        if !Path::new(path).exists() {
            return Err(anyhow!("File not found: {}", path));
        }
        fs::read_to_string(path).context("Failed to read file")
    }

    pub fn write(path: &str, content: &str) -> Result<()> {
        fs::write(path, content).context("Failed to write file")
    }

    pub fn list(path: &str) -> Result<Vec<String>> {
        let entries = fs::read_dir(path)?
            .map(|res| res.map(|e| e.path().to_string_lossy().into_owned()))
            .collect::<Result<Vec<_>, std::io::Error>>()?;
        Ok(entries)
    }
}
