use super::{Sandbox, Language, ExecutionResult};
use async_trait::async_trait;
use anyhow::Result;
use std::time::Instant;
use tokio::process::Command;
use tracing::warn;

pub struct LocalSandbox;

#[async_trait]
impl Sandbox for LocalSandbox {
    async fn init(&self) -> Result<()> {
        warn!("INITIALIZING LOCAL SANDBOX: This environment is NOT isolated and poses security risks.");
        Ok(())
    }

    async fn execute(&self, language: Language, code: &str) -> Result<ExecutionResult> {
        warn!("EXECUTING IN LOCAL SANDBOX: No isolation for language {:?}. Code: {}", language, code);
        // DANGER: This is not sandboxed! Only for dev/test or when Docker is unavailable.
        let (program, args) = match language {
            Language::Python => ("python3", vec!["-c", code]),
            Language::JavaScript => ("node", vec!["-e", code]),
            Language::Shell => ("sh", vec!["-c", code]),
            Language::Lua => ("lua", vec!["-e", code]),
        };
        
        let start = Instant::now();
        let output_result = Command::new(program)
            .args(&args) // Borrow args
            .output()
            .await;

        let duration = start.elapsed();

        match output_result {
            Ok(output) => {
                Ok(ExecutionResult {
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    exit_code: output.status.code().unwrap_or(-1),
                    duration_ms: duration.as_millis() as u64,
                })
            },
            Err(e) => {
                // Return error as result but format it
                Ok(ExecutionResult {
                    stdout: "".to_string(),
                    stderr: format!("Execution failed: {}", e),
                    exit_code: -1,
                    duration_ms: duration.as_millis() as u64,
                })
            }
        }
    }

    async fn cleanup(&self) -> Result<()> {
        Ok(())
    }
}
