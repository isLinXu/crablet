//! Evaluation Module
//!
//! Provides tools for evaluating and optimizing system performance,
//! including A/B testing, metrics collection, and performance analysis.

pub mod ab_testing;

// Re-export commonly used types
pub use ab_testing::{
    ABTestManager,
    ABTestConfig,
    TestResults,
    VariantConfig,
    SuccessMetric,
    TestPresets,
};
