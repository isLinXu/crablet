use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher, Event};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, Mutex};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};
use crate::skills::SkillRegistry;

pub struct ReloadStats {
    pub total_reloads: usize,
    pub recent_reloads: usize,
    pub last_reload: Option<Instant>,
}

pub struct SkillWatcher {
    _watcher: RecommendedWatcher,
    reload_history: Arc<Mutex<Vec<Instant>>>,
}

impl SkillWatcher {
    pub fn new(registry: Arc<RwLock<SkillRegistry>>, skills_dir: &Path) -> Result<Self> {
        Self::new_with_limits(registry, skills_dir, 10) // Default 10 reloads per minute
    }

    pub fn new_with_limits(
        registry: Arc<RwLock<SkillRegistry>>, 
        skills_dir: &Path,
        max_reloads_per_minute: usize,
    ) -> Result<Self> {
        let (tx, mut rx) = mpsc::channel(1);
        let registry_clone = registry.clone();
        let skills_dir_buf = skills_dir.to_path_buf();
        let reload_history = Arc::new(Mutex::new(Vec::new()));
        let history_clone = reload_history.clone();

        // Create a watcher that sends events to our channel
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            match res {
                Ok(event) => {
                    if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                        let _ = tx.blocking_send(());
                    }
                }
                Err(e) => error!("Watch error: {:?}", e),
            }
        })?;

        // Start watching
        watcher.watch(skills_dir, RecursiveMode::Recursive)?;
        info!("Hot Reloading enabled for skills in {:?} (Max {} reloads/min)", skills_dir, max_reloads_per_minute);

        // Spawn a task to handle reload events (debounced)
        tokio::spawn(async move {
            while rx.recv().await.is_some() {
                // Debounce: wait 500ms and drain pending
                tokio::time::sleep(Duration::from_millis(500)).await;
                while rx.try_recv().is_ok() {}

                // Check rate limit
                let mut history = history_clone.lock().await;
                let now = Instant::now();
                history.retain(|&t| now.duration_since(t) < Duration::from_secs(60));
                
                if history.len() >= max_reloads_per_minute {
                    warn!("Skill reload rate limit exceeded ({} reloads in last minute), skipping...", history.len());
                    continue;
                }

                info!("Detected skill changes, reloading registry...");
                history.push(now);
                drop(history); // Release lock before registry write lock

                let mut reg = registry_clone.write().await;
                
                // Backup current state for rollback
                let backup = reg.backup();
                
                // Clear and reload
                reg.clear();
                
                if let Err(e) = reg.load_from_dir(&skills_dir_buf).await {
                    warn!("Failed to hot-reload skills: {}. Rolling back...", e);
                    reg.restore(backup);
                } else {
                    info!("Skills successfully reloaded!");
                }
            }
        });

        Ok(Self {
            _watcher: watcher,
            reload_history,
        })
    }

    pub async fn get_reload_stats(&self) -> ReloadStats {
        let history = self.reload_history.lock().await;
        let now = Instant::now();
        let recent = history.iter().filter(|&&t| now.duration_since(t) < Duration::from_secs(60)).count();
        
        ReloadStats {
            total_reloads: history.len(),
            recent_reloads: recent,
            last_reload: history.last().copied(),
        }
    }
}
