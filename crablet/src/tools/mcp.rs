use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use anyhow::{Result, anyhow};
use tracing::info;

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
    _child: Mutex<Child>,
    stdin: Mutex<tokio::process::ChildStdin>,
    stdout: Mutex<BufReader<tokio::process::ChildStdout>>,
    request_id: Mutex<u64>,
}

impl McpClient {
    pub async fn new(command: &str, args: &[String]) -> Result<Self> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // Log stderr to parent's stderr
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn MCP server '{}': {}", command, e))?;

        let stdin = child.stdin.take().ok_or_else(|| anyhow!("Failed to open stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("Failed to open stdout"))?;
        let reader = BufReader::new(stdout);

        let client = Self {
            _child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(reader),
            request_id: Mutex::new(0),
        };

        client.initialize().await?;
        Ok(client)
    }

    async fn send_request(&self, method: &str, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let id = {
            let mut lock = self.request_id.lock().await;
            *lock += 1;
            *lock
        };

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let json_str = serde_json::to_string(&request)?;
        
        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(json_str.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
        }

        // Read response
        // Note: This is a simplified implementation that assumes synchronous request-response over stdio.
        // A robust implementation would need a background reader task and a map of pending requests.
        // For MVP, we assume the server responds to requests in order or we just read lines until we find the matching ID.
        
        let mut stdout = self.stdout.lock().await;
        let mut line = String::new();
        
        loop {
            line.clear();
            if stdout.read_line(&mut line).await? == 0 {
                return Err(anyhow!("MCP Server closed connection"));
            }

            // Parse line
            if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(&line) {
                if response.id == Some(id) {
                    if let Some(error) = response.error {
                        return Err(anyhow!("MCP Error {}: {}", error.code, error.message));
                    }
                    return Ok(response.result.unwrap_or(serde_json::Value::Null));
                }
                // Ignore notifications or other responses for now
            } else {
                // Maybe log debug?
                // warn!("MCP Client received invalid JSON: {}", line);
            }
        }
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
        
        // Send initialized notification
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        
        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(serde_json::to_string(&notification)?.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
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
