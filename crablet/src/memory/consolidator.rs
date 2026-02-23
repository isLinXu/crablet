use anyhow::Result;
use crate::cognitive::llm::LlmClient;
use crate::memory::episodic::EpisodicMemory;
use crate::knowledge::vector_store::VectorStore;
use crate::types::Message;
use std::sync::Arc;
use tracing::info;

pub struct MemoryConsolidator {
    llm: Arc<Box<dyn LlmClient>>,
    vector_store: Arc<VectorStore>,
}

impl MemoryConsolidator {
    pub fn new(llm: Arc<Box<dyn LlmClient>>, vector_store: Arc<VectorStore>) -> Self {
        Self { llm, vector_store }
    }

    pub async fn consolidate(&self, memory: &EpisodicMemory, session_id: &str) -> Result<()> {
        info!("Consolidating memory for session: {}", session_id);
        
        // 1. Fetch recent messages
        let messages = memory.get_context(session_id, 20).await?;
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

        let summary = self.llm.chat_complete(&summary_msgs).await?;
        info!("Generated summary: {}", summary);

        // 3. Store in Vector Store (Long-term Memory)
        // We treat the summary as a "document"
        self.vector_store.add_document(&summary, Some(serde_json::json!({
            "type": "conversation_summary",
            "session_id": session_id,
            "timestamp": chrono::Utc::now().to_rfc3339()
        }))).await?;

        info!("Memory consolidated into Vector Store.");
        Ok(())
    }
}
