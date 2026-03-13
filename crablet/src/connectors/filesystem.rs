//! # FileSystem Connector
//!
//! File system monitoring for file-based triggers.
//!
//! ## Features
//!
//! - Watch directories for file changes
//! - Filter by file patterns (glob, regex)
//! - Recursive watching
//! - File content hashing for change detection
//! - Batch processing support
//!
//! ## Example
//!
//! ```rust
//! use crablet::connectors::{FileSystemConnector, ConnectorConfig};
//!
//! let config = ConnectorConfig {
//!     connector_type: "filesystem".to_string(),
//!     name: "Uploads Watcher".to_string(),
//!     enabled: true,
//!     settings: serde_json::json!({
//!         "paths": ["/data/uploads"],
//!         "recursive": true,
//!         "patterns": ["*.csv", "*.json"],
//!         "ignore_patterns": ["*.tmp", ".*"],
//!         "debounce_ms": 1000
//!     }),
//!     filters: vec![],
//!     transformations: vec![],
//! };
//!
//! let connector = FileSystemConnector::new(config).await?;
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::connectors::{Connector, ConnectorConfig, ConnectorError, ConnectorEvent, ConnectorHealth, ConnectorResult, FileChangeType, FileMetadata, HealthStatus};

/// FileSystem connector configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSystemConfig {
    pub paths: Vec<String>,
    #[serde(default = "default_recursive")]
    pub recursive: bool,
    #[serde(default)]
    pub patterns: Vec<String>,
    #[serde(default)]
    pub ignore_patterns: Vec<String>,
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
    #[serde(default)]
    pub emit_initial: bool,
    #[serde(default)]
    pub follow_symlinks: bool,
    #[serde(default)]
    pub poll_interval_seconds: Option<u64>,
}

fn default_recursive() -> bool {
    true
}

fn default_debounce_ms() -> u64 {
    1000
}

/// File watcher implementation
pub struct FileSystemConnector {
    id: String,
    config: ConnectorConfig,
    fs_config: FileSystemConfig,
    connected: bool,
    running: bool,
    event_tx: mpsc::Sender<ConnectorEvent>,
    event_rx: Option<mpsc::Receiver<ConnectorEvent>>,
    watcher_handle: Option<JoinHandle<()>>,
    watched_files: std::sync::Arc<tokio::sync::RwLock<HashMap<PathBuf, FileState>>>,
}

#[derive(Debug, Clone)]
struct FileState {
    modified: DateTime<Utc>,
    size: u64,
    hash: Option<String>,
}

impl FileSystemConnector {
    pub fn new(config: ConnectorConfig) -> ConnectorResult<Self> {
        let fs_config: FileSystemConfig = serde_json::from_value(config.settings.clone())
            .map_err(|e| ConnectorError::ConfigurationError(format!("Invalid filesystem config: {}", e)))?;
        
        // Validate paths
        if fs_config.paths.is_empty() {
            return Err(ConnectorError::ConfigurationError(
                "At least one path must be specified".to_string()
            ));
        }
        
        for path in &fs_config.paths {
            let path_obj = Path::new(path);
            if !path_obj.exists() {
                warn!("Watch path does not exist: {}", path);
            } else if !path_obj.is_dir() {
                return Err(ConnectorError::ConfigurationError(
                    format!("Path is not a directory: {}", path)
                ));
            }
        }
        
        let (event_tx, event_rx) = mpsc::channel(1000);
        
        Ok(Self {
            id: uuid::Uuid::new_v4().to_string(),
            config,
            fs_config,
            connected: false,
            running: false,
            event_tx,
            event_rx: Some(event_rx),
            watcher_handle: None,
            watched_files: std::sync::Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        })
    }
    
    /// Check if a file matches the configured patterns
    fn matches_patterns(&self, path: &Path) -> bool {
        let file_name = match path.file_name() {
            Some(name) => name.to_string_lossy(),
            None => return false,
        };
        
        // Check ignore patterns first
        for pattern in &self.fs_config.ignore_patterns {
            if Self::glob_match(&file_name, pattern) {
                return false;
            }
        }
        
        // If no include patterns specified, include all
        if self.fs_config.patterns.is_empty() {
            return true;
        }
        
        // Check include patterns
        for pattern in &self.fs_config.patterns {
            if Self::glob_match(&file_name, pattern) {
                return true;
            }
        }
        
        false
    }
    
    /// Simple glob matching
    fn glob_match(name: &str, pattern: &str) -> bool {
        let regex_pattern = pattern
            .replace(".", "\\.")
            .replace("*", ".*")
            .replace("?", ".");
        
        match regex::Regex::new(&format!("^{}$", regex_pattern)) {
            Ok(re) => re.is_match(name),
            Err(_) => name.contains(pattern),
        }
    }
    
