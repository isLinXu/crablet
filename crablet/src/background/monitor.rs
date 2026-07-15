use crate::error::Result;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tracing::{error, info};

/// Type alias for async background task actions
pub type AsyncTaskFn =
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync>;

/// BackgroundMonitor manages periodic maintenance and system check tasks.
pub struct BackgroundMonitor {
    tasks: Vec<BackgroundTask>,
}

pub struct BackgroundTask {
    pub name: String,
    pub interval: Duration,
    pub action: AsyncTaskFn,
}

impl Default for BackgroundMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl BackgroundMonitor {
    pub fn new() -> Self {
        Self { tasks: Vec::new() }
    }

    pub fn with_task(mut self, task: BackgroundTask) -> Self {
        self.tasks.push(task);
        self
    }

    pub async fn start(self) {
        info!(
            "Starting Background Monitor with {} tasks",
            self.tasks.len()
        );

        for task in self.tasks {
            let name = task.name.clone();
            let interval = task.interval;
            let action = task.action;

            tokio::spawn(async move {
                let mut ticker = tokio::time::interval(interval);
                loop {
                    ticker.tick().await;
                    info!("Running background task: {}", name);
                    if let Err(e) = action().await {
                        error!("Background task '{}' failed: {}", name, e);
                    }
                }
            });
        }
    }
}

/// Create a standard cargo check task
pub fn create_cargo_check_task(interval: Duration) -> BackgroundTask {
    BackgroundTask {
        name: "cargo_check".to_string(),
        interval,
        action: Box::new(|| {
            Box::pin(async {
                use tokio::process::Command;
                let output = Command::new("cargo").arg("check").output().await?;
                if !output.status.success() {
                    error!(
                        "Cargo check failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                } else {
                    info!("Cargo check passed.");
                }
                Ok(())
            })
        }),
    }
}
