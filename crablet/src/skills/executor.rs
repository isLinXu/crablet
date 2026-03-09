use anyhow::{Result, Context, anyhow};
use tracing::{info, debug};
use tokio::time::Duration;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use chrono::{DateTime, Utc};
use super::{SkillType, SkillRegistry};
use crate::sandbox::docker::DockerExecutor;
use crate::sandbox::local::LocalSandbox;
use crate::sandbox::{Sandbox, Language};

/// 安全等级定义
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafetyLevel {
    /// L1 - 信任: 系统内置技能、MCP，无限制执行
    Trust,
    /// L2 - 隔离: 用户自定义技能，Docker 沙箱隔离
    Isolated,
    /// L3 - 强隔离: 第三方不可信技能，Wasm 沙箱隔离
    StronglyIsolated,
}

/// 执行沙箱类型
pub enum ExecutionSandbox {
    Docker(DockerExecutor),
    Native(LocalSandbox),
    Wasm, // Placeholder for future implementation
}

/// 执行审计日志
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionAudit {
    pub skill_name: String,
    pub skill_type: String,
    pub safety_level: SafetyLevel,
    pub args: Value,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub success: bool,
    pub exit_code: i32,
    pub duration_ms: u64,
}

pub struct SkillExecutor;

impl SkillExecutor {
    pub async fn execute(registry: &SkillRegistry, name: &str, args: Value) -> Result<String> {
        let skill_type = registry.skills.get(name).context(format!("Skill not found: {}", name))?;
        
        let manifest = match skill_type {
            SkillType::Local(s) => &s.manifest,
            SkillType::Mcp(m, _, _) => m,
            SkillType::Plugin(m, _) => m,
            SkillType::OpenClaw(s, _) => &s.manifest,
        };

        // 1. 确定安全等级和沙箱
        let safety_level = Self::determine_safety_level(skill_type);
        
        // 2. 准备执行上下文 (超时等)
        let timeout_duration = Self::get_timeout(manifest);
        
        let start_time = Utc::now();
        info!("Executing skill {} (Safety: {:?}, Timeout: {:?})", name, safety_level, timeout_duration);

        // 3. 执行
        let result = match skill_type {
            SkillType::Local(skill) => {
                Self::execute_local(skill, safety_level, args.clone(), timeout_duration).await
            },
            SkillType::Mcp(_, client, tool_name) => {
                client.call_tool(tool_name.as_str(), args.clone()).await
            },
            SkillType::Plugin(_, plugin) => {
                plugin.execute(name, args.clone()).await
            },
            SkillType::OpenClaw(_skill, instruction) => {
                Self::execute_openclaw(name, instruction, args.clone()).await
            }
        };

        // 4. 审计日志 (目前仅打印，后续可扩展为持久化存储)
        let end_time = Utc::now();
        let duration = end_time.signed_duration_since(start_time).num_milliseconds() as u64;
        
        let audit = ExecutionAudit {
            skill_name: name.to_string(),
            skill_type: match skill_type {
                SkillType::Local(_) => "Local".to_string(),
                SkillType::Mcp(_, _, _) => "MCP".to_string(),
                SkillType::Plugin(_, _) => "Plugin".to_string(),
                SkillType::OpenClaw(_, _) => "OpenClaw".to_string(),
            },
            safety_level,
            args,
            start_time,
            end_time: Some(end_time),
            success: result.is_ok(),
            exit_code: if result.is_ok() { 0 } else { -1 },
            duration_ms: duration,
        };
        
        debug!("Execution Audit: {:?}", audit);

        result
    }

    fn determine_safety_level(skill: &SkillType) -> SafetyLevel {
        match skill {
            SkillType::Local(s) => {
                // Check if it's a known trusted builtin skill
                let trusted_builtins = ["help", "status", "version"];
                if trusted_builtins.contains(&s.manifest.name.as_str()) && s.manifest.permissions.is_empty() {
                    SafetyLevel::Trust
                } else {
                    // All other local skills must be isolated
                    SafetyLevel::Isolated
                }
            },
            SkillType::Mcp(_, _, _) => SafetyLevel::Trust, // MCP has its own transport-level isolation
            SkillType::Plugin(_, _) => SafetyLevel::Trust, // Compiled-in plugins are trusted
            SkillType::OpenClaw(_, _) => SafetyLevel::Trust, // Prompt-only, no code execution risk
        }
    }

    fn get_timeout(manifest: &super::SkillManifest) -> Duration {
        if let Some(res) = &manifest.resources {
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
        }
    }

    async fn execute_local(
        skill: &super::Skill, 
        safety: SafetyLevel, 
        args: Value, 
        timeout: Duration
    ) -> Result<String> {
        match safety {
            SafetyLevel::Trust => {
                // 使用 Native 执行 (LocalSandbox)
                let sandbox = LocalSandbox;
                let lang = match skill.manifest.runtime.as_deref() {
                    Some("python3") | Some("python") => Language::Python,
                    Some("node") | Some("javascript") => Language::JavaScript,
                    Some("lua") => Language::Lua,
                    _ => Language::Shell,
                };
                
                // 构建执行代码 (这里简单拼接 entrypoint 和参数)
                let args_str = serde_json::to_string(&args)?;
                let code = format!("{} '{}'", skill.manifest.entrypoint, args_str);
                
                let res = sandbox.execute(lang, &code).await?;
                if res.exit_code == 0 {
                    Ok(res.stdout)
                } else {
                    Err(anyhow!("Execution failed (exit {}): {}", res.exit_code, res.stderr))
                }
            },
            SafetyLevel::Isolated => {
                // 使用 Docker 执行
                let executor = DockerExecutor::strict()
                    .with_work_dir(skill.path.to_string_lossy().to_string())
                    .with_timeout(timeout.as_secs());
                
                let parts: Vec<&str> = skill.manifest.entrypoint.split_whitespace().collect();
                if parts.is_empty() {
                    return Err(anyhow!("Invalid entrypoint"));
                }

                let mut full_cmd = parts;
                let args_json = serde_json::to_string(&args)?;
                full_cmd.push(&args_json);

                let result = executor.execute("alpine:latest", &full_cmd).await?;
                if result.success {
                    Ok(result.stdout)
                } else {
                    Err(anyhow!("Docker execution failed ({}): {}", result.exit_code, result.stderr))
                }
            },
            SafetyLevel::StronglyIsolated => {
                // TODO: 实现 Wasm 执行器
                Err(anyhow!("Wasm isolation not yet implemented"))
            }
        }
    }

    async fn execute_openclaw(name: &str, instruction: &str, args: Value) -> Result<String> {
        info!("Executing OpenClaw skill: {}", name);
        let mut prompt = instruction.to_string();
        
        // 参数插值
        if let Some(obj) = args.as_object() {
            for (k, v) in obj {
                let key = format!("{{{{{}}}}}", k);
                let val = v.as_str().unwrap_or(&v.to_string()).to_string();
                prompt = prompt.replace(&key, &val);
            }
        }
        
        if prompt.len() > 1000 {
            Ok(format!("Executed skill '{}'. (Output truncated)", name))
        } else {
            Ok(format!("### SKILL INSTRUCTION\n{}", prompt))
        }
    }
}