    /// Get file metadata
    async fn get_file_metadata(&self, path: &Path) -> Option<FileMetadata> {
        match tokio::fs::metadata(path).await {
            Ok(metadata) => {
                let modified = metadata.modified()
                    .ok()
                    .map(|t| chrono::DateTime::from(std::time::SystemTime::from(t)));
                let created = metadata.created()
                    .ok()
                    .map(|t| chrono::DateTime::from(std::time::SystemTime::from(t)));
                
                Some(FileMetadata {
                    size: metadata.len(),
                    modified,
                    created,
                    permissions: Some(metadata.permissions().readonly() as u32),
                })
            }
            Err(e) => {
                debug!("Failed to get metadata for {:?}: {}", path, e);
                None
            }
        }
    }
    
    /// Emit a file change event
    async fn emit_event(&self, path: PathBuf, change_type: FileChangeType) {
        let metadata = self.get_file_metadata(&path).await;
        
        let event = ConnectorEvent::FileChanged {
            connector_id: self.id.clone(),
            watch_id: uuid::Uuid::new_v4().to_string(),
            path: path.to_string_lossy().to_string(),
            change_type,
            metadata,
            timestamp: Utc::now(),
        };
        
        if let Err(e) = self.event_tx.send(event).await {
            error!("Failed to send file event: {}", e);
        }
    }
    
