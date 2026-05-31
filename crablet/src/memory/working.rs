#[cfg(feature = "knowledge")]
use crate::memory::consolidator::MemoryConsolidator;
use crate::types::{ContentPart, Message};
use std::collections::VecDeque;
#[cfg(feature = "knowledge")]
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tiktoken_rs::cl100k_base;
use tracing::info;

static BPE: OnceLock<tiktoken_rs::CoreBPE> = OnceLock::new();

#[derive(Clone)]
pub struct WorkingMemory {
    pub capacity_messages: usize, // Soft limit on message count
    pub max_tokens: usize,        // Hard limit on tokens (e.g., 4000, 8000)
    pub history: VecDeque<Message>,
    pub last_accessed: Instant,
    #[cfg(feature = "knowledge")]
    pub consolidator: Option<Arc<MemoryConsolidator>>,
}

impl WorkingMemory {
    pub fn new(capacity_messages: usize, max_tokens: usize) -> Self {
        Self {
            capacity_messages,
            max_tokens,
            history: VecDeque::with_capacity(capacity_messages),
            last_accessed: Instant::now(),
            #[cfg(feature = "knowledge")]
            consolidator: None,
        }
    }

    #[cfg(feature = "knowledge")]
    pub fn with_consolidator(mut self, consolidator: Arc<MemoryConsolidator>) -> Self {
        self.consolidator = Some(consolidator);
        self
    }

    pub fn add_message(&mut self, role: &str, content: &str) {
        self.last_accessed = Instant::now();
        self.history.push_back(Message::new(role, content));
        self.compress_context();
    }

    pub fn add_full_message(&mut self, message: Message) {
        self.last_accessed = Instant::now();
        self.history.push_back(message);
        self.compress_context();
    }

    pub fn count_tokens(&self, text: &str) -> usize {
        BPE.get_or_init(|| cl100k_base().expect("Failed to init tokenizer"))
            .encode_with_special_tokens(text)
            .len()
    }

    pub fn estimate_message_tokens(&self, msg: &Message) -> usize {
        // Simple estimation including role overhead
        let content_str = msg
            .content
            .as_ref()
            .map(|parts| {
                parts
                    .iter()
                    .map(|p| match p {
                        ContentPart::Text { text } => text.as_str(),
                        _ => "",
                    })
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        self.count_tokens(&content_str) + 4 // +4 for role/structure overhead
    }

    pub fn compress_context(&mut self) {
        self.last_accessed = Instant::now();

        // 1. Check Token Limit
        let mut total_tokens: usize = self
            .history
            .iter()
            .map(|m| self.estimate_message_tokens(m))
            .sum();

        if total_tokens <= self.max_tokens {
            // If within token limit, check message count limit but be lenient if tokens are low
            if self.history.len() <= self.capacity_messages {
                return;
            }
            // If messages > capacity but tokens low, maybe keep them?
            // For now, enforce message limit to prevent infinite context drift
        }

        // Strategy:
        // 1. Preserve System Message (index 0)
        // 2. Preserve last N messages (recent context)
        // 3. Consolidate or Drop middle messages

        let preserve_recent = 4; // Keep last 2 exchanges

        while (total_tokens > self.max_tokens || self.history.len() > self.capacity_messages)
            && self.history.len() > preserve_recent + 1
        {
            if self.history.len() > 1 {
                if let Some(removed) = self.pop_oldest_after_initial_message() {
                    let tokens_removed = self.estimate_message_tokens(&removed);
                    total_tokens = total_tokens.saturating_sub(tokens_removed);

                    // Ideally: Consolidate 'removed' into a summary
                    // But we can't do async here easily in sync method.
                    // We rely on background consolidation or MemoryManager to handle semantic archival.
                    // Here we focused on Working Memory truncation.
                    info!("WorkingMemory: Truncated message (role: {}) to fit context window. Freed {} tokens.", removed.role, tokens_removed);
                } else {
                    break;
                }
            } else {
                break; // Only system left?
            }
        }
    }

    fn pop_oldest_after_initial_message(&mut self) -> Option<Message> {
        let initial = self.history.pop_front()?;
        let removed = self.history.pop_front();
        self.history.push_front(initial);
        removed
    }

    pub fn clear(&mut self) {
        self.last_accessed = Instant::now();
        self.history.clear();
    }

    pub fn get_context(&self) -> Vec<Message> {
        self.history.iter().cloned().collect()
    }

    pub fn is_expired(&self, ttl: Duration) -> bool {
        self.last_accessed.elapsed() > ttl
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pop_oldest_after_initial_message_preserves_initial() {
        let mut memory = WorkingMemory::new(10, 1000);
        memory.add_message("system", "system");
        memory.add_message("user", "oldest");
        memory.add_message("assistant", "newest");

        let removed = memory
            .pop_oldest_after_initial_message()
            .expect("middle message should be removed");

        let context = memory.get_context();
        assert_eq!(removed.text().as_deref(), Some("oldest"));
        assert_eq!(context[0].text().as_deref(), Some("system"));
        assert_eq!(context[1].text().as_deref(), Some("newest"));
    }
}
