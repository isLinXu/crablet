use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, oneshot};
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use anyhow::{Result, anyhow};
use tracing::{info, warn, debug};
use std::time::Duration;

// --- JSON-RPC Types ---

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonRpcError {
    code: i64,
    message: String,
    data: Option<serde_json::Value>,
}

// --- MCP Types ---

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpPrompt {
    pub name: String,
    pub description: Option<String>,
    pub arguments: Option<Vec<McpPromptArgument>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpPromptArgument {
    pub name: String,
    pub description: Option<String>,
    pub required: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug)]
struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    protocol_version: String,
    capabilities: serde_json::Value,
    #[serde(rename = "clientInfo")]
    client_info: ClientInfo,
}

#[derive(Serialize, Deserialize, Debug)]
struct ClientInfo {
    name: String,
    version: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct CallToolParams {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug)]
struct CallToolResult {
    content: Vec<ContentItem>,
    #[serde(rename = "isError")]
    is_error: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ContentItem {
    r#type: String,
    text: String,
}

// --- MCP Client ---

pub struct McpClient {
    writer: Arc<Mutex<tokio::process::ChildStdin>>,
    pending: Arc<DashMap<u64, oneshot::Sender<JsonRpcResponse>>>,
    next_id: Arc<AtomicU64>,
    _reader_task: tokio::task::JoinHandle<()>,
    _child: Arc<Mutex<Child>>,
}

impl McpClient {
    pub async fn new(command: &str, args: &[String]) -> Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn MCP server '{}': {}", command, e))?;

        let stdin = child.stdin.take().ok_or_else(|| anyhow!("Failed to open stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("Failed to open stdout"))?;
        
        let pending: Arc<DashMap<u64, oneshot::Sender<JsonRpcResponse>>> = Arc::new(DashMap::new());
        let pending_clone = pending.clone();
        
        // Background reader task
        let reader_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            
            while let Ok(Some(line)) = lines.next_line().await {
                debug!("MCP Client received: {}", line);
                if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(&line) {
                    if let Some(id) = response.id {
                        if let Some((_, sender)) = pending_clone.remove(&id) {
                            let _ = sender.send(response);
                        } else {
                            // ID mismatch or timed out
                            warn!("Received response for unknown ID: {}", id);
                        }
                    } else {
                        // Notification or error without ID
                        if response.error.is_some() {
                             warn!("MCP Notification Error: {:?}", response.error);
                        }
                    }
                } else {
                    warn!("MCP Client received invalid JSON: {}", line);
                }
            }
            debug!("MCP Server stdout closed (Normal shutdown)");
        });

        let client = Self {
            writer: Arc::new(Mutex::new(stdin)),
            pending,
            next_id: Arc::new(AtomicU64::new(1)),
            _reader_task: reader_task,
            _child: Arc::new(Mutex::new(child)),
        };

        client.initialize().await?;
        Ok(client)
    }

