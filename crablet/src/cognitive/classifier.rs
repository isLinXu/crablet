use anyhow::Result;
use crate::cognitive::llm::LlmClient;
use crate::types::Message;
use std::sync::Arc;
use tracing::info;

#[derive(Clone, Debug)]
pub enum Intent {
    ChitChat,
    Reasoning, // Requires Tools/Planning
    Research,  // Requires Deep Research
    Coding,    // Requires Code Execution (future)
    Unknown,
}

pub struct IntentClassifier {
    llm: Arc<Box<dyn LlmClient>>,
}

impl IntentClassifier {
    pub fn new(llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self { llm }
    }

    pub async fn classify(&self, input: &str) -> Result<Intent> {
        // Fast path for obvious keywords
        let lower = input.to_lowercase();
        if lower.starts_with("research ") || lower.contains("deep research") {
            return Ok(Intent::Research);
        }
        if lower == "hi" || lower == "hello" || lower == "help" {
            return Ok(Intent::ChitChat);
        }

        // Use LLM for classification
        // We use a very strict prompt to force a single word output
        let prompt = format!(
            "Classify the following user input into one of these categories: \
            [CHITCHAT, REASONING, RESEARCH, CODING]. \
            Output ONLY the category name. \
            \nInput: {}", 
            input
        );

        let response = self.llm.chat_complete(&[Message::new("user", &prompt)]).await?;
        let category = response.trim().to_uppercase();
        
        // Remove any surrounding punctuation if LLM is chatty
        let category = category.trim_matches(|c: char| !c.is_alphabetic());

        info!("Intent Classified: {} -> {}", input, category);

        match category {
            "CHITCHAT" => Ok(Intent::ChitChat),
            "REASONING" => Ok(Intent::Reasoning),
            "RESEARCH" => Ok(Intent::Research),
            "CODING" => Ok(Intent::Coding),
            _ => Ok(Intent::Reasoning), // Default fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cognitive::llm::LlmClient;
    use crate::types::Message;
    use async_trait::async_trait;
    use std::sync::Arc;
    use anyhow::Result;

    struct MockClassifierLlm {
        response: String,
    }

    #[async_trait]
    impl LlmClient for MockClassifierLlm {
        async fn chat_complete(&self, _messages: &[Message]) -> Result<String> {
            Ok(self.response.clone())
        }
        async fn chat_complete_with_tools(&self, _messages: &[Message], _tools: &[serde_json::Value]) -> Result<Message> {
            Ok(Message::new("assistant", &self.response))
        }
    }

    #[tokio::test]
    async fn test_classify_chitchat() {
        let llm = Arc::new(Box::new(MockClassifierLlm { response: "CHITCHAT".to_string() }) as Box<dyn LlmClient>);
        let classifier = IntentClassifier::new(llm);
        // "Hello" is caught by fast path
        let intent = classifier.classify("Hello").await.unwrap();
        assert!(matches!(intent, Intent::ChitChat));
    }

    #[tokio::test]
    async fn test_classify_research() {
        let llm = Arc::new(Box::new(MockClassifierLlm { response: "RESEARCH".to_string() }) as Box<dyn LlmClient>);
        let classifier = IntentClassifier::new(llm);
        // "research ..." is caught by fast path
        let intent = classifier.classify("research quantum computing").await.unwrap();
        assert!(matches!(intent, Intent::Research));
    }

    #[tokio::test]
    async fn test_classify_coding() {
        let llm = Arc::new(Box::new(MockClassifierLlm { response: "CODING".to_string() }) as Box<dyn LlmClient>);
        let classifier = IntentClassifier::new(llm);
        let intent = classifier.classify("write a python script").await.unwrap();
        assert!(matches!(intent, Intent::Coding));
    }
}
