//! Rule Actions
//!
//! Defines what happens when a rule matches.

/// Actions that can be taken when a rule matches
#[derive(Debug, Clone)]
pub enum Action {
    /// Allow the action to proceed
    Allow,
    /// Block the action with a reason
    Block(String),
    /// Require user confirmation with a message
    RequireConfirmation(String),
    /// Log the action but don't change its outcome
    Log(String),
    /// Redirect to an alternative tool/action
    Redirect {
        target: String,
        reason: String,
    },
    /// Transform the input before processing
    Transform {
        /// Description of the transformation (actual transform is external)
        transform_type: TransformType,
    },
    /// Allow with a warning message (logs but continues)
    Warn(String),
}

/// Types of input transformations
#[derive(Debug, Clone, PartialEq)]
pub enum TransformType {
    /// Sanitize dangerous characters
    Sanitize,
    /// Limit input length
    Truncate { max_length: usize },
    /// Normalize whitespace
    NormalizeWhitespace,
    /// Custom transformation (described by name)
    Custom(String),
}

impl Action {
    /// Check if this action blocks execution
    pub fn is_blocking(&self) -> bool {
        matches!(self, Action::Block(_))
    }

    /// Check if this action requires user interaction
    pub fn requires_confirmation(&self) -> bool {
        matches!(self, Action::RequireConfirmation(_))
    }

    /// Get the human-readable reason/message (if any)
    pub fn message(&self) -> Option<&str> {
        match self {
            Action::Block(msg) => Some(msg),
            Action::RequireConfirmation(msg) => Some(msg),
            Action::Log(msg) => Some(msg),
            Action::Redirect { reason, .. } => Some(reason),
            Action::Warn(msg) => Some(msg),
            Action::Allow | Action::Transform { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_properties() {
        assert!(!Action::Allow.is_blocking());
        assert!(!Action::Allow.requires_confirmation());

        assert!(Action::Block("test".to_string()).is_blocking());
        assert!(!Action::Block("test".to_string()).requires_confirmation());

        assert!(!Action::RequireConfirmation("test".to_string()).is_blocking());
        assert!(Action::RequireConfirmation("test".to_string()).requires_confirmation());
    }

    #[test]
    fn test_action_message() {
        assert!(Action::Block("forbidden".to_string()).message().is_some());
        assert_eq!(Action::Block("forbidden".to_string()).message(), Some("forbidden"));
        assert!(Action::Allow.message().is_none());
        assert!(Action::Warn("careful".to_string()).message().is_some());
    }
}
