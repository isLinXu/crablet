use tracing::warn;

#[derive(Clone, Debug)]
pub enum SafetyLevel {
    Strict,     // Block all potentially dangerous commands
    Permissive, // Allow with warning
    Disabled,   // Allow everything
}

#[derive(Debug, PartialEq)]
pub enum SafetyDecision {
    Allowed,
    Blocked(String),
    RequireConfirmation(String),
}

#[derive(Clone)]
pub struct SafetyOracle {
    level: SafetyLevel,
    allowed_commands: Vec<String>,
}

impl SafetyOracle {
    pub fn new(level: SafetyLevel) -> Self {
        Self {
            level,
            allowed_commands: vec![
                "ls".to_string(), 
                "cat".to_string(), 
                "echo".to_string(), 
                "grep".to_string(),
                "pwd".to_string(),
                "whoami".to_string(),
                "date".to_string(),
            ],
        }
    }

    pub fn check_bash_command(&self, cmd: &str) -> SafetyDecision {
        match self.level {
            SafetyLevel::Disabled => SafetyDecision::Allowed,
            SafetyLevel::Permissive => {
                warn!("Executing potentially dangerous command: {}", cmd);
                SafetyDecision::Allowed
            }
            SafetyLevel::Strict => {
                // Use shlex to split
                let tokens = match shlex::split(cmd) {
                    Some(t) => t,
                    None => return SafetyDecision::Blocked("Failed to parse command syntax".to_string()),
                };
                
                if tokens.is_empty() {
                    return SafetyDecision::Allowed;
                }

                // Identify command verbs. 
                // Heuristic: First token, and any token following a control operator.
                let control_ops = vec!["|", "||", "&", "&&", ";", "(", ")", "`"];
                
                let mut is_command_pos = true;
                for token in &tokens {
                    if control_ops.contains(&token.as_str()) {
                        is_command_pos = true;
                        continue;
                    }
                    
                    if is_command_pos {
                        // This token is likely a command executable
                        if !self.allowed_commands.contains(token) {
                             // Check if it is a dangerous one explicitly for better error message
                             let dangerous = vec!["rm", "mv", "dd", "chmod", "chown", "sudo", "ssh", "curl", "wget", "sh", "bash", "python", "perl"];
                             if dangerous.contains(&token.as_str()) {
                                 return SafetyDecision::RequireConfirmation(format!("Command '{}' contains dangerous program '{}'.", cmd, token));
                             }
                             return SafetyDecision::Blocked(format!("Program '{}' is not in the allowed whitelist.", token));
                        }
                        is_command_pos = false; // Next args are arguments
                    }
                }
                
                SafetyDecision::Allowed
            }
        }
    }
    
    pub fn check_file_access(&self, path: &str) -> SafetyDecision {
         match self.level {
            SafetyLevel::Disabled => SafetyDecision::Allowed,
            _ => {
                if path.contains("..") {
                     return SafetyDecision::Blocked(format!("Path traversal detected: {}", path));
                }
                SafetyDecision::Allowed
            }
        }
    }
}
