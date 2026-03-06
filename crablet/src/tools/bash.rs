use anyhow::{Result, Context, anyhow};
use crate::plugins::Plugin;
use crate::safety::oracle::{SafetyOracle, SafetyDecision};
use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use std::process::Stdio;

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
            
        // Safety Check 2: Static Analysis (Whitelist/Blacklist)
        if let Err(e) = BashTool::validate_command(cmd) {
             return Ok(format!("🚫 Security Policy Blocked: {}", e));
        }

        // Safety Check 1: Oracle
        match self.oracle.check_bash_command(cmd) {
            SafetyDecision::Allowed => BashTool::execute(cmd).await,
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
    fn validate_command(cmd: &str) -> Result<()> {
        let cmd = cmd.trim();
        if cmd.is_empty() { return Ok(()); }

        // STRICT WHITELIST APPROACH
        // We only allow specific commands and arguments that are known to be safe(r).
        // Complex commands with pipes, redirects, or subshells are BLOCKED by default unless explicitly handled.
        
        // 1. Check for dangerous characters that enable command injection or chaining
        let dangerous_chars = [';', '&', '|', '`', '$', '(', ')', '<', '>'];
        if cmd.chars().any(|c| dangerous_chars.contains(&c)) {
             return Err(anyhow!("Command contains dangerous characters/operators. Only simple commands are allowed."));
        }

        // 2. Binary Whitelist
        let allowed_binaries = [
            "ls", "grep", "cat", "echo", "pwd", "whoami", "date", 
            "git", "cargo", "mkdir", "touch", "rm", "cp", "mv", 
            "python", "python3", "node", "npm", "tree", "find", 
            "head", "tail", "wc", "sort", "uniq", "awk", "sed", "ps"
        ];
        
        let binary = cmd.split_whitespace().next().unwrap_or("");
        let binary_name = std::path::Path::new(binary)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(binary);
            
        if !allowed_binaries.contains(&binary_name) {
             return Err(anyhow!("Command binary '{}' is not in the whitelist", binary_name));
        }

        Ok(())
    }

    pub async fn execute(cmd: &str) -> Result<String> {
        // Security: Timeout and Output Size Limit
        let max_duration = Duration::from_secs(10);
        let max_output_size = 10 * 1024; // 10KB

        // Use parameterized execution if possible, but for raw bash commands we must use sh -c.
        // We rely on SafetyOracle for command validation before this point.
        // TODO: Implement command whitelist or restricted shell.
        
        let output_result = timeout(max_duration, 
            Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .output()
        ).await;

        let output = match output_result {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Err(anyhow!("Failed to execute command: {}", e)),
            Err(_) => return Err(anyhow!("Command execution timed out after {:?}", max_duration)),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if stdout.len() > max_output_size {
            return Err(anyhow!("Output size exceeds limit of {} bytes", max_output_size));
        }

        if output.status.success() {
            Ok(stdout.to_string())
        } else {
            Ok(format!("Exit Code: {}\nError: {}\nOutput: {}", output.status, stderr, stdout))
        }
    }
}
