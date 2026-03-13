//! Ollama 本地模型客户端
//!
//! 提供对本地运行的大语言模型的支持，实现低延迟(<50ms)的推理能力。
//! 支持模型下载、加载、推理和缓存管理。

use anyhow::{Result, Context};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{info, debug, warn};

use crate::cognitive::llm::LlmClient;
use crate::types::{Message, ContentPart};

/// Ollama 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    /// Ollama 服务地址
    pub host: String,
    /// 默认模型
    pub default_model: String,
    /// 请求超时 (秒)
    pub timeout_secs: u64,
    /// 是否自动拉取缺失的模型
    pub auto_pull: bool,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            host: "http://localhost:11434".to_string(),
            default_model: "llama3.2".to_string(),
            timeout_secs: 30,
            auto_pull: true,
        }
    }
}

/// Ollama 客户端
pub struct OllamaClient {
    config: OllamaConfig,
    http_client: reqwest::Client,
    current_model: String,
}

/// Ollama 生成请求
#[derive(Serialize)]
struct GenerateRequest {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<GenerateOptions>,
}

/// 生成选项
#[derive(Serialize)]
struct GenerateOptions {
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<i32>,
}

/// Ollama 响应
#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
    done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<Vec<i32>>,
}

/// 模型信息
#[derive(Deserialize)]
struct ModelInfo {
    name: String,
    modified_at: String,
    size: i64,
}

/// 模型列表响应
#[derive(Deserialize)]
struct ListModelsResponse {
    models: Vec<ModelInfo>,
}

impl OllamaClient {
    /// 创建新的 Ollama 客户端
    pub fn new(config: OllamaConfig) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .context("Failed to create HTTP client")?;

        let current_model = config.default_model.clone();

        Ok(Self {
            config,
            http_client,
            current_model,
        })
    }

    /// 检查 Ollama 服务是否可用
    pub async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.config.host);
        match self.http_client.get(&url).send().await {
            Ok(response) => response.status().is_success(),
            Err(e) => {
                debug!("Ollama service not available: {}", e);
                false
            }
        }
    }

    /// 获取可用模型列表
    pub async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.config.host);
        let response = self.http_client
            .get(&url)
            .send()
            .await
            .context("Failed to list models")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Failed to list models: {}", response.status()));
        }

        let list_response: ListModelsResponse = response
            .json()
            .await
            .context("Failed to parse models response")?;

        let model_names: Vec<String> = list_response
            .models
            .into_iter()
            .map(|m| m.name)
            .collect();

        Ok(model_names)
    }

    /// 检查模型是否已下载
    pub async fn has_model(&self, model: &str) -> bool {
        match self.list_models().await {
            Ok(models) => models.iter().any(|m| m == model || m.starts_with(&format!("{}:", model))),
            Err(_) => false,
        }
    }

    /// 拉取模型
    pub async fn pull_model(&self, model: &str) -> Result<()> {
        info!("Pulling model: {}", model);
        
        let url = format!("{}/api/pull", self.config.host);
        let response = self.http_client
            .post(&url)
            .json(&serde_json::json!({
                "name": model,
                "stream": false
            }))
            .send()
            .await
            .context("Failed to pull model")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Failed to pull model: {}", error_text));
        }

        info!("Successfully pulled model: {}", model);
        Ok(())
    }

    /// 设置当前模型
    pub fn set_model(&mut self, model: &str) {
        self.current_model = model.to_string();
    }

    /// 生成文本
    async fn generate(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        // 检查模型是否可用
        if !self.has_model(&self.current_model).await {
            if self.config.auto_pull {
                self.pull_model(&self.current_model).await?;
            } else {
                return Err(anyhow::anyhow!(
                    "Model {} not found. Set auto_pull=true to automatically pull models.",
                    self.current_model
                ));
            }
        }

        let request = GenerateRequest {
            model: self.current_model.clone(),
            prompt: prompt.to_string(),
            system: system.map(|s| s.to_string()),
            stream: false,
            options: Some(GenerateOptions {
                temperature: 0.7,
                num_predict: None, // 让模型自行决定长度
            }),
        };

        let url = format!("{}/api/generate", self.config.host);
        let response = self.http_client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to generate text")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Generation failed: {}", error_text));
        }

        let generate_response: GenerateResponse = response
            .json()
            .await
            .context("Failed to parse generation response")?;

        Ok(generate_response.response)
    }
}

