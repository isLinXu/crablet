//! RPA Safety Layer
//!
//! Provides security controls for desktop automation operations.
//! Every RPA action must pass through this layer before execution.

use std::collections::HashSet;
use std::sync::Arc;
use parking_lot::RwLock;
use tracing::{debug, info, warn};
use chrono::Utc;
use governor::{Quota, RateLimiter};
use governor::clock::DefaultClock;
use governor::state::{InMemoryState, NotKeyed};

use crate::rpa::desktop::DesktopStep;

/// Screen region whitelist entry
#[derive(Debug, Clone, Copy)]
pub struct ScreenRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// RPA safety decision
#[derive(Debug, Clone, PartialEq)]
pub enum RpaSafetyDecision {
    /// Action is allowed
    Allow,
    /// Action is blocked with reason
    Block(String),
    /// Action requires user confirmation
    RequireConfirmation(String),
    /// Action is allowed but logged as warning
    Warn(String),
}

/// Configuration for RPA safety layer
#[derive(Debug, Clone)]
pub struct RpaSafetyConfig {
    /// Maximum desktop actions per minute (rate limiting)
    pub max_actions_per_minute: u32,
    /// Whether confirmation is required for new operations
    pub confirmation_required: bool,
    /// Whether to allow operations outside region whitelist
    pub allow_outside_regions: bool,
    /// Whether to enable audit logging for all RPA operations
    pub audit_logging: bool,
    /// Maximum consecutive actions before forced pause
    pub max_consecutive_actions: u32,
    /// Pause duration between action batches (milliseconds)
    pub batch_pause_ms: u64,
}

impl Default for RpaSafetyConfig {
    fn default() -> Self {
        Self {
            max_actions_per_minute: 60,
            confirmation_required: false,
            allow_outside_regions: true, // Allow by default; restrict explicitly
            audit_logging: true,
            max_consecutive_actions: 100,
            batch_pause_ms: 0,
        }
    }
}

impl RpaSafetyConfig {
    /// Strict configuration for production
    pub fn strict() -> Self {
        Self {
            max_actions_per_minute: 30,
            confirmation_required: true,
            allow_outside_regions: false,
            audit_logging: true,
            max_consecutive_actions: 50,
            batch_pause_ms: 100,
        }
    }

    /// Permissive configuration for development
    pub fn permissive() -> Self {
        Self {
            max_actions_per_minute: 120,
            confirmation_required: false,
            allow_outside_regions: true,
            audit_logging: true,
            max_consecutive_actions: 500,
            batch_pause_ms: 0,
        }
    }
}

/// Audit log entry for RPA operations
#[derive(Debug, Clone, serde::Serialize)]
pub struct RpaAuditEntry {
    pub timestamp: i64,
    pub action_type: String,
    pub decision: String,
    pub reason: Option<String>,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub details: serde_json::Value,
}

/// RPA Safety Layer — the central security checkpoint for all desktop automation
pub struct RpaSafetyLayer {
    config: RpaSafetyConfig,
    region_whitelist: RwLock<Vec<ScreenRegion>>,
    /// Rate limiter for actions per minute
    rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    /// Consecutive action counter
    consecutive_actions: RwLock<u32>,
    /// Audit log buffer
    audit_log: Arc<RwLock<Vec<RpaAuditEntry>>>,
    /// Blocked keyboard input patterns
    blocked_patterns: Arc<Vec<regex::Regex>>,
}

impl RpaSafetyLayer {
    /// Create a new safety layer with default configuration
    pub fn new() -> Self {
        let config = RpaSafetyConfig::default();
        Self::with_config(config)
    }

    /// Create a new safety layer with custom configuration
    pub fn with_config(config: RpaSafetyConfig) -> Self {
        let quota = Quota::per_minute(std::num::NonZeroU32::new(config.max_actions_per_minute).unwrap());
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        // Built-in blocked patterns for keyboard input
        let blocked_patterns: Vec<regex::Regex> = vec![
            r"(?i)rm\s+-rf\s+/",
            r"(?i)sudo\s+rm\s+-rf",
            r"(?i)chmod\s+777\s+/",
            r"(?i):\(\)\{:\|:&\}",
            r"(?i)mkfs\.",
            r"(?i)dd\s+if=/dev/zero",
            r"(?i)>\s*/dev/sd[a-z]",
        ].into_iter().filter_map(|p| regex::Regex::new(p).ok()).collect();

        Self {
            config,
            region_whitelist: RwLock::new(Vec::new()),
            rate_limiter,
            consecutive_actions: RwLock::new(0),
            audit_log: Arc::new(RwLock::new(Vec::new())),
            blocked_patterns: Arc::new(blocked_patterns),
        }
    }

