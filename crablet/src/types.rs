use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "deserialize_content")]
    pub content: Option<Vec<ContentPart>>, 
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    // Add tool_call_id for role="tool" messages
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(default = "default_tool_type")]
    pub r#type: String,
    pub function: FunctionCall,
}

fn default_tool_type() -> String {
    "function".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    #[serde(deserialize_with = "deserialize_arguments")]
    pub arguments: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatChunk {
    pub delta: String,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStep {
    pub step: usize,
    pub thought: String,
    pub action: Option<String>,      // e.g. "search"
    pub action_input: Option<String>, // e.g. "{\"query\": \"rust\"}"
    pub observation: Option<String>,  // Tool execution result
}

impl Message {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: Some(vec![ContentPart::Text { text: content.into() }]),
            tool_calls: None,
            tool_call_id: None,
        }
    }
    
    pub fn new_tool_response(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(vec![ContentPart::Text { text: content.into() }]),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(vec![ContentPart::Text { text: content.into() }]),
            tool_calls: None,
            tool_call_id: None,
        }
    }
    
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(vec![ContentPart::Text { text: content.into() }]),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn text(&self) -> Option<String> {
        self.content.as_ref().map(|parts| {
            parts.iter().map(|p| match p {
                ContentPart::Text { text } => text.clone(),
                _ => "".to_string(),
            }).collect::<Vec<_>>().join("")
        })
    }
}

impl TraceStep {
    pub fn cache_hit(score: f32) -> Self {
        Self {
            step: 0,
            thought: format!("Semantic Cache Hit (score: {:.2})", score),
            action: None,
            action_input: None,
            observation: Some("Returned cached response".to_string()),
        }
    }
}

// Custom deserializer to handle both string and array content
fn deserialize_content<'de, D>(deserializer: D) -> Result<Option<Vec<ContentPart>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Content {
        String(String),
        Parts(Vec<ContentPart>),
    }

    let content = Option::<Content>::deserialize(deserializer)?;
    
    match content {
        Some(Content::String(s)) => Ok(Some(vec![ContentPart::Text { text: s }])),
        Some(Content::Parts(parts)) => Ok(Some(parts)),
        None => Ok(None),
    }
}

fn deserialize_arguments<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Arguments {
        String(String),
        Object(serde_json::Value),
    }

    match Arguments::deserialize(deserializer)? {
        Arguments::String(s) => Ok(s),
        Arguments::Object(v) => Ok(v.to_string()),
    }
}
