use anyhow::{Result, Context};
use crate::cognitive::llm::{LlmClient, OpenAiClient};
use crate::types::Message;
use std::sync::Arc;
use std::env;
use serde::{Deserialize, Serialize};

pub struct KnowledgeExtractor {
    llm: Arc<Box<dyn LlmClient>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExtractedEntity {
    pub name: String,
    pub r#type: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExtractedRelation {
    pub source: String,
    pub target: String,
    pub relation: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ExtractionResult {
    pub entities: Vec<ExtractedEntity>,
    pub relations: Vec<ExtractedRelation>,
}

impl KnowledgeExtractor {
    pub fn new() -> Result<Self> {
        let model = env::var("OPENAI_MODEL_NAME").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let client = OpenAiClient::new(&model)?;
        Ok(Self {
            llm: Arc::new(Box::new(client)),
        })
    }

    pub async fn extract_from_text(&self, text: &str) -> Result<ExtractionResult> {
        let prompt = format!(
            r#"Extract entities and relations from the following text.
            Return ONLY a JSON object with this structure:
            {{
                "entities": [{{ "name": "Entity Name", "type": "Entity Type" }}],
                "relations": [{{ "source": "Entity Name", "target": "Entity Name", "relation": "Relation Name" }}]
            }}
            
            Text:
            {}
            "#,
            text
        );

        let message = Message::new("user", &prompt);
        let response = self.llm.chat_complete(&[message]).await?;
        
        // Clean up response (remove markdown code blocks if present)
        let cleaned_response = response.trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        let result: ExtractionResult = serde_json::from_str(cleaned_response)
            .context("Failed to parse LLM extraction result")?;
            
        Ok(result)
    }
}
