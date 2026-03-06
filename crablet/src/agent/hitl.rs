use std::sync::Arc;
use dashmap::DashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, oneshot};
use tokio::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ReviewType {
    Approval,
    Edit,
    Selection,
    FreeformFeedback,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum HumanDecision {
    Approved,
    Rejected(String),
    Edited(String),
    Selected(usize),
    Feedback(String),
    Timeout,
}

pub struct PendingReview {
    pub review_id: String,
    pub graph_id: String,
    pub task_id: String,
    pub agent_output: String,
    pub review_type: ReviewType,
    pub deadline: DateTime<Utc>,
    pub options: Vec<String>,
    response_channel: Option<oneshot::Sender<HumanDecision>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "payload")]
pub enum HITLNotification {
    ReviewRequested {
        review_id: String,
        graph_id: String,
        task_id: String,
        review_type: ReviewType,
        deadline: DateTime<Utc>,
    },
    ReviewResolved {
        review_id: String,
        task_id: String,
        decision: HumanDecision,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingReviewView {
    pub review_id: String,
    pub graph_id: String,
    pub task_id: String,
    pub agent_output: String,
    pub review_type: ReviewType,
    pub deadline: DateTime<Utc>,
    pub options: Vec<String>,
}

pub struct HumanInTheLoop {
    pending_reviews: Arc<DashMap<String, PendingReview>>,
    notification_channel: broadcast::Sender<HITLNotification>,
}

impl Default for HumanInTheLoop {
    fn default() -> Self {
        Self::new()
    }
}

impl HumanInTheLoop {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            pending_reviews: Arc::new(DashMap::new()),
            notification_channel: tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<HITLNotification> {
        self.notification_channel.subscribe()
    }

    pub fn list_pending(&self) -> Vec<PendingReviewView> {
        self.pending_reviews
            .iter()
            .map(|entry| PendingReviewView {
                review_id: entry.value().review_id.clone(),
                graph_id: entry.value().graph_id.clone(),
                task_id: entry.value().task_id.clone(),
                agent_output: entry.value().agent_output.clone(),
                review_type: entry.value().review_type.clone(),
                deadline: entry.value().deadline,
                options: entry.value().options.clone(),
            })
            .collect()
    }

    pub async fn request_review(
        &self,
        graph_id: &str,
        task_id: &str,
        output: &str,
        review_type: ReviewType,
    ) -> HumanDecision {
        self.request_review_with_timeout(graph_id, task_id, output, review_type, Duration::from_secs(600), vec![]).await
    }

    pub async fn request_review_with_timeout(
        &self,
        graph_id: &str,
        task_id: &str,
        output: &str,
        review_type: ReviewType,
        timeout: Duration,
        options: Vec<String>,
    ) -> HumanDecision {
        let (tx, rx) = oneshot::channel();
        let review_id = uuid::Uuid::new_v4().to_string();
        let deadline = Utc::now()
            + chrono::Duration::from_std(timeout)
                .unwrap_or_else(|_| chrono::Duration::minutes(10));

        let review = PendingReview {
            review_id: review_id.clone(),
            graph_id: graph_id.to_string(),
            task_id: task_id.to_string(),
            agent_output: output.to_string(),
            review_type: review_type.clone(),
            deadline,
            options,
            response_channel: Some(tx),
        };

        self.pending_reviews.insert(task_id.to_string(), review);
        let _ = self.notification_channel.send(HITLNotification::ReviewRequested {
            review_id: review_id.clone(),
            graph_id: graph_id.to_string(),
            task_id: task_id.to_string(),
            review_type,
            deadline,
        });

        let decision = match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(d)) => d,
            _ => HumanDecision::Timeout,
        };

        self.pending_reviews.remove(task_id);
        let _ = self.notification_channel.send(HITLNotification::ReviewResolved {
            review_id,
            task_id: task_id.to_string(),
            decision: decision.clone(),
        });
        decision
    }

    pub fn submit_decision(&self, task_id: &str, decision: HumanDecision) -> bool {
        if let Some((_, mut pending)) = self.pending_reviews.remove(task_id) {
            if let Some(tx) = pending.response_channel.take() {
                let _ = tx.send(decision);
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hitl_submit_decision_works() {
        let hitl = HumanInTheLoop::new();
        let hitl2 = HumanInTheLoop {
            pending_reviews: hitl.pending_reviews.clone(),
            notification_channel: hitl.notification_channel.clone(),
        };
        let task_id = "t-approve";
        let wait = tokio::spawn(async move {
            hitl2
                .request_review_with_timeout("g1", task_id, "output", ReviewType::Approval, Duration::from_secs(2), vec![])
                .await
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        let ok = hitl.submit_decision(task_id, HumanDecision::Approved);
        assert!(ok);
        let decision = wait.await.expect("join").clone();
        match decision {
            HumanDecision::Approved => {}
            _ => panic!("unexpected decision"),
        }
    }

    #[tokio::test]
    async fn hitl_timeout_returns_timeout() {
        let hitl = HumanInTheLoop::new();
        let decision = hitl
            .request_review_with_timeout("g1", "t-timeout", "output", ReviewType::Approval, Duration::from_millis(80), vec![])
            .await;
        match decision {
            HumanDecision::Timeout => {}
            _ => panic!("expected timeout"),
        }
    }
}
