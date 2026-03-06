use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;
use tracing::{info, error};
use serde::{Serialize, Deserialize};
use crate::agent::swarm::{SwarmAgent, SwarmMessage, AgentId};
use crate::cognitive::llm::LlmClient;
use crate::types::Message;
use tokio::process::Command;
use tokio::fs;

#[derive(Clone)]
pub struct DataAnalystAgent {
    id: AgentId,
    llm: Arc<Box<dyn LlmClient>>,
    work_dir: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnalysisRequest {
    file_path: String,
    goal: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnalysisResult {
    summary: String,
    code_executed: String,
    output: String,
}

impl DataAnalystAgent {
    pub fn new(llm: Arc<Box<dyn LlmClient>>, work_dir: PathBuf) -> Self {
        Self {
            id: AgentId::new(),
            llm,
            work_dir,
        }
    }

    async fn generate_python_code(&self, file_path: &str, goal: &str, head: &str) -> Option<String> {
        let prompt = format!(
            "You are a Data Analyst. Write a Python script to analyze the file '{}'.\n\
            Goal: {}\n\
            \n\
            File Preview (first 5 lines):\n\
            {}\n\
            \n\
            Requirements:\n\
            1. Use pandas to read the file.\n\
            2. Print the analysis results to stdout.\n\
            3. Do not generate plots/images, only text output.\n\
            4. Handle potential errors gracefully.\n\
            5. Output ONLY the python code within ```python ... ``` blocks.\n",
            file_path, goal, head
        );

        let messages = vec![Message::user(prompt)];
        match self.llm.chat_complete(&messages).await {
            Ok(response) => {
                // Extract code from markdown blocks
                let re = regex::Regex::new(r"```python\s*([\s\S]*?)\s*```").ok()?;
                if let Some(caps) = re.captures(&response) {
                    Some(caps[1].to_string())
                } else {
                    // Fallback: assume the whole response is code if no blocks
                    Some(response) 
                }
            }
            Err(e) => {
                error!("LLM error generating analysis code: {}", e);
                None
            }
        }
    }

    async fn execute_python(&self, code: &str) -> Result<String, String> {
        let script_path = self.work_dir.join(format!("analysis_{}.py", uuid::Uuid::new_v4()));
        
        if let Err(e) = fs::write(&script_path, code).await {
            return Err(format!("Failed to write script: {}", e));
        }

        let output = Command::new("python3")
            .arg(&script_path)
            .output()
            .await;

        // Clean up
        let _ = fs::remove_file(&script_path).await;

        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                if out.status.success() {
                    Ok(stdout)
                } else {
                    Err(format!("Script failed:\nSTDOUT: {}\nSTDERR: {}", stdout, stderr))
                }
            }
            Err(e) => Err(format!("Failed to execute python3: {}", e)),
        }
    }
}

#[async_trait]
impl SwarmAgent for DataAnalystAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }

    fn name(&self) -> &str {
        "DataAnalystAgent"
    }

    fn description(&self) -> &str {
        "Specialized agent for analyzing data files (CSV/JSON) using Python."
    }

    fn capabilities(&self) -> Vec<String> {
        vec!["data_analysis".to_string(), "python".to_string()]
    }

    async fn receive(&mut self, message: SwarmMessage, _sender: AgentId) -> Option<SwarmMessage> {
        match message {
            SwarmMessage::Task { task_id, description, payload, .. } => {
                info!("DataAnalystAgent received task: {}", description);
                
                // Parse payload or extract from description
                let (file_path, goal) = if let Some(p) = payload {
                    let path = p.get("file_path").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let goal = p.get("goal").and_then(|v| v.as_str()).map(|s| s.to_string());
                    (path, goal.unwrap_or(description.clone()))
                } else {
                    // Naive parsing: assume description is "Analyze <file>"
                    let parts: Vec<&str> = description.split_whitespace().collect();
                    if parts.len() >= 2 {
                        (Some(parts.last().unwrap().to_string()), description.clone())
                    } else {
                        (None, description.clone())
                    }
                };

                let file_path = match file_path {
                    Some(p) => p,
                    None => return Some(SwarmMessage::Error {
                        task_id,
                        error: "No file path provided in payload or description".to_string(),
                    })
                };

                // Check file existence
                let path = Path::new(&file_path);
                if !path.exists() {
                     return Some(SwarmMessage::Error {
                        task_id,
                        error: format!("File not found: {}", file_path),
                    });
                }

                // Read head
                let head = match fs::read_to_string(&path).await {
                    Ok(c) => c.lines().take(5).collect::<Vec<_>>().join("\n"),
                    Err(e) => return Some(SwarmMessage::Error {
                        task_id,
                        error: format!("Failed to read file: {}", e),
                    })
                };

                // Generate Code
                let code = match self.generate_python_code(&file_path, &goal, &head).await {
                    Some(c) => c,
                    None => return Some(SwarmMessage::Error {
                        task_id,
                        error: "Failed to generate analysis code".to_string(),
                    })
                };

                // Execute Code
                let (output, success) = match self.execute_python(&code).await {
                    Ok(out) => (out, true),
                    Err(err) => (err, false),
                };

                let result = AnalysisResult {
                    summary: if success { "Analysis completed successfully.".to_string() } else { "Analysis failed.".to_string() },
                    code_executed: code,
                    output: output.clone(),
                };

                Some(SwarmMessage::Result {
                    task_id,
                    content: output,
                    payload: Some(serde_json::to_value(&result).unwrap_or_default()),
                })
            }
            _ => None,
        }
    }
}
