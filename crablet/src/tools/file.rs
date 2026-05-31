use crate::plugins::Plugin;
use crate::safety::oracle::{SafetyDecision, SafetyOracle};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::fs;

const MAX_READ_BYTES: u64 = 10 * 1024 * 1024;
const MAX_WRITE_BYTES: usize = 5 * 1024 * 1024;
const MAX_LIST_ENTRIES: usize = 1000;

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
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("read");

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing 'path' argument")?;

        // Safety Check
        if let SafetyDecision::Blocked(reason) = self.oracle.check_file_access(path) {
            return Ok(format!("🚫 Safety Oracle Blocked: {}", reason));
        }

        let sandbox_root = FileTool::default_sandbox_root();

        match action {
            "read" => FileTool::read_safe(path, &sandbox_root).await,
            "list" => FileTool::list_safe(path, &sandbox_root)
                .await
                .map(|entries| entries.join("\n")),
            "write" => {
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
                FileTool::write_safe(path, content, &sandbox_root)
                    .await
                    .map(|_| "File written successfully".to_string())
            }
            _ => Err(anyhow!("Unknown action: {}", action)),
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct FileTool;

impl FileTool {
    fn default_sandbox_root() -> PathBuf {
        std::env::var_os("CRABLET_FILE_TOOL_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    async fn canonical_sandbox_root(sandbox_root: &Path) -> Result<PathBuf> {
        fs::canonicalize(sandbox_root)
            .await
            .with_context(|| format!("Failed to resolve sandbox root {}", sandbox_root.display()))
    }

    async fn resolve_existing_path(path: &str, sandbox_root: &Path) -> Result<PathBuf> {
        let root = Self::canonical_sandbox_root(sandbox_root).await?;
        let canonical = fs::canonicalize(path)
            .await
            .with_context(|| format!("Failed to resolve path '{}'", path))?;

        if !canonical.starts_with(&root) {
            return Err(anyhow!("Access denied: path outside sandbox"));
        }

        Ok(canonical)
    }

    async fn resolve_writable_path(path: &str, sandbox_root: &Path) -> Result<PathBuf> {
        let root = Self::canonical_sandbox_root(sandbox_root).await?;
        let requested = Path::new(path);

        let resolved = if fs::metadata(requested).await.is_ok() {
            fs::canonicalize(requested)
                .await
                .with_context(|| format!("Failed to resolve path '{}'", path))?
        } else {
            let parent = requested.parent().unwrap_or_else(|| Path::new("."));
            let filename = requested
                .file_name()
                .ok_or_else(|| anyhow!("Path must include a file name"))?;
            fs::canonicalize(parent)
                .await
                .with_context(|| format!("Parent directory does not exist: {}", parent.display()))?
                .join(filename)
        };

        if !resolved.starts_with(&root) {
            return Err(anyhow!("Access denied: path outside sandbox"));
        }

        Ok(resolved)
    }

    pub async fn read_safe(path: &str, sandbox_root: &Path) -> Result<String> {
        let canonical = Self::resolve_existing_path(path, sandbox_root).await?;

        let metadata = fs::metadata(&canonical).await?;
        if !metadata.is_file() {
            return Err(anyhow!("Path is not a regular file"));
        }
        if metadata.len() > MAX_READ_BYTES {
            return Err(anyhow!("File too large (max 10MB)"));
        }

        fs::read_to_string(&canonical)
            .await
            .context("Failed to read file")
    }

    pub async fn read(path: &str) -> Result<String> {
        // Fallback for non-sandboxed internal use if needed, but prefer safe version
        // For now, redirect to safe with CWD
        let root = Self::default_sandbox_root();
        Self::read_safe(path, &root).await
    }

    pub async fn write_safe(path: &str, content: &str, sandbox_root: &Path) -> Result<()> {
        if content.len() > MAX_WRITE_BYTES {
            return Err(anyhow!("Content too large (max 5MB)"));
        }

        let resolved = Self::resolve_writable_path(path, sandbox_root).await?;
        fs::write(&resolved, content)
            .await
            .context("Failed to write file")
    }

    pub async fn write(path: &str, content: &str) -> Result<()> {
        let root = Self::default_sandbox_root();
        Self::write_safe(path, content, &root).await
    }

    pub async fn list_safe(path: &str, sandbox_root: &Path) -> Result<Vec<String>> {
        let canonical = Self::resolve_existing_path(path, sandbox_root).await?;
        let metadata = fs::metadata(&canonical).await?;
        if !metadata.is_dir() {
            return Err(anyhow!("Path is not a directory"));
        }

        let mut entries = fs::read_dir(&canonical).await?;
        let mut results = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            if results.len() >= MAX_LIST_ENTRIES {
                return Err(anyhow!(
                    "Directory listing exceeded {} entries",
                    MAX_LIST_ENTRIES
                ));
            }
            results.push(entry.path().to_string_lossy().into_owned());
        }
        Ok(results)
    }

    pub async fn list(path: &str) -> Result<Vec<String>> {
        let root = Self::default_sandbox_root();
        Self::list_safe(path, &root).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn read_safe_blocks_paths_outside_sandbox() {
        let sandbox = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let outside_file = outside.path().join("secret.txt");
        fs::write(&outside_file, "secret").await.unwrap();

        let error = FileTool::read_safe(outside_file.to_str().unwrap(), sandbox.path())
            .await
            .expect_err("outside path should be blocked");

        assert!(error.to_string().contains("outside sandbox"));
    }

    #[tokio::test]
    async fn write_safe_writes_inside_sandbox() {
        let sandbox = tempdir().unwrap();
        let target = sandbox.path().join("note.txt");

        FileTool::write_safe(target.to_str().unwrap(), "hello", sandbox.path())
            .await
            .unwrap();

        let written = fs::read_to_string(&target).await.unwrap();
        assert_eq!(written, "hello");
    }

    #[tokio::test]
    async fn write_safe_blocks_large_content() {
        let sandbox = tempdir().unwrap();
        let target = sandbox.path().join("large.txt");
        let content = "x".repeat(MAX_WRITE_BYTES + 1);

        let error = FileTool::write_safe(target.to_str().unwrap(), &content, sandbox.path())
            .await
            .expect_err("large write should be blocked");

        assert!(error.to_string().contains("Content too large"));
    }

    #[tokio::test]
    async fn list_safe_blocks_files_outside_sandbox() {
        let sandbox = tempdir().unwrap();
        let outside = tempdir().unwrap();

        let error = FileTool::list_safe(outside.path().to_str().unwrap(), sandbox.path())
            .await
            .expect_err("outside directory should be blocked");

        assert!(error.to_string().contains("outside sandbox"));
    }
}
