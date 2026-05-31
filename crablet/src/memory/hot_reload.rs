use crate::error::Result;
use crate::memory::core::CoreMemory;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// HotReloader monitors the Core Memory file for changes and reloads it automatically.
pub struct CoreMemoryHotReloader {
    path: PathBuf,
    #[allow(dead_code)]
    watcher: Option<RecommendedWatcher>,
}

impl CoreMemoryHotReloader {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            watcher: None,
        }
    }

    pub fn start_watch(&mut self, core_memory: Arc<RwLock<CoreMemory>>) -> Result<()> {
        let path = self.path.clone();
        let core = core_memory.clone();

        info!("Starting hot-reload watcher for Core Memory at {:?}", path);

        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<Event>| match res {
                Ok(event) => {
                    if matches!(event.kind, EventKind::Modify(_)) {
                        let path_clone = path.clone();
                        let core_clone = core.clone();

                        tokio::spawn(async move {
                            match CoreMemory::load(&path_clone) {
                                Ok(new_core) => {
                                    let mut current = core_clone.write().await;
                                    if new_core.version > current.version {
                                        *current = new_core;
                                        info!(
                                            "Core Memory hot-reloaded from disk (version: {})",
                                            current.version
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to hot-reload Core Memory: {}", e);
                                }
                            }
                        });
                    }
                }
                Err(e) => error!("Watcher error: {:?}", e),
            })?;

        watcher.watch(&self.path, RecursiveMode::NonRecursive)?;
        self.watcher = Some(watcher);

        Ok(())
    }
}