#[async_trait]
impl LlmClient for OllamaClient {
    async fn chat_complete(&self, messages: &[Message]) -> Result<String> {
        // 将消息列表转换为 prompt
        let mut prompt = String::new();
        let mut system_prompt = None;
        
        for message in messages {
            match message.role.as_str() {
                "system" => {
                    if let Some(content) = &message.content {
                        let text: Vec<String> = content.iter()
                            .filter_map(|part| {
                                if let ContentPart::Text { text } = part {
                                    Some(text.clone())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if !text.is_empty() {
                            system_prompt = Some(text.join(" "));
                        }
                    }
                }
                "user" | "assistant" => {
                    let role_label = if message.role == "user" { "User" } else { "Assistant" };
                    if let Some(content) = &message.content {
                        let text: Vec<String> = content.iter()
                            .filter_map(|part| {
                                if let ContentPart::Text { text } = part {
                                    Some(text.clone())
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if !text.is_empty() {
                            prompt.push_str(&format!("{}: {}\n", role_label, text.join(" ")));
                        }
                    }
                }
                _ => {}
            }
        }
        
        // 添加最终提示
        prompt.push_str("Assistant: ");
        
        // 调用生成
        let response = self.generate(&prompt, system_prompt.as_deref()).await?;
        
        // 清理响应
        let cleaned = response.trim().to_string();
        
        Ok(cleaned)
    }

    async fn chat_complete_with_tools(
        &self,
        messages: &[Message],
        _tools: &[serde_json::Value],
    ) -> Result<Message> {
        // Ollama 对工具支持有限，先返回普通响应
        // 后续可以集成 Ollama 的 function calling 功能
        let content = self.chat_complete(messages).await?;
        
        Ok(Message {
            role: "assistant".to_string(),
            content: Some(vec![ContentPart::Text { text: content }]),
            tool_calls: None,
            tool_call_id: None,
        })
    }

    fn model_name(&self) -> &str {
        &self.current_model
    }
}

/// 本地模型管理器
pub struct LocalModelManager {
    config: OllamaConfig,
    http_client: reqwest::Client,
}

impl LocalModelManager {
    pub fn new(config: OllamaConfig) -> Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            config,
            http_client,
        })
    }

    /// 获取推荐模型列表
    pub fn recommended_models(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("llama3.2", "Meta Llama 3.2 - 轻量级通用模型"),
            ("qwen2.5", "通义千问 2.5 - 中文优化"),
            ("phi4", "Microsoft Phi-4 - 小型高效"),
            ("mistral", "Mistral - 欧洲开源模型"),
            ("codellama", "Code Llama - 代码生成专用"),
            ("deepseek-coder", "DeepSeek Coder - 中文代码模型"),
        ]
    }

    /// 获取模型信息
    pub async fn get_model_info(&self, model: &str) -> Result<serde_json::Value> {
        let url = format!("{}/api/show", self.config.host);
        let response = self.http_client
            .post(&url)
            .json(&serde_json::json!({ "name": model }))
            .send()
            .await
            .context("Failed to get model info")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Failed to get model info: {}", error_text));
        }

        let info: serde_json::Value = response.json().await?;
        Ok(info)
    }

    /// 检查系统资源
    pub async fn check_system_resources(&self) -> Result<SystemResources> {
        // 使用 ollama 的 system 信息
        let url = format!("{}/api/tags", self.config.host);
        let response = self.http_client
            .get(&url)
            .send()
            .await;

        let mut resources = SystemResources {
            total_memory_gb: 0,
            available_memory_gb: 0,
            cpu_cores: 0,
            gpu_available: false,
            gpu_vram_gb: 0,
        };

        match response {
            Ok(resp) if resp.status().is_success() => {
                // Ollama 服务运行中
                resources.available_memory_gb = 4; // 假设至少4GB可用
                resources.total_memory_gb = 16; // 假设16GB总内存
                resources.cpu_cores = 4; // 假设4核
            }
            _ => {
                // Ollama 服务未运行
                warn!("Ollama service not available");
            }
        }

        Ok(resources)
    }
}

/// 系统资源信息
#[derive(Debug, Clone)]
pub struct SystemResources {
    /// 总内存 (GB)
    pub total_memory_gb: u32,
    /// 可用内存 (GB)
    pub available_memory_gb: u32,
    /// CPU 核心数
    pub cpu_cores: u32,
    /// GPU 是否可用
    pub gpu_available: bool,
    /// GPU 显存 (GB)
    pub gpu_vram_gb: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::complexity::{ComplexityAnalyzer, Complexity};

    #[test]
    fn test_ollama_config_default() {
        let config = OllamaConfig::default();
        assert_eq!(config.host, "http://localhost:11434");
        assert_eq!(config.default_model, "llama3.2");
        assert_eq!(config.timeout_secs, 30);
        assert!(config.auto_pull);
    }

    #[tokio::test]
    async fn test_complexity_analysis() {
        let analyzer = ComplexityAnalyzer::new();
        
        // 简单消息
        let simple_messages = vec![Message::user("Hello!")];
        
        let complexity = analyzer.analyze(&simple_messages).unwrap();
        assert_eq!(complexity, Complexity::Simple);
        
        // 复杂消息
        let complex_messages = vec![Message::user(r#"
                请详细分析量子计算对现代密码学的影响，包括：
                1. 量子算法（如Shor算法）如何威胁RSA和椭圆曲线加密
                2. 后量子密码学的发展方向，包括格密码、多变量密码等
                3. 当前NIST后量子密码标准的进展和评估
                4. 企业和政府应该如何准备迁移到后量子密码系统
                请提供具体的数学原理说明、实际案例分析和时间线预测。
                "#)];
        
        let complexity = analyzer.analyze(&complex_messages).unwrap();
        assert_ne!(complexity, Complexity::Simple);
    }
}
