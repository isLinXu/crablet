use std::process::Command;
use anyhow::{Result, Context};
use crate::plugins::Plugin;
use crate::safety::oracle::{SafetyOracle, SafetyDecision};
use async_trait::async_trait;
use serde_json::Value;

pub struct BashPlugin {
    oracle: SafetyOracle,
}

impl BashPlugin {
    pub fn new(oracle: SafetyOracle) -> Self {
        Self { oracle }
    }
}

#[async_trait]
impl Plugin for BashPlugin {
    fn name(&self) -> &str {
        "run"
    }

    fn description(&self) -> &str {
        "Execute bash commands. Args: {\"cmd\": \"...\"}"
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let cmd = args.get("cmd")
            .and_then(|v| v.as_str())
            .context("Missing 'cmd' argument")?;
            
        // Safety Check
        match self.oracle.check_bash_command(cmd) {
            SafetyDecision::Allowed => BashTool::execute(cmd),
            SafetyDecision::Blocked(reason) => Ok(format!("🚫 Safety Oracle Blocked: {}", reason)),
            SafetyDecision::RequireConfirmation(reason) => {
                // For Plugins, we don't have an interactive confirmation loop yet.
                // We'll block it for now with a message.
                Ok(format!("⚠️ Confirmation Required: {}. Interactive confirmation not supported in plugin mode yet.", reason))
            }
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct BashTool;

impl BashTool {
    pub fn execute(cmd: &str) -> Result<String> {
        // MVP: Simple execution, no sandbox yet
        // TODO: Add timeout and sandbox (chroot/docker)
        let output = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .output()
            .context("Failed to execute command")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(stdout.to_string())
        } else {
            Ok(format!("Exit Code: {}\nError: {}\nOutput: {}", output.status, stderr, stdout))
        }
    }
}
