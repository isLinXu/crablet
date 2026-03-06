use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;
use tracing::{info, warn, error};
use walkdir::WalkDir;
use serde::{Serialize, Deserialize};
use crate::agent::swarm::{SwarmAgent, SwarmMessage, AgentId};
use crate::cognitive::llm::LlmClient;
use crate::types::Message;

#[derive(Clone)]
pub struct SecurityAuditAgent {
    id: AgentId,
    llm: Arc<Box<dyn LlmClient>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Vulnerability {
    file: String,
    line: Option<usize>,
    severity: String, // Critical, High, Medium, Low
    description: String,
    suggestion: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AuditReport {
    summary: String,
    vulnerabilities: Vec<Vulnerability>,
}

impl SecurityAuditAgent {
    pub fn new(llm: Arc<Box<dyn LlmClient>>) -> Self {
        Self {
            id: AgentId::new(),
            llm,
        }
    }

    async fn analyze_file(&self, path: &Path) -> Option<Vec<Vulnerability>> {
        let extension = path.extension()?.to_str()?;
        if !["rs", "py", "js", "ts", "go", "java", "c", "cpp"].contains(&extension) {
            return None;
        }

        let content = match tokio::fs::read_to_string(path).await {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to read file {:?}: {}", path, e);
                return None;
            }
        };

        if content.len() > 50_000 {
            warn!("Skipping large file {:?} ({} bytes)", path, content.len());
            return None;
        }

        let prompt = format!(
            "You are a Security Audit Expert. Analyze the following code for security vulnerabilities.\n\
            File: {:?}\n\
            \n\
            Code:\n\
            ```{}\n\
            {}\n\
            ```\n\
            \n\
            Output ONLY a JSON array of objects with fields: 'file' (string, use {:?}), 'line' (number, optional), 'severity' (string: Critical/High/Medium/Low), 'description' (string), 'suggestion' (string). \
            If no vulnerabilities are found, output an empty array []. \
            Do not output markdown formatting like ```json ... ```, just the raw JSON string.",
            path, extension, content, path
        );

        let messages = vec![Message::user(prompt)];
        match self.llm.chat_complete(&messages).await {
            Ok(response) => {
                let cleaned = response.trim().trim_start_matches("```json").trim_start_matches("```").trim_end_matches("```");
                match serde_json::from_str::<Vec<Vulnerability>>(cleaned) {
                    Ok(vulns) => Some(vulns),
                    Err(e) => {
                        warn!("Failed to parse LLM response for {:?}: {}", path, e);
                        // Fallback: try to recover or just log raw response
                        None
                    }
                }
            }
            Err(e) => {
                error!("LLM error analyzing {:?}: {}", path, e);
                None
            }
        }
    }
}

#[async_trait]
impl SwarmAgent for SecurityAuditAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }

    fn name(&self) -> &str {
        "SecurityAuditAgent"
    }

    fn description(&self) -> &str {
        "Specialized agent for auditing codebases for security vulnerabilities."
    }

    fn capabilities(&self) -> Vec<String> {
        vec!["security_audit".to_string(), "code_analysis".to_string()]
    }

    async fn receive(&mut self, message: SwarmMessage, _sender: AgentId) -> Option<SwarmMessage> {
        match message {
            SwarmMessage::Task { task_id, description, payload, .. } => {
                info!("SecurityAuditAgent received task: {}", description);
                
                // Extract path from payload or description
                let path_str = if let Some(p) = payload {
                    p.get("path").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or(description.clone())
                } else {
                    description.clone()
                };
                
                let path = PathBuf::from(&path_str);
                if !path.exists() {
                    return Some(SwarmMessage::Error {
                        task_id,
                        error: format!("Path not found: {:?}", path),
                    });
                }

                let mut all_vulns = Vec::new();
                let walker = WalkDir::new(&path).into_iter();

                for entry in walker.filter_map(|e| e.ok()) {
                    if entry.file_type().is_file() {
                        if let Some(vulns) = self.analyze_file(entry.path()).await {
                            all_vulns.extend(vulns);
                        }
                    }
                }

                let summary = format!("Found {} vulnerabilities in {:?}", all_vulns.len(), path);
                let report = AuditReport {
                    summary: summary.clone(),
                    vulnerabilities: all_vulns,
                };

                let json_report = serde_json::to_value(&report).unwrap_or_default();
                
                Some(SwarmMessage::Result {
                    task_id,
                    content: summary,
                    payload: Some(json_report),
                })
            }
            _ => None,
        }
    }
}
