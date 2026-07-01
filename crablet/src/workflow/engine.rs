/// Re-export the v2 DAG-scheduling engine as the canonical `WorkflowEngine`.
///
/// All callers using `crate::workflow::engine::WorkflowEngine` automatically
/// get the full DAG execution engine without changes.
pub use crate::workflow::engine_v2::WorkflowEngine;
pub use crate::workflow::engine_v2::WorkflowEngineError;
