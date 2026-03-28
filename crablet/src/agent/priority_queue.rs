//! Priority Message Queue for Swarm Communication
//!
//! Provides priority-based message handling:
//! - Multiple priority levels
//! - Deadline-aware scheduling
//! - Priority inheritance for responses

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::swarm::{AgentId, SwarmMessage};

// ============================================================================
// Priority Levels
// ============================================================================

/// Message priority levels (lower is higher priority)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(u8)]
pub enum MessagePriority {
    /// Direct interrupt - preempts everything
    Critical = 0,
    /// High priority task
    High = 1,
    /// Normal priority
    Normal = 2,
    /// Background task
    Low = 3,
}

impl MessagePriority {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            MessagePriority::Critical => "CRITICAL",
            MessagePriority::High => "HIGH",
            MessagePriority::Normal => "NORMAL",
            MessagePriority::Low => "LOW",
        }
    }

    /// Get description
    pub fn description(&self) -> &'static str {
        match self {
            MessagePriority::Critical => "Direct interrupt, preempts all",
            MessagePriority::High => "High priority task",
            MessagePriority::Normal => "Normal task",
            MessagePriority::Low => "Background task",
        }
    }
}

// ============================================================================
// Priority Message
// ============================================================================

/// A message with priority metadata
#[derive(Debug, Clone)]
pub struct PriorityMessage {
    /// Unique message ID
    pub id: String,
    /// Priority level
    pub priority: MessagePriority,
    /// The actual message
    pub message: SwarmMessage,
    /// Target agent
    pub to: AgentId,
    /// Source agent
    pub from: AgentId,
    /// Optional deadline (using SystemTime for Eq/Ord derivation compatibility instead of Instant)
    pub deadline: Option<std::time::SystemTime>,
    /// When the message was created
    pub created_at: std::time::SystemTime,
    /// Retry count for failed deliveries
    pub retry_count: u8,
    /// Maximum retries
    pub max_retries: u8,
    /// Custom metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl PartialEq for PriorityMessage {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for PriorityMessage {}

impl PriorityMessage {
    pub fn new(
        message: SwarmMessage,
        to: AgentId,
        from: AgentId,
        priority: MessagePriority,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            priority,
            message,
            to,
            from,
            deadline: None,
            created_at: std::time::SystemTime::now(),
            retry_count: 0,
            max_retries: 3,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Set a deadline for this message
    pub fn with_deadline(mut self, deadline: std::time::SystemTime) -> Self {
        self.deadline = Some(deadline);
        self
    }

    /// Set deadline from duration from now
    pub fn with_timeout(mut self, duration: Duration) -> Self {
        self.deadline = Some(std::time::SystemTime::now() + duration);
        self
    }

    /// Set max retries
    pub fn with_max_retries(mut self, max: u8) -> Self {
        self.max_retries = max;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Check if message has exceeded deadline
    pub fn is_overdue(&self) -> bool {
        self.deadline.map(|d| std::time::SystemTime::now() > d).unwrap_or(false)
    }

    /// Increment retry count
    pub fn retry(&mut self) {
        self.retry_count += 1;
    }

    /// Check if can retry
    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
    }
}

/// Ordering for priority queue (highest priority first)
impl Ord for PriorityMessage {
    fn cmp(&self, other: &Self) -> Ordering {
        // First compare by priority (lower = higher priority)
        match self.priority.cmp(&other.priority) {
            Ordering::Equal => {
                // Same priority: earlier deadline first
                match (self.deadline, other.deadline) {
                    (Some(d1), Some(d2)) => d1.cmp(&d2),
                    (Some(_), None) => Ordering::Less, // Has deadline = higher priority
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => {
                        // No deadline: earlier creation time first
                        self.created_at.cmp(&other.created_at)
                    }
                }
            }
            other => other,
        }
    }
}

impl PartialOrd for PriorityMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// ============================================================================
// Priority Queue
// ============================================================================

/// Thread-safe priority queue for messages
pub struct PriorityMessageQueue {
    /// The priority queue (uses BinaryHeap internally)
    queue: Arc<RwLock<BinaryHeap<Vec<PriorityMessage>>>>,
    /// Statistics
    stats: Arc<RwLock<QueueStats>>,
    /// Notification channel for new messages
    notification_tx: broadcast::Sender<QueueNotification>,
}

/// Queue statistics
#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    pub messages_enqueued: u64,
    pub messages_dequeued: u64,
    pub messages_dropped: u64,
    pub messages_retried: u64,
    pub priority_counts: [u64; 4],
}

impl QueueStats {
    pub fn record_enqueue(&mut self, priority: MessagePriority) {
        self.messages_enqueued += 1;
        self.priority_counts[priority as usize] += 1;
    }

