//! System 1 — Unified Intuitive Response Layer
//!
//! This module re-exports the canonical `System1Enhanced` implementation as `System1`.
//! The enhanced implementation provides:
//! - Multi-pattern matching (exact, prefix, regex, fuzzy, contains)
//! - 20+ command categories with priority ordering
//! - Context-aware responses
//! - Runtime command registration (dynamic extensibility)
//! - Template-based response generation
//!
//! The legacy `System1` (basic Trie + Levenshtein) and `System1Dynamic` (runtime rules)
//! have been consolidated into `System1Enhanced` to eliminate code duplication.

// Re-export the unified implementation
pub use super::system1_enhanced::{
    Command, CommandCategory, CommandHandler, ResponseTemplate, System1Enhanced,
};

/// Canonical type alias — all callers should use `System1`.
pub type System1 = System1Enhanced;
