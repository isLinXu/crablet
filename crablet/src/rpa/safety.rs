//! RPA Safety Layer
//!
//! Provides security controls for desktop automation operations.
//! Every RPA action must pass through this layer before execution.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use parking_lot::RwLock;
use tracing::{info, warn, error, debug};
use governor::{Quota, RateLimiter as GovernorLimiter};
use chrono::Utc;

use crate::rpa::desktop::{DesktopStep, MouseButton, Region};
use crate::rules::{RuleEngine, RuleContext, RuleDecision};

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
    rule_engine: Arc<RuleEngine>,
    region_whitelist: RwLock<Vec<ScreenRegion>>,
    /// Rate limiter for actions per minute
    rate_limiter: Arc<GovernorLimiter<guarantor::direct::DirectCellBucket>>,
    /// Consecutive action counter
    consecutive_actions: RwLock<u32>,
    /// Audit log buffer
    audit_log: Arc<RwLock<Vec<RpaAuditEntry>>>,
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
        let rate_limiter = Arc::new(GovernorLimiter::direct(quota));

        let rule_engine = Arc::new(RuleEngine::new());
        // Add built-in RPA safety rules
        Self::add_builtin_rpa_rules(&rule_engine);

        Self {
            config,
            rule_engine,
            region_whitelist: RwLock::new(Vec::new()),
            rate_limiter,
            consecutive_actions: RwLock::new(0),
            audit_log: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Add built-in safety rules for RPA operations
    fn add_builtin_rpa_rules(engine: &RuleEngine) {
        use crate::rules::{Condition, Action, Rule};

        // Block potential malicious patterns in keyboard input
        engine.add_rule(Rule::builder("rpa-block-cmd-injection")
            .name("Block command injection via keyboard")
            .priority(100)
            .description("Blocks typing of known command injection patterns")
            .condition(Condition::All(vec![
                Condition::RpaActionIs("keyboard_type".to_string()),
                Condition::Any(vec![
                    Condition::Regex(regex::Regex::new(r"(?i)rm\s+-rf").unwrap()),
                    Condition::Regex(regex::Regex::new(r"(?i)sudo\s+").unwrap()),
                    Condition::Regex(regex::Regex::new(r"(?i)chmod\s+777").unwrap()),
                    Condition::Regex(regex::Regex::new(r"(?i):\(\)\{:\|:&\}").unwrap()),
                ]),
            ]))
            .action(Action::Block("Potential command injection detected in keyboard input".to_string()))
            .build()
        );

        // Require confirmation for hotkey combinations that could be dangerous
        engine.add_rule(Rule::builder("rpa-confirm-dangerous-hotkey")
            .name("Confirm dangerous hotkeys")
            .priority(80)
            .condition(Condition::RpaActionIs("keyboard_hotkey".to_string()))
            .action(Action::RequireConfirmation(
                "Hotkey combination requires confirmation".to_string()
            ))
            .build()
        );
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
            Err(negative) => {
                let wait = negative.wait_time_from(Instant::now());
                warn!("RPA rate limit exceeded, wait time: {:?}", wait);
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

        // 3. Build rule context from step
        let ctx = self.step_to_context(step, user_id, session_id);

        // 4. Evaluate rules
        let decision = self.rule_engine.evaluate(&ctx);

        let rpa_decision = match &decision {
            RuleDecision::Allow => RpaSafetyDecision::Allow,
            RuleDecision::Block(reason) => RpaSafetyDecision::Block(reason.clone()),
            RuleDecision::RequireConfirmation(msg) => RpaSafetyDecision::RequireConfirmation(msg.clone()),
            RuleDecision::NoMatch => {
                // No rule matched — check region whitelist
                if !self.config.allow_outside_regions {
                    if let Some(region_check) = self.check_region_whitelist(step) {
                        region_check
                    } else {
                        RpaSafetyDecision::Allow
                    }
                } else {
                    RpaSafetyDecision::Allow
                }
            }
        };

        // 5. Audit log
        if self.config.audit_logging {
            let details = serde_json::json!({
                "step": format!("{:?}", step),
            });
            self.audit_action(
                &format!("{:?}", std::mem::discriminant(step)),
                &rpa_decision,
                user_id,
                session_id,
                Some(details),
            );
        }

        // 6. If confirmation is globally required, upgrade Allow to RequireConfirmation
        if matches!(rpa_decision, RpaSafetyDecision::Allow) && self.config.confirmation_required {
            RpaSafetyDecision::RequireConfirmation(
                "Global confirmation required for RPA operations".to_string()
            )
        } else {
            rpa_decision
        }
    }

    /// Convert a DesktopStep into a RuleContext
    fn step_to_context(&self, step: &DesktopStep, user_id: Option<&str>, session_id: Option<&str>) -> RuleContext {
        let (action_name, region) = match step {
            DesktopStep::MouseMove { x, y } => ("mouse_move", Some((*x, *y, 0u32, 0u32))),
            DesktopStep::MouseClick { button } => ("mouse_click", None),
            DesktopStep::MouseDrag { from, to } => ("mouse_drag", Some((from.x, from.y, (to.x - from.x) as u32, (to.y - from.y) as u32))),
            DesktopStep::KeyboardType { text } => {
                let mut ctx = RuleContext::for_rpa("keyboard_type", None);
                if let Some(uid) = user_id { ctx.user_id = Some(uid.to_string()); }
                if let Some(sid) = session_id { ctx.session_id = Some(sid.to_string()); }
                ctx.input = Some(text.clone());
                return ctx;
            }
            DesktopStep::KeyboardHotkey { keys } => {
                let key_str = format!("{:?}", keys);
                let mut ctx = RuleContext::for_rpa("keyboard_hotkey", None);
                if let Some(uid) = user_id { ctx.user_id = Some(uid.to_string()); }
                if let Some(sid) = session_id { ctx.session_id = Some(sid.to_string()); }
                ctx.input = Some(key_str);
                return ctx;
            }
            DesktopStep::Screenshot { region, .. } => ("screenshot", region.map(|r| (r.x, r.y, r.width, r.height))),
            DesktopStep::Wait { .. } => ("wait", None),
            DesktopStep::ClipboardSet { .. } => ("clipboard_set", None),
            DesktopStep::ClipboardGet { .. } => ("clipboard_get", None),
            DesktopStep::FindAndClick { .. } => ("find_and_click", None),
        };

        let whitelist: Vec<(i32, i32, u32, u32)> = self.region_whitelist.read()
            .iter()
            .map(|r| (r.x, r.y, r.width, r.height))
            .collect();

        RuleContext::for_rpa(action_name, region)
            .with_user(user_id.unwrap_or_default())
            .with_session(session_id.unwrap_or_default())
            .with_rpa_whitelist(whitelist)
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
        // Note: In a real implementation, you'd use RwLock for config too
        // For now we log the update
        info!("RPA safety config updated: {:?}", config);
    }

    /// Get access to the underlying rule engine
    pub fn rule_engine(&self) -> &RuleEngine {
        &self.rule_engine
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
            log.drain(..log.len() - 10000);
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
