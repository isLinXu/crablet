//! Compatibility shim for future agent memory pipeline work.

use serde::{Deserialize, Serialize};

/// Minimal result type for wiring higher-level pipeline code later.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryPipelineResult {
    pub stored_items: usize,
    pub summarized: bool,
}