    async fn send_request(&self, method: &str, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        
        self.pending.insert(id, tx);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let json_str = serde_json::to_string(&request)?;
        debug!("MCP Client sending request: {}", json_str);
        
        {
            let mut writer = self.writer.lock().await;
            writer.write_all(json_str.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        }

        // Wait for response with timeout
        let response = tokio::time::timeout(Duration::from_secs(60), rx)
            .await
            .map_err(|_| {
                self.pending.remove(&id); // Cleanup on timeout
                anyhow!("MCP Request '{}' timed out (ID: {})", method, id)
            })?
            .map_err(|_| anyhow!("MCP Response channel closed (ID: {})", id))?;
            
        if let Some(error) = response.error {
            return Err(anyhow!("MCP Error {}: {}", error.code, error.message));
        }
        
        Ok(response.result.unwrap_or(serde_json::Value::Null))
    }

    async fn initialize(&self) -> Result<()> {
        let params = InitializeParams {
            protocol_version: "2024-11-05".to_string(),
            capabilities: serde_json::json!({ "roots": { "listChanged": false } }),
            client_info: ClientInfo {
                name: "Crablet".to_string(),
                version: "0.1.0".to_string(),
            },
        };

        let _res = self.send_request("initialize", Some(serde_json::to_value(params)?)).await?;
        
        // Send initialized notification (no response expected)
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        
        {
            let mut writer = self.writer.lock().await;
            writer.write_all(serde_json::to_string(&notification)?.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        }
        
        info!("MCP Client initialized");
        Ok(())
    }

    pub async fn list_tools(&self) -> Result<Vec<McpTool>> {
        let res = self.send_request("tools/list", None).await?;
        
        #[derive(Deserialize)]
        struct ListToolsResult {
            tools: Vec<McpTool>,
        }
        
        let list: ListToolsResult = serde_json::from_value(res)?;
        Ok(list.tools)
    }

    pub async fn list_resources(&self) -> Result<Vec<McpResource>> {
        let res = self.send_request("resources/list", None).await?;
        
        #[derive(Deserialize)]
        struct ListResourcesResult {
            resources: Vec<McpResource>,
        }
        
        let list: ListResourcesResult = serde_json::from_value(res)?;
        Ok(list.resources)
    }

    pub async fn read_resource(&self, uri: &str) -> Result<String> {
        let params = serde_json::json!({ "uri": uri });
        let res = self.send_request("resources/read", Some(params)).await?;
        
        #[derive(Deserialize)]
        struct ReadResourceResult {
            contents: Vec<ResourceContent>,
        }
        
        #[derive(Deserialize)]
        struct ResourceContent {
            text: Option<String>,
            blob: Option<String>,
        }
        
        let result: ReadResourceResult = serde_json::from_value(res)?;
        
        let mut content = String::new();
        for item in result.contents {
            if let Some(text) = item.text {
                content.push_str(&text);
            } else if let Some(blob) = item.blob {
                content.push_str(&format!("[Blob: {}]", blob));
            }
        }
        
        Ok(content)
    }

    pub async fn list_prompts(&self) -> Result<Vec<McpPrompt>> {
        let res = self.send_request("prompts/list", None).await?;
        
        #[derive(Deserialize)]
        struct ListPromptsResult {
            prompts: Vec<McpPrompt>,
        }
        
        let list: ListPromptsResult = serde_json::from_value(res)?;
        Ok(list.prompts)
    }

    pub async fn get_prompt(&self, name: &str, arguments: Option<serde_json::Value>) -> Result<String> {
        let params = serde_json::json!({ 
            "name": name,
            "arguments": arguments
        });
        
        let res = self.send_request("prompts/get", Some(params)).await?;
        
        #[derive(Deserialize)]
        struct GetPromptResult {
            messages: Vec<PromptMessage>,
        }
        
        #[derive(Deserialize)]
        struct PromptMessage {
            role: String,
            content: PromptContent,
        }

        #[derive(Deserialize)]
        struct PromptContent {
            #[allow(dead_code)]
             r#type: String,
             text: String,
        }
        
        let result: GetPromptResult = serde_json::from_value(res)?;
        
        let mut full_prompt = String::new();
        for msg in result.messages {
            full_prompt.push_str(&format!("{}: {}\n\n", msg.role, msg.content.text));
        }
        
        Ok(full_prompt)
    }

    pub async fn call_tool(&self, name: &str, arguments: serde_json::Value) -> Result<String> {
        let params = CallToolParams {
            name: name.to_string(),
            arguments,
        };

        let res = self.send_request("tools/call", Some(serde_json::to_value(params)?)).await?;
        let result: CallToolResult = serde_json::from_value(res)?;

        if result.is_error.unwrap_or(false) {
             let error_msg = result.content.iter()
                .map(|c| c.text.clone())
                .collect::<Vec<_>>()
                .join("\n");
             return Err(anyhow!("Tool execution error: {}", error_msg));
        }

        let output = result.content.iter()
            .map(|c| c.text.clone())
            .collect::<Vec<_>>()
            .join("\n");
            
        Ok(output)
    }
}
