use anyhow::{Result, Context, anyhow};
use crate::plugins::Plugin;
use crate::safety::oracle::{SafetyOracle, SafetyDecision};
use crate::sandbox::docker::{DockerExecutor};
use async_trait::async_trait;
use serde_json::Value;
use tracing::{info, warn};

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
        // Security: Use Docker sandbox instead of direct shell execution
        // This prevents command injection attacks by running in an isolated container
        let executor = DockerExecutor::strict()
            .with_timeout(10); // 10 second timeout
        
        info!("Executing bash command in Docker sandbox: {}", cmd);
        
        // Execute command in Docker sandbox
        let result = executor.execute("alpine:latest", &["sh", "-c", cmd]).await?;
        
        if result.success {
            Ok(result.stdout)
        } else {
            warn!("Bash command failed with exit code: {}", result.exit_code);
            Ok(format!("Exit Code: {}\nError: {}\nOutput: {}", 
                result.exit_code, result.stderr, result.stdout))
        }
    }
}