    /// Start watching files
    async fn start_watching(&mut self) -> ConnectorResult<()> {
        let paths = self.fs_config.paths.clone();
        let recursive = self.fs_config.recursive;
        let debounce_ms = self.fs_config.debounce_ms;
        let event_tx = self.event_tx.clone();
        let watched_files = self.watched_files.clone();
        let patterns = self.fs_config.patterns.clone();
        let ignore_patterns = self.fs_config.ignore_patterns.clone();
        
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                
                for path_str in &paths {
                    let path = Path::new(path_str);
                    if !path.exists() {
                        continue;
                    }
                    
                    // Scan directory
                    if let Err(e) = Self::scan_directory(
                        path,
                        recursive,
                        &patterns,
                        &ignore_patterns,
                        &watched_files,
                        &event_tx,
                    ).await {
                        debug!("Scan error for {:?}: {}", path, e);
                    }
                }
                
                tokio::time::sleep(tokio::time::Duration::from_millis(debounce_ms)).await;
            }
        });
        
        self.watcher_handle = Some(handle);
        Ok(())
    }
    
    async fn scan_directory(
        path: &Path,
        recursive: bool,
        patterns: &[String],
        ignore_patterns: &[String],
        watched_files: &tokio::sync::RwLock<HashMap<PathBuf, FileState>>,
        event_tx: &mpsc::Sender<ConnectorEvent>,
    ) -> ConnectorResult<()> {
        let mut entries = match tokio::fs::read_dir(path).await {
            Ok(entries) => entries,
            Err(e) => return Err(ConnectorError::IoError(e)),
        };
        
        while let Ok(Some(entry)) = entries.next_entry().await {
            let entry_path = entry.path();
            
            if entry_path.is_dir() {
                if recursive {
                    Box::pin(Self::scan_directory(
                        &entry_path,
                        recursive,
                        patterns,
                        ignore_patterns,
                        watched_files,
                        event_tx,
                    )).await?;
                }
                continue;
            }
            
            // Check patterns
            let file_name = entry_path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            
            // Check ignore patterns
            let ignored = ignore_patterns.iter().any(|p| {
                let regex_pattern = p.replace(".", "\\.").replace("*", ".*").replace("?", ".");
                regex::Regex::new(&format!("^{}$", regex_pattern))
                    .map(|re| re.is_match(&file_name))
                    .unwrap_or(false)
            });
            
            if ignored {
                continue;
            }
            
            // Check include patterns
            if !patterns.is_empty() {
                let matched = patterns.iter().any(|p| {
                    let regex_pattern = p.replace(".", "\\.").replace("*", ".*").replace("?", ".");
                    regex::Regex::new(&format!("^{}$", regex_pattern))
                        .map(|re| re.is_match(&file_name))
                        .unwrap_or(false)
                });
                
                if !matched {
                    continue;
                }
            }
            
            // Get file metadata
            let metadata = match entry.metadata().await {
                Ok(m) => m,
                Err(_) => continue,
            };
            
            let modified = metadata.modified()
                .ok()
                .map(|t| chrono::DateTime::from(std::time::SystemTime::from(t)))
                .unwrap_or_else(Utc::now);
            
            let size = metadata.len();
            
            // Check if file has changed
            let mut files = watched_files.write().await;
            let changed = match files.get(&entry_path) {
                Some(state) => state.modified != modified || state.size != size,
                None => true,
            };
            
            if changed {
                let change_type = if files.contains_key(&entry_path) {
                    FileChangeType::Modified
                } else {
                    FileChangeType::Created
                };
                
                files.insert(entry_path.clone(), FileState {
                    modified,
                    size,
                    hash: None,
                });
                
                let event = ConnectorEvent::FileChanged {
                    connector_id: "filesystem".to_string(),
                    watch_id: uuid::Uuid::new_v4().to_string(),
                    path: entry_path.to_string_lossy().to_string(),
                    change_type,
                    metadata: Some(FileMetadata {
                        size,
                        modified: Some(modified),
                        created: metadata.created().ok().map(|t| chrono::DateTime::from(std::time::SystemTime::from(t))),
                        permissions: Some(metadata.permissions().readonly() as u32),
                    }),
                    timestamp: Utc::now(),
                };
                
                if let Err(e) = event_tx.send(event).await {
                    error!("Failed to send file event: {}", e);
                }
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl Connector for FileSystemConnector {
    fn id(&self) -> &str {
        &self.id
    }
    
    fn name(&self) -> &str {
        &self.config.name
    }
    
    fn connector_type(&self) -> &str {
        "filesystem"
    }
    
    fn is_connected(&self) -> bool {
        self.connected
    }
    
    async fn connect(&mut self) -> ConnectorResult<()> {
        info!("Initializing filesystem connector: {}", self.config.name);
        
        // Validate all paths exist
        for path in &self.fs_config.paths {
            if !Path::new(path).exists() {
                return Err(ConnectorError::ConfigurationError(
                    format!("Path does not exist: {}", path)
                ));
            }
        }
        
        self.connected = true;
        info!("Filesystem connector '{}' initialized", self.config.name);
        Ok(())
    }
    
    async fn disconnect(&mut self) -> ConnectorResult<()> {
        self.running = false;
        self.connected = false;
        
        if let Some(handle) = self.watcher_handle.take() {
            handle.abort();
        }
        
        info!("Filesystem connector '{}' disconnected", self.config.name);
        Ok(())
    }
    
    async fn start(&mut self) -> ConnectorResult<()> {
        if !self.connected {
            return Err(ConnectorError::NotConnected);
        }
        
        self.start_watching().await?;
        self.running = true;
        
        info!("Filesystem connector '{}' started watching {:?}", 
            self.config.name, 
            self.fs_config.paths
        );
        Ok(())
    }
    
    async fn stop(&mut self) -> ConnectorResult<()> {
        self.running = false;
        
        if let Some(handle) = self.watcher_handle.take() {
            handle.abort();
        }
        
        info!("Filesystem connector '{}' stopped", self.config.name);
        Ok(())
    }
    
    fn event_receiver(&mut self) -> Option<mpsc::Receiver<ConnectorEvent>> {
        self.event_rx.take()
    }
    
    async fn test(&self) -> ConnectorResult<()> {
        for path in &self.fs_config.paths {
            if !Path::new(path).exists() {
                return Err(ConnectorError::ConfigurationError(
                    format!("Path does not exist: {}", path)
                ));
            }
            
            // Test read permissions
            match tokio::fs::read_dir(path).await {
                Ok(_) => {}
                Err(e) => return Err(ConnectorError::IoError(e)),
            }
        }
        
        Ok(())
    }
    
    async fn health(&self) -> ConnectorHealth {
        ConnectorHealth {
            status: if self.connected {
                if self.running {
                    HealthStatus::Healthy
                } else {
                    HealthStatus::Degraded
                }
            } else {
                HealthStatus::Unhealthy
            },
            last_check: Utc::now(),
            message: None,
            latency_ms: None,
        }
    }
}

/// File watcher builder
pub struct FileWatcherBuilder {
    paths: Vec<String>,
    recursive: bool,
    patterns: Vec<String>,
    ignore_patterns: Vec<String>,
    debounce_ms: u64,
}

impl FileWatcherBuilder {
    pub fn new() -> Self {
        Self {
            paths: Vec::new(),
            recursive: true,
            patterns: Vec::new(),
            ignore_patterns: vec!["*.tmp".to_string(), ".*".to_string()],
            debounce_ms: 1000,
        }
    }
    
    pub fn watch(mut self, path: impl Into<String>) -> Self {
        self.paths.push(path.into());
        self
    }
    
    pub fn recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }
    
    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        self.patterns.push(pattern.into());
        self
    }
    
    pub fn ignore(mut self, pattern: impl Into<String>) -> Self {
        self.ignore_patterns.push(pattern.into());
        self
    }
    
    pub fn debounce(mut self, ms: u64) -> Self {
        self.debounce_ms = ms;
        self
    }
    
    pub fn build(self) -> ConnectorConfig {
        ConnectorConfig {
            connector_type: "filesystem".to_string(),
            name: "File Watcher".to_string(),
            enabled: true,
            settings: serde_json::json!({
                "paths": self.paths,
                "recursive": self.recursive,
                "patterns": self.patterns,
                "ignore_patterns": self.ignore_patterns,
                "debounce_ms": self.debounce_ms,
            }),
            filters: vec![],
            transformations: vec![],
        }
    }
}

impl Default for FileWatcherBuilder {
    fn default() -> Self {
        Self::new()
    }
}