    pub fn record_dequeue(&mut self) {
        self.messages_dequeued += 1;
    }

    pub fn record_drop(&mut self) {
        self.messages_dropped += 1;
    }

    pub fn record_retry(&mut self) {
        self.messages_retried += 1;
    }
}

/// Notification events from the queue
#[derive(Debug, Clone)]
pub enum QueueNotification {
    MessageEnqueued { id: String, priority: MessagePriority },
    MessageDequeued { id: String },
    MessageDropped { id: String, reason: String },
    QueueEmpty,
}

impl PriorityMessageQueue {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            queue: Arc::new(RwLock::new(BinaryHeap::new())),
            stats: Arc::new(RwLock::new(QueueStats::default())),
            notification_tx: tx,
        }
    }

    /// Enqueue a message
    pub async fn enqueue(&self, msg: PriorityMessage) {
        let mut queue = self.queue.write().await;
        queue.push(vec![msg.clone()]);

        // Update stats
        let mut stats = self.stats.write().await;
        stats.record_enqueue(msg.priority);

        // Send notification
        let _ = self.notification_tx.send(QueueNotification::MessageEnqueued {
            id: msg.id,
            priority: msg.priority,
        });
    }

    /// Dequeue the highest priority message
    pub async fn dequeue(&self) -> Option<PriorityMessage> {
        let mut queue = self.queue.write().await;

        // Find a non-empty batch
        while let Some(mut batch) = queue.pop() {
            if batch.is_empty() {
                continue;
            }

            if batch.len() == 1 {
                // Update stats
                let mut stats = self.stats.write().await;
                stats.record_dequeue();

                // Send notification
                let _ = self.notification_tx.send(QueueNotification::MessageDequeued {
                    id: batch[0].id.clone(),
                });

                return Some(batch.into_iter().next().unwrap());
            }

            // Take first message from batch
            let msg = batch.remove(0);

            // Push remaining back
            if !batch.is_empty() {
                queue.push(batch);
            }

            // Update stats
            let mut stats = self.stats.write().await;
            stats.record_dequeue();

            // Send notification
            let _ = self.notification_tx.send(QueueNotification::MessageDequeued {
                id: msg.id.clone(),
            });

            return Some(msg);
        }

        // Queue empty notification
        let _ = self.notification_tx.send(QueueNotification::QueueEmpty);

        None
    }

    /// Peek at highest priority message without removing
    pub async fn peek(&self) -> Option<PriorityMessage> {
        let queue = self.queue.read().await;
        queue.peek().and_then(|batch| batch.first().cloned())
    }

    /// Get queue length
    pub async fn len(&self) -> usize {
        let queue = self.queue.read().await;
        queue.iter().map(|batch| batch.len()).sum()
    }

    /// Check if queue is empty
    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// Get a message by ID (mostly for internal use/testing)
    pub async fn get(&self, id: &str) -> Option<PriorityMessage> {
        let queue = self.queue.read().await;
        // Since BinaryHeap doesn't support random access easily, we need to iterate
        for batch in queue.iter() {
            if let Some(pos) = batch.iter().position(|m: &PriorityMessage| m.id == id) {
                return Some(batch[pos].clone());
            }
        }
        None
    }

    /// Cancel/retry a specific message by ID
    /// Note: This is O(n) since BinaryHeap doesn't support direct mutation
    pub async fn cancel_message(&self, id: &str) -> bool {
        let mut queue_guard = self.queue.write().await;

        // Drain and rebuild to find and modify the message
        let mut new_queue = BinaryHeap::new();
        let mut found = false;
        let mut should_retry = false;
        let mut target_msg: Option<PriorityMessage> = None;

        // Extract all batches
        let mut batches: Vec<Vec<PriorityMessage>> = Vec::new();
        while let Some(batch) = queue_guard.pop() {
            batches.push(batch);
        }

        // Find and modify the target message
        for batch in &mut batches {
            if let Some(pos) = batch.iter().position(|m| m.id == id) {
                let msg = batch.remove(pos);
                found = true;
                target_msg = Some(msg);
                break;
            }
        }

        if let Some(mut msg) = target_msg {
            if msg.can_retry() {
                msg.retry();
                should_retry = true;
                // Put back the modified message and remaining batch
                new_queue.push(vec![msg]);
            } else {
                // Drop the message
                drop(msg);
                let mut stats = self.stats.write().await;
                stats.record_drop();
                let _ = self.notification_tx.send(QueueNotification::MessageDropped {
                    id: id.to_string(),
                    reason: "Max retries exceeded".to_string(),
                });
            }
        }

        // Rebuild remaining batches
        for batch in batches {
            if !batch.is_empty() {
                new_queue.push(batch);
            }
        }

        // Replace the queue
        *queue_guard = new_queue;

        if should_retry {
            let mut stats = self.stats.write().await;
            stats.record_retry();
        }

        found
    }

    /// Get statistics
    pub async fn get_stats(&self) -> QueueStats {
        self.stats.read().await.clone()
    }

    /// Subscribe to notifications
    pub fn subscribe(&self) -> broadcast::Receiver<QueueNotification> {
        self.notification_tx.subscribe()
    }

    /// Clear all messages
    pub async fn clear(&self) {
        let mut queue = self.queue.write().await;
        queue.clear();
    }

    /// Get messages by priority
    pub async fn get_by_priority(&self, priority: MessagePriority) -> Vec<PriorityMessage> {
        let queue = self.queue.read().await;
        queue
            .iter()
            .flat_map(|batch| batch.iter().filter(|m| m.priority == priority).cloned())
            .collect()
    }

    /// Get overdue messages
    pub async fn get_overdue(&self) -> Vec<PriorityMessage> {
        let queue = self.queue.read().await;
        queue
            .iter()
            .flat_map(|batch| batch.iter().filter(|m| m.is_overdue()).cloned())
            .collect()
    }
}

