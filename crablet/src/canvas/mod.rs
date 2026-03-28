//! Canvas System Module
//!
//! Provides an interactive canvas for visual operations, drawings, and collaborative editing.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      Canvas System                              │
//! │                                                                  │
//! │   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐       │
//! │   │   Manager   │    │   Renderer  │    │  Interaction │       │
//! │   │             │    │             │    │              │       │
//! │   │ • Sessions  │    │ • WebGL     │    │ • Mouse      │       │
//! │   │ • History   │    │ • Layers    │    │ • Touch      │       │
//! │   │ • Export    │    │ • Shapes    │    │ • Keyboard   │       │
//! │   └─────────────┘    └─────────────┘    └─────────────┘       │
//! │          │                  │                  │               │
//! │          └──────────────────┼──────────────────┘               │
//! │                             ▼                                  │
//! │                    ┌─────────────────┐                        │
//! │                    │   CanvasState   │                        │
//! │                    │  • viewport     │                        │
//! │                    │  • elements     │                        │
//! │                    │  • transform    │                        │
//! │                    └─────────────────┘                        │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

pub mod manager;
pub mod renderer;
pub mod types;
pub mod layer;
pub mod shape;
pub mod transform;
pub mod interaction;
pub mod history;
pub mod export;

pub use manager::CanvasManager;
pub use types::*;
pub use layer::CanvasLayer;
pub use shape::{Shape, ShapeType, ShapeEngine};
pub use transform::{Transform, TransformMode};
pub use interaction::{InteractionHandler, InteractionEvent};
pub use history::{HistoryManager, HistoryAction};
pub use export::{ExportFormat, CanvasExporter};

use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

/// Canvas system coordinator
pub struct CanvasSystem {
    manager: Arc<RwLock<CanvasManager>>,
    enabled: bool,
}

impl CanvasSystem {
    /// Create a new canvas system
    pub fn new() -> Self {
        Self {
            manager: Arc::new(RwLock::new(CanvasManager::new())),
            enabled: true,
        }
    }

    /// Get canvas manager
    pub fn manager(&self) -> Arc<RwLock<CanvasManager>> {
        self.manager.clone()
    }

    /// Check if canvas is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Enable/disable canvas
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl Default for CanvasSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_system_creation() {
        let canvas = CanvasSystem::new();
        assert!(canvas.is_enabled());
    }

    #[tokio::test]
    async fn test_canvas_manager() {
        let manager = CanvasManager::new();
        let session = manager.create_session("test").await;
        assert!(session.is_some());
    }
}