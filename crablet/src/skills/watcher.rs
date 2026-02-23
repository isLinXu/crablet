use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use crate::skills::SkillRegistry;

pub struct SkillWatcher {
    _watcher: RecommendedWatcher,
}

impl SkillWatcher {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>, skills_dir: &Path) -> Result<Self> {
        let (tx, mut rx) = mpsc::channel(1);
        let registry_clone = registry.clone();
        let skills_dir_buf = skills_dir.to_path_buf();

        // Create a watcher that sends events to our channel
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            match res {
                Ok(event) => {
                    // Only care about write, create, or remove events on skill files
                    if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                        let _ = tx.blocking_send(());
                    }
                }
                Err(e) => error!("Watch error: {:?}", e),
            }
        })?;

        // Start watching
        watcher.watch(skills_dir, RecursiveMode::Recursive)?;
        info!("Hot Reloading enabled for skills in {:?}", skills_dir);

        // Spawn a task to handle reload events (debounced)
        tokio::spawn(async move {
            while rx.recv().await.is_some() {
                // Simple debounce: wait 500ms and drain any other pending events
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                while rx.try_recv().is_ok() {}

                info!("Detected skill changes, reloading registry...");
                let mut reg = registry_clone.write().await;
                
                // Clear existing skills to handle removals
                reg.clear();
                
                if let Err(e) = reg.load_from_dir(&skills_dir_buf).await {
                    warn!("Failed to hot-reload skills: {}", e);
                } else {
                    info!("Skills successfully reloaded!");
                }
            }
        });

        Ok(Self {
            _watcher: watcher,
        })
    }
}