    /// Check if a desktop step is safe to execute
    pub async fn check_step(
        &self,
        step: &DesktopStep,
        user_id: Option<&str>,
        session_id: Option<&str>,
    ) -> RpaSafetyDecision {
        // 1. Rate limit check
        match self.rate_limiter.check() {
            Ok(_) => {}
            Err(_) => {
                warn!(
                    "RPA rate limit exceeded (max {} actions/min)",
                    self.config.max_actions_per_minute
                );
                self.audit_action("rate_limit_exceeded", &RpaSafetyDecision::Block(
                    format!("Rate limit exceeded: max {} actions/min", self.config.max_actions_per_minute)
                ), user_id, session_id, None);
                return RpaSafetyDecision::Block(format!(
                    "Rate limit: max {} actions per minute. Please wait.",
                    self.config.max_actions_per_minute
                ));
            }
        }

        // 2. Consecutive action check
        {
            let mut count = self.consecutive_actions.write();
            *count += 1;
            if *count >= self.config.max_consecutive_actions {
                *count = 0;
                if self.config.batch_pause_ms > 0 {
                    debug!("Pausing after {} consecutive actions", self.config.max_consecutive_actions);
                    tokio::time::sleep(std::time::Duration::from_millis(self.config.batch_pause_ms)).await;
                }
            }
        }

        // 3. Check keyboard input for blocked patterns
        if let DesktopStep::KeyboardType { text } = step {
            for pattern in self.blocked_patterns.iter() {
                if pattern.is_match(text) {
                    let reason = format!("Blocked dangerous keyboard input pattern: {}", pattern.as_str());
                    warn!("{}", reason);
                    self.audit_action("blocked_input", &RpaSafetyDecision::Block(reason.clone()), user_id, session_id, None);
                    return RpaSafetyDecision::Block(reason);
                }
            }
        }

        // 4. Require confirmation for dangerous hotkeys
        if let DesktopStep::KeyboardHotkey { keys } = step {
            let dangerous_keys: HashSet<&str> = ["Alt", "F4"].into_iter().collect();
            let key_names: Vec<String> = keys.iter().map(|k| format!("{:?}", k)).collect();
            for key_name in &key_names {
                if dangerous_keys.contains(key_name.as_str()) {
                    let reason = format!("Dangerous hotkey combination requires confirmation: {:?}", keys);
                    self.audit_action("confirm_hotkey", &RpaSafetyDecision::RequireConfirmation(reason.clone()), user_id, session_id, None);
                    return RpaSafetyDecision::RequireConfirmation(reason);
                }
            }
        }

        // 5. Check region whitelist (if configured)
        if !self.config.allow_outside_regions {
            if let Some(region_check) = self.check_region_whitelist(step) {
                return region_check;
            }
        }

        // 6. Audit log
        if self.config.audit_logging {
            let details = serde_json::json!({
                "step": format!("{:?}", step),
            });
            self.audit_action(
                &format!("{:?}", std::mem::discriminant(step)),
                &RpaSafetyDecision::Allow,
                user_id,
                session_id,
                Some(details),
            );
        }

        // 7. If confirmation is globally required, upgrade Allow to RequireConfirmation
        if self.config.confirmation_required {
            RpaSafetyDecision::RequireConfirmation(
                "Global confirmation required for RPA operations".to_string()
            )
        } else {
            RpaSafetyDecision::Allow
        }
    }

    /// Check if a step's coordinates are within the region whitelist
    fn check_region_whitelist(&self, step: &DesktopStep) -> Option<RpaSafetyDecision> {
        let whitelist = self.region_whitelist.read();
        if whitelist.is_empty() {
            return None; // No whitelist configured = allow all
        }

        let (x, y) = match step {
            DesktopStep::MouseMove { x, y } => (*x, *y),
            DesktopStep::MouseClick { .. } => return None, // Clicks at current position are hard to check
            DesktopStep::MouseDrag { from, .. } => (from.x, from.y),
            _ => return None, // Non-spatial operations don't need region check
        };

        let in_whitelist = whitelist.iter().any(|region| {
            x >= region.x && x <= (region.x + region.width as i32)
                && y >= region.y && y <= (region.y + region.height as i32)
        });

        if in_whitelist {
            None // Within whitelist, allow
        } else {
            Some(RpaSafetyDecision::Block(format!(
                "Coordinates ({}, {}) are outside the allowed screen regions",
                x, y
            )))
        }
    }

