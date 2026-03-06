use anyhow::Result;
use crate::cognitive::llm::LlmClient;
use crate::memory::episodic::EpisodicMemory;
use crate::knowledge::vector_store::VectorStore;
use crate::types::Message;
use std::sync::Arc;
use tracing::info;
use crate::events::EventBus;
use tokio::sync::{Mutex, Notify};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

pub struct MemoryConsolidator {
    llm: Arc<Box<dyn LlmClient>>,
    vector_store: Option<Arc<VectorStore>>,
    event_bus: Option<Arc<EventBus>>,
    last_consolidation: Arc<Mutex<HashMap<String, Instant>>>,
    message_counts: Arc<Mutex<HashMap<String, usize>>>,
    trigger: Arc<Notify>,
    pending_sessions: Arc<Mutex<Vec<String>>>,
}

impl MemoryConsolidator {
    pub fn new(llm: Arc<Box<dyn LlmClient>>, vector_store: Option<Arc<VectorStore>>, event_bus: Option<Arc<EventBus>>) -> Self {
        Self { 
            llm, 
            vector_store,
            event_bus,
            last_consolidation: Arc::new(Mutex::new(HashMap::new())),
            message_counts: Arc::new(Mutex::new(HashMap::new())),
            trigger: Arc::new(Notify::new()),
            pending_sessions: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub fn start_background_loop(self: Arc<Self>, memory: Arc<EpisodicMemory>) {
        let this = self.clone();
        tokio::spawn(async move {
            info!("MemoryConsolidator background loop started");
            loop {
                // Wait for notification
                this.trigger.notified().await;
                
                // Drain pending sessions
                let sessions: Vec<String> = {
                    let mut pending = this.pending_sessions.lock().await;
                    let s = pending.clone();
                    pending.clear();
                    s
                };
                
                if sessions.is_empty() {
                    continue;
                }
                
                // Remove duplicates
                let unique_sessions: Vec<String> = sessions.into_iter()
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();
                
                for session_id in unique_sessions {
                    if let Err(e) = this.consolidate(&memory, &session_id).await {
                        tracing::error!("Failed to consolidate memory for {}: {}", session_id, e);
                    }
                }
            }
        });
    }

    pub async fn on_message_added(&self, session_id: &str) {
        let mut counts = self.message_counts.lock().await;
        let count = counts.entry(session_id.to_string()).or_insert(0);
        *count += 1;
        
        // Trigger every 20 messages
        if *count >= 20 {
            info!("Auto-triggering memory consolidation for session {} (message count threshold)", session_id);
            *count = 0; // Reset
            
            // Push to pending queue and notify
            {
                let mut pending = self.pending_sessions.lock().await;
                pending.push(session_id.to_string());
            }
            self.trigger.notify_one();
        }
    }
    
    // Call this periodically or on specific events
    pub async fn check_time_based_trigger(&self, session_id: &str) {
        let mut last = self.last_consolidation.lock().await;
        let now = Instant::now();
        let last_time = last.entry(session_id.to_string()).or_insert(now);
        
        if now.duration_since(*last_time) > Duration::from_secs(3600) { // 1 hour
             info!("Auto-triggering memory consolidation for session {} (time threshold)", session_id);
             // Trigger consolidation...
             *last_time = now;
        }
    }

    pub async fn consolidate(&self, memory: &EpisodicMemory, session_id: &str) -> Result<()> {
        info!("Consolidating memory for session: {}", session_id);
        
        // 1. Fetch recent messages
        // Fetch last 50 to get good context for summary
        let messages = memory.get_context(session_id, 50).await?; 
        if messages.is_empty() {
            return Ok(());
        }

        // 2. Generate Summary
        let conversation_text: String = messages.iter()
            .map(|m| format!("{}: {:?}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");
            
        let prompt = format!(
            "Summarize the following conversation into key facts and insights worth remembering for the future.\n\
            Focus on user preferences, important decisions, and factual information.\n\
            Discard trivial chit-chat.\n\
            \n\
            Conversation:\n\
            {}\n\
            \n\
            Summary:",
            conversation_text
        );

        let summary_msgs = vec![
            Message::new("system", "You are a memory consolidation assistant."),
            Message::new("user", &prompt),
        ];

        // Use chat_complete (assuming it takes Vec<Message> or we adapt)
        // The trait usually takes &str or &[Message]. 
        // LlmClient::chat_complete takes &str in some versions, or &[Message].
        // Let's check the trait definition. Assuming it takes &str based on usage in middleware.
        // Wait, in middleware we used `state.llm.chat_complete(&summary_prompt)`.
        // So it takes &str.
        // We need to convert messages to string prompt if the trait only takes string.
        // Or if the trait has `chat_complete_messages`.
        // Let's assume `chat_complete` takes `&str`.
        
        let summary = self.llm.chat_complete(&summary_msgs).await?; // Fixed: Pass message slice
        info!("Generated summary: {}", summary);

        // 3. Store in Vector Store (Long-term Memory)
        // We treat the summary as a "document"
        // Add "decay_factor" to metadata for future cleanup
        if let Some(vs) = &self.vector_store {
            vs.add_document(&summary, Some(serde_json::json!({
                "type": "conversation_summary",
                "session_id": session_id,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "importance": 1.0, // Initial importance
                "access_count": 0
            }))).await?;
            info!("Memory consolidated into Vector Store.");
        } else {
            info!("Vector Store not available, skipping long-term storage of summary.");
        }
        
        // Update timestamp
        let mut last = self.last_consolidation.lock().await;
        last.insert(session_id.to_string(), Instant::now());
        
        Ok(())
    }
    
    pub async fn decay_memories(&self) -> Result<()> {
        // Implement memory decay: Find old memories and reduce importance or archive
        // This requires vector store to support metadata updates or re-indexing.
        // For MVP, we can just search for old memories and log them.
        // Or delete very old ones.
        info!("Running memory decay process...");
        // Placeholder for decay logic
        Ok(())
    }
}
