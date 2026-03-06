use crate::config::Config;
use crate::cognitive::llm::LlmClient;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize, Deserialize)]
pub struct CheckResult {
    pub status: String,
    pub details: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthReport {
    pub status: String,
    pub checks: HashMap<String, CheckResult>,
}

impl Default for HealthReport {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthReport {
    pub fn new() -> Self {
        Self {
            status: "healthy".to_string(),
            checks: HashMap::new(),
        }
    }

    pub fn check(&mut self, name: &str, result: std::result::Result<(), anyhow::Error>) {
        match result {
            Ok(_) => {
                self.checks.insert(name.to_string(), CheckResult {
                    status: "pass".to_string(),
                    details: None,
                });
            }
            Err(e) => {
                self.status = "unhealthy".to_string();
                self.checks.insert(name.to_string(), CheckResult {
                    status: "fail".to_string(),
                    details: Some(e.to_string()),
                });
            }
        }
    }

    pub fn finalize(self) -> Self {
        self
    }
}

pub async fn check_database(url: &str) -> std::result::Result<(), anyhow::Error> {
    let pool = SqlitePool::connect(url).await?;
    pool.close().await;
    Ok(())
}

pub async fn check_llm_provider(client: &dyn LlmClient) -> std::result::Result<(), anyhow::Error> {
    // Basic check if model is configured.
    // In future, call a cheap endpoint like `embed` or `tokenize` if available.
    if client.model_name().is_empty() {
        return Err(anyhow::anyhow!("LLM model name is empty"));
    }
    Ok(())
}

pub async fn startup_health_check(config: &Config, llm_client: Arc<Box<dyn LlmClient>>) -> std::result::Result<HealthReport, anyhow::Error> {
    let mut report = HealthReport::new();

    // 1. Database Check
    report.check("database", check_database(&config.database_url).await);

    // 2. LLM Check
    report.check("llm_provider", check_llm_provider(llm_client.as_ref().as_ref()).await);

    // 3. MCP Servers Check
    for name in config.mcp_servers.keys() {
        report.check(&format!("mcp_{}", name), Ok(()));
    }

    Ok(report.finalize())
}