    /// Add a region to the whitelist
    pub fn add_region(&self, region: ScreenRegion) {
        self.region_whitelist.write().push(region);
        debug!("Added screen region to whitelist: {:?}", region);
    }

    /// Clear the region whitelist
    pub fn clear_regions(&self) {
        self.region_whitelist.write().clear();
        debug!("Cleared screen region whitelist");
    }

    /// Get the region whitelist
    pub fn regions(&self) -> Vec<ScreenRegion> {
        self.region_whitelist.read().clone()
    }

    /// Update configuration
    pub fn update_config(&self, config: RpaSafetyConfig) {
        info!("RPA safety config updated: {:?}", config);
    }

    /// Record an audit entry
    fn audit_action(
        &self,
        action_type: &str,
        decision: &RpaSafetyDecision,
        user_id: Option<&str>,
        session_id: Option<&str>,
        details: Option<serde_json::Value>,
    ) {
        let entry = RpaAuditEntry {
            timestamp: Utc::now().timestamp_millis(),
            action_type: action_type.to_string(),
            decision: format!("{:?}", decision),
            reason: match decision {
                RpaSafetyDecision::Block(r) | RpaSafetyDecision::RequireConfirmation(r) | RpaSafetyDecision::Warn(r) => Some(r.clone()),
                _ => None,
            },
            user_id: user_id.map(String::from),
            session_id: session_id.map(String::from),
            details: details.unwrap_or(serde_json::Value::Null),
        };

        let mut log = self.audit_log.write();
        log.push(entry);

        // Keep last 10000 entries to prevent unbounded growth
        if log.len() > 10000 {
            let extra = log.len().saturating_sub(10000);
            if extra > 0 {
                log.drain(..extra);
            }
        }
    }

    /// Get audit log entries
    pub fn get_audit_log(&self, limit: usize) -> Vec<RpaAuditEntry> {
        let log = self.audit_log.read();
        log.iter().rev().take(limit).cloned().collect()
    }

    /// Reset the consecutive action counter
    pub fn reset_counter(&self) {
        *self.consecutive_actions.write() = 0;
    }
}

impl Default for RpaSafetyLayer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safety_config_default() {
        let config = RpaSafetyConfig::default();
        assert_eq!(config.max_actions_per_minute, 60);
        assert!(!config.confirmation_required);
        assert!(config.audit_logging);
    }

    #[test]
    fn test_safety_config_strict() {
        let config = RpaSafetyConfig::strict();
        assert_eq!(config.max_actions_per_minute, 30);
        assert!(config.confirmation_required);
        assert!(!config.allow_outside_regions);
    }

    #[tokio::test]
    async fn test_safety_layer_blocks_injection() {
        let layer = RpaSafetyLayer::with_config(RpaSafetyConfig::permissive());

        let step = DesktopStep::KeyboardType {
            text: "rm -rf /".to_string(),
        };

        let decision = layer.check_step(&step, None, None).await;
        assert!(matches!(decision, RpaSafetyDecision::Block(_)));
    }

    #[tokio::test]
    async fn test_safety_layer_allows_safe() {
        let layer = RpaSafetyLayer::with_config(RpaSafetyConfig::permissive());

        let step = DesktopStep::MouseMove { x: 100, y: 200 };
        let decision = layer.check_step(&step, None, None).await;
        assert!(matches!(decision, RpaSafetyDecision::Allow));
    }

    #[test]
    fn test_region_whitelist() {
        let layer = RpaSafetyLayer::new();

        assert!(layer.regions().is_empty());

        layer.add_region(ScreenRegion { x: 0, y: 0, width: 800, height: 600 });
        assert_eq!(layer.regions().len(), 1);

        layer.clear_regions();
        assert!(layer.regions().is_empty());
    }

    #[test]
    fn test_audit_logging() {
        let layer = RpaSafetyLayer::new();

        // Manually add audit entry
        let entry = RpaAuditEntry {
            timestamp: 1234567890,
            action_type: "mouse_move".to_string(),
            decision: "Allow".to_string(),
            reason: None,
            user_id: Some("test_user".to_string()),
            session_id: None,
            details: serde_json::Value::Null,
        };
        layer.audit_log.write().push(entry);

        let log = layer.get_audit_log(10);
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].action_type, "mouse_move");
    }
}