impl Default for PriorityMessageQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Integration with Swarm
// ============================================================================

/// Priority-aware mailbox for agents
pub struct PriorityMailbox {
    queue: PriorityMessageQueue,
    agent_id: AgentId,
}

impl PriorityMailbox {
    pub fn new(agent_id: AgentId) -> Self {
        Self {
            queue: PriorityMessageQueue::new(),
            agent_id,
        }
    }

    /// Send a message to this mailbox
    pub async fn send(&self, msg: PriorityMessage) {
        self.queue.enqueue(msg).await;
    }

    /// Receive next message (blocking)
    pub async fn recv(&self) -> Option<PriorityMessage> {
        self.queue.dequeue().await
    }

    /// Get current queue depth
    pub async fn depth(&self) -> usize {
        self.queue.len().await
    }

    /// Get the agent ID
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_agent_id(name: &str) -> AgentId {
        AgentId(name.to_string())
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let queue = PriorityMessageQueue::new();

        // Create messages with different priorities
        let low = PriorityMessage::new(
            SwarmMessage::Result {
                task_id: "1".to_string(),
                content: "low".to_string(),
                payload: None,
            },
            create_test_agent_id("agent1"),
            create_test_agent_id("agent2"),
            MessagePriority::Low,
        );

        let critical = PriorityMessage::new(
            SwarmMessage::Result {
                task_id: "2".to_string(),
                content: "critical".to_string(),
                payload: None,
            },
            create_test_agent_id("agent1"),
            create_test_agent_id("agent2"),
            MessagePriority::Critical,
        );

        let normal = PriorityMessage::new(
            SwarmMessage::Result {
                task_id: "3".to_string(),
                content: "normal".to_string(),
                payload: None,
            },
            create_test_agent_id("agent1"),
            create_test_agent_id("agent2"),
            MessagePriority::Normal,
        );

        // Enqueue in reverse order
        queue.enqueue(low.clone()).await;
        queue.enqueue(normal.clone()).await;
        queue.enqueue(critical.clone()).await;

        // Dequeue should get critical first
        let first = queue.dequeue().await;
        assert!(first.is_some());
        assert_eq!(first.unwrap().priority, MessagePriority::Critical);

        let second = queue.dequeue().await;
        assert!(second.is_some());
        assert_eq!(second.unwrap().priority, MessagePriority::Normal);

        let third = queue.dequeue().await;
        assert!(third.is_some());
        assert_eq!(third.unwrap().priority, MessagePriority::Low);
    }

