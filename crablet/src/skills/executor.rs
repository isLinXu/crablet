use anyhow::{Result, Context};
use std::process::Stdio;
use tokio::process::Command;
use tracing::info;
use tokio::time::Duration;
use super::{SkillType, SkillRegistry};

pub struct SkillExecutor;

impl SkillExecutor {
    pub async fn execute(registry: &SkillRegistry, name: &str, args: serde_json::Value) -> Result<String> {
        let skill_type = registry.skills.get(name).context(format!("Skill not found: {}", name))?;
        
        let manifest = match skill_type {
            SkillType::Local(s) => &s.manifest,
            SkillType::Mcp(m, _, _) => m,
            SkillType::Plugin(m, _) => m,
            SkillType::OpenClaw(s, _) => &s.manifest,
        };

        // Timeout for execution
        let timeout_duration = if let Some(res) = &manifest.resources {
             if let Some(t) = &res.timeout {
                 let val: u64 = t.chars().take_while(|c| c.is_numeric()).collect::<String>().parse().unwrap_or(30);
                 if t.ends_with('m') {
                     Duration::from_secs(val * 60)
                 } else {
                     Duration::from_secs(val)
                 }
             } else {
                 Duration::from_secs(30)
             }
        } else {
             Duration::from_secs(30)
        };
        
        let skill_type = skill_type.clone();

        let execution_future = async move {
            match skill_type {
                SkillType::Local(skill) => {
                    // Prepare command
                    let parts: Vec<&str> = skill.manifest.entrypoint.split_whitespace().collect();
                    if parts.is_empty() {
                        return Err(anyhow::anyhow!("Invalid entrypoint for skill {}", name));
                    }

                    let cmd = parts[0];
                    let cmd_args = &parts[1..];

                    let mut command = Command::new(cmd);
                    command.args(cmd_args);
                    command.current_dir(&skill.path);
                    
                    let args_json = serde_json::to_string(&args)?;
                    command.arg(&args_json);

                    for (k, v) in &skill.manifest.env {
                        command.env(k, v);
                    }

                    command.stdout(Stdio::piped());
                    command.stderr(Stdio::piped());

                    info!("Executing skill {}: {} {}", name, skill.manifest.entrypoint, args_json);

                    let output = command.spawn()?.wait_with_output().await?;

                    if output.status.success() {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        Ok(stdout)
                    } else {
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        Err(anyhow::anyhow!("Skill execution failed: {}", stderr))
                    }
                },
                SkillType::Mcp(_, client, tool_name) => {
                    info!("Executing MCP tool {}: {}", tool_name, args);
                    client.call_tool(tool_name.as_str(), args).await
                },
                SkillType::Plugin(_, plugin) => {
                    info!("Executing Plugin {}: {}", name, args);
                    plugin.execute(name, args).await
                },
                SkillType::OpenClaw(_skill, instruction) => {
                    info!("Executing OpenClaw skill: {}", name);
                    
                    // Simple interpolation: Replace {{arg}} with value
                    let mut prompt = instruction.clone();
                    
                    // Check if instruction contains python code block ```python
                    if prompt.contains("```python") {
                        // Extract python code
                        if let Some(start) = prompt.find("```python") {
                            // Wait, end needs to be after start + len
                            let code_start = start + 9; // len("```python")
                            let code_block = &prompt[code_start..];
                            if let Some(_code_end) = code_block.find("```") {
                                // No special handling for 'see' here anymore.
                                // If the skill exists, it executes as a prompt skill.
                                // If we want to disable 'see', we must ensure it's not loaded into the registry.
                            }
                        }
                    }
                    
                    if let Some(obj) = args.as_object() {
                        for (k, v) in obj {
                            let key = format!("{{{{{}}}}}", k); // {{key}}
                            let val = v.as_str().unwrap_or(&v.to_string()).to_string();
                            prompt = prompt.replace(&key, &val);
                        }
                    }
                    
                    // Let's just return a generic success message if it's not a prompt skill
                    if prompt.len() > 500 {
                        Ok(format!("Executed skill '{}'. (Output suppressed as it seems to be documentation)", name))
                    } else {
                         Ok(format!("### INSTRUCTION FROM SKILL\n{}", prompt))
                    }
                }
            }
        };

        match tokio::time::timeout(timeout_duration, execution_future).await {
            Ok(result) => result,
            Err(_) => Err(anyhow::anyhow!("Skill execution timed out after {:?}", timeout_duration)),
        }
    }
}
