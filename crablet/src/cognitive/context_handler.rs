//! Context Handler - Context-aware response enhancement
//!
//! Analyzes conversation context to provide context-aware responses,
//! including topic detection, repetition detection, and confirmation handling.

use crate::types::Message;

/// Context information extracted from conversation history
#[derive(Default, Debug)]
pub struct ContextInfo {
    pub last_role: String,
    pub turn_count: usize,
    pub expecting_confirmation: bool,
    pub topic: Option<String>,
    pub is_repeating: bool,
}

/// Context handler for analyzing conversation context
pub struct ContextHandler;

impl ContextHandler {
    /// Analyze conversation context to enhance matching
    pub fn analyze_context(context: &[Message]) -> ContextInfo {
        let mut info = ContextInfo::default();

        if context.is_empty() {
            return info;
        }

        // Check last message role
        if let Some(last) = context.last() {
            info.last_role = last.role.clone();

            // Check if waiting for confirmation
            let last_content = get_message_text(last).to_lowercase();
            if last_content.contains("?")
                || last_content.contains("confirm")
                || last_content.contains("sure")
                || last_content.contains("ok")
            {
                info.expecting_confirmation = true;
            }

            // Detect conversation topic
            if last_content.contains("code") || last_content.contains("function") {
                info.topic = Some("coding".to_string());
            } else if last_content.contains("search") || last_content.contains("find") {
                info.topic = Some("search".to_string());
            }
        }

        // Count turns
        info.turn_count = context.len();

        // Check for repeated patterns
        let user_messages: Vec<_> = context.iter().filter(|m| m.role == "user").collect();

        if user_messages.len() >= 2 {
            let last_two: Vec<_> = user_messages.iter().rev().take(2).collect();
            if get_message_text(last_two[0]).to_lowercase()
                == get_message_text(last_two[1]).to_lowercase()
            {
                info.is_repeating = true;
            }
        }

        info
    }

    /// Get contextual response modifier
    pub fn get_context_modifier(info: &ContextInfo) -> Option<String> {
        if info.is_repeating {
            Some("I notice you've asked this before. ".to_string())
        } else if info.turn_count > 10 {
            Some("We've been chatting for a while! ".to_string())
        } else {
            None
        }
    }
}

/// Helper function to extract text content from Message
pub(crate) fn get_message_text(msg: &Message) -> String {
    match &msg.content {
        Some(parts) => parts
            .iter()
            .filter_map(|part| match part {
                crate::types::ContentPart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(""),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ContentPart;

    #[test]
    fn test_empty_context() {
        let info = ContextHandler::analyze_context(&[]);
        assert_eq!(info.turn_count, 0);
        assert!(!info.expecting_confirmation);
    }

    #[test]
    fn test_confirmation_detection() {
        let context = vec![Message {
            role: "assistant".to_string(),
            content: Some(vec![ContentPart::Text {
                text: "Do you want me to proceed?".to_string(),
            }]),
            tool_calls: None,
            tool_call_id: None,
        }];
        let info = ContextHandler::analyze_context(&context);
        assert!(info.expecting_confirmation);
    }

    #[test]
    fn test_topic_detection() {
        let context = vec![Message {
            role: "user".to_string(),
            content: Some(vec![ContentPart::Text {
                text: "Can you write some code?".to_string(),
            }]),
            tool_calls: None,
            tool_call_id: None,
        }];
        let info = ContextHandler::analyze_context(&context);
        assert_eq!(info.topic.as_deref(), Some("coding"));
    }

    #[test]
    fn test_repetition_detection() {
        let context = vec![
            Message {
                role: "user".to_string(),
                content: Some(vec![ContentPart::Text {
                    text: "hello".to_string(),
                }]),
                tool_calls: None,
                tool_call_id: None,
            },
            Message {
                role: "assistant".to_string(),
                content: Some(vec![ContentPart::Text {
                    text: "Hi there!".to_string(),
                }]),
                tool_calls: None,
                tool_call_id: None,
            },
            Message {
                role: "user".to_string(),
                content: Some(vec![ContentPart::Text {
                    text: "hello".to_string(),
                }]),
                tool_calls: None,
                tool_call_id: None,
            },
        ];
        let info = ContextHandler::analyze_context(&context);
        assert!(info.is_repeating);
    }

    #[test]
    fn test_context_modifier_repeating() {
        let info = ContextInfo {
            is_repeating: true,
            ..Default::default()
        };
        assert!(ContextHandler::get_context_modifier(&info).is_some());
    }

    #[test]
    fn test_context_modifier_long_conversation() {
        let info = ContextInfo {
            turn_count: 15,
            ..Default::default()
        };
        assert!(ContextHandler::get_context_modifier(&info).is_some());
    }
}