    #[tokio::test]
    async fn test_deadline_ordering() {
        let queue = PriorityMessageQueue::new();

        let msg1 = PriorityMessage::new(
            SwarmMessage::Result {
                task_id: "1".to_string(),
                content: "later".to_string(),
                payload: None,
            },
            create_test_agent_id("agent1"),
            create_test_agent_id("agent2"),
            MessagePriority::Normal,
        )
        .with_deadline(std::time::SystemTime::now() + Duration::from_secs(100));

        let msg2 = PriorityMessage::new(
            SwarmMessage::Result {
                task_id: "2".to_string(),
                content: "sooner".to_string(),
                payload: None,
            },
            create_test_agent_id("agent1"),
            create_test_agent_id("agent2"),
            MessagePriority::Normal,
        )
        .with_deadline(std::time::SystemTime::now() + Duration::from_secs(10));

        queue.enqueue(msg1).await;
        queue.enqueue(msg2).await;

        // Earlier deadline should come first
        let first = queue.dequeue().await;
        assert!(first.is_some());
        if let SwarmMessage::Result { content, .. } = &first.unwrap().message {
            assert!(content.contains("sooner"));
        } else {
            panic!("Expected SwarmMessage::Result");
        }
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let queue = PriorityMessageQueue::new();

        queue
            .enqueue(PriorityMessage::new(
                SwarmMessage::Result {
                    task_id: "1".to_string(),
                    content: "test".to_string(),
                    payload: None,
                },
                create_test_agent_id("agent1"),
                create_test_agent_id("agent2"),
                MessagePriority::High,
            ))
            .await;

        queue.dequeue().await;

        let stats = queue.get_stats().await;
        assert_eq!(stats.messages_enqueued, 1);
        assert_eq!(stats.messages_dequeued, 1);
        assert_eq!(stats.priority_counts[MessagePriority::High as usize], 1);
    }

    #[tokio::test]
    async fn test_mailbox() {
        let mailbox = PriorityMailbox::new(create_test_agent_id("test_agent"));

        let msg = PriorityMessage::new(
            SwarmMessage::Result {
                task_id: "1".to_string(),
                content: "test".to_string(),
                payload: None,
            },
            create_test_agent_id("agent1"),
            create_test_agent_id("test_agent"),
            MessagePriority::Normal,
        );

        mailbox.send(msg).await;

        let depth = mailbox.depth().await;
        assert_eq!(depth, 1);

        assert_eq!(mailbox.agent_id().0, "test_agent");
    }
}