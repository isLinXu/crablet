use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use crate::memory::core::{CoreMemory, CoreMemoryBlock};
use crate::error::Result;

/// DistributedCoreMemory syncs core memory events across multiple processes or agents.
pub struct DistributedCoreMemory {
    local: Arc<RwLock<CoreMemory>>,
    bus: broadcast::Sender<CoreMemoryEvent>,
    #[allow(dead_code)]
    path: PathBuf,
}

#[derive(Clone, Debug)]
pub enum CoreMemoryEvent {
    Updated {
        block: CoreMemoryBlock,
        content: String,
        version: u64,
    },
    SyncRequested,
}

impl DistributedCoreMemory {
    pub fn new(local: Arc<RwLock<CoreMemory>>, path: PathBuf) -> Self {
        let (bus, _) = broadcast::channel(100);
        Self {
            local,
            bus,
            path,
        }
    }

    pub async fn append(&self, block: CoreMemoryBlock, content: &str) -> Result<usize> {
        let mut core = self.local.write().await;
        let added = core.append(block, content)?;
        
        // Broadcast the update
        let _ = self.bus.send(CoreMemoryEvent::Updated {
            block,
            content: content.to_string(),
            version: core.version,
        });
        
        Ok(added)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CoreMemoryEvent> {
        self.bus.subscribe()
    }
    
    pub async fn sync(&self) -> Result<()> {
        let _ = self.bus.send(CoreMemoryEvent::SyncRequested);
        Ok(())
    }
}
