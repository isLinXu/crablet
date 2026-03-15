use anyhow::Result;
use tracing::{info, warn};
use std::sync::Arc;
use crate::config::Config;
use crate::cognitive::router::CognitiveRouter;
use crate::channels::cli::args::{SkillSubcommands, SkillDevSubcommands};
use crate::skills::{
    SkillRegistry, AtomicInstaller, SkillSignatureVerifier, VerificationResult,
    VersionManager, DevTools,
};

pub async fn handle_skill(subcmd: &SkillSubcommands, config: &Config, _router: &CognitiveRouter) -> Result<()> {
    match subcmd {
        SkillSubcommands::Install { name_or_url, name, interactive, skip_verify, force: _, isolated: _ } => {
            // 交互式模式
            if *interactive {
                println!("Starting interactive installation wizard...");
                // TODO: 初始化 SkillSearchManager 并启动向导
                println!("Interactive mode not yet fully implemented.");
                return Ok(());
            }

            // If it looks like a URL (starts with http/https/git), treat as direct install
            if name_or_url.starts_with("http") || name_or_url.starts_with("git@") {
                let url = name_or_url;
                
                // Check if it's a ClawHub URL
                if url.contains("clawhub.dev") {
                    return handle_clawhub_import(url, config).await;
                }
                
                let skills_dir = &config.skills_dir;
                info!("Installing skill from {} into {:?}", url, skills_dir);
                
                // 使用原子性安装器
                let repo_name = name.clone().unwrap_or_else(|| {
                    url.split('/').next_back().unwrap_or("unknown").trim_end_matches(".git").to_string()
                });
                
                println!("🔧 Installing skill '{}' from {}...", repo_name, url);
                
                match AtomicInstaller::install_from_git(url, skills_dir, name.as_deref()).await {
                    Ok(result) => {
                        println!("✅ Skill '{}' (v{}) installed successfully!", 
                            result.skill_name, 
                            result.version
                        );
                        println!("📁 Location: {:?}", result.install_path);
                        
                        // 验证签名
                        if !skip_verify {
                            let verifier = SkillSignatureVerifier::new();
                            match verifier.verify(&result.install_path).await {
                                VerificationResult::Trusted { fingerprint, signer, .. } => {
                                    println!("🔒 Verified: Signed by {} ({})", signer, fingerprint);
                                }
                                VerificationResult::Unsigned => {
                                    println!("⚠️  Warning: Skill is not signed");
                                }
                                VerificationResult::Untrusted { fingerprint } => {
                                    println!("⚠️  Warning: Unknown signer ({})", fingerprint);
                                }
                                VerificationResult::Invalid { reason } => {
                                    println!("❌ Signature invalid: {}", reason);
                                }
                                _ => {}
                            }
                        }
                        
                        // 显示使用帮助
                        println!("\n📝 Usage:");
                        println!("  crablet skill test {} '{{\"arg\": \"value\"}}'", result.skill_name);
                    }
                    Err(e) => {
                        eprintln!("❌ Installation failed: {}", e);
                        return Err(e);
                    }
                }
            } else {
                // Treat as registry name
                let skill_name = name_or_url;
                info!("Searching registry for skill '{}'...", skill_name);
                
                let mut registry = SkillRegistry::new();
                let target_dir = config.skills_dir.clone();
                registry.install(skill_name, target_dir).await?;
                println!("Skill '{}' installed successfully from registry!", skill_name);
            }
        }
        SkillSubcommands::Search { query, category, limit, semantic } => {
            if *semantic {
                println!("Performing semantic search for: '{}'", query);
                // TODO: 使用 SkillSearchManager 进行语义搜索
            } else {
                let registry = SkillRegistry::new();
                println!("Searching registry for '{}'...", query);
                match registry.search(query).await {
                    Ok(results) => {
                        let filtered: Vec<_> = if let Some(ref cat) = category {
                            results.into_iter()
                                .filter(|s| s.description.to_lowercase().contains(&cat.to_lowercase()))
                                .take(*limit)
                                .collect()
                        } else {
                            results.into_iter().take(*limit).collect()
                        };
                        
                        if filtered.is_empty() {
                            println!("No skills found matching '{}'.", query);
                        } else {
                            println!("Found {} skills:", filtered.len());
                            for skill in filtered {
                                println!("- {} (v{}): {}", skill.name, skill.version, skill.description);
                                if let Some(author) = skill.author {
                                    println!("  Author: {}", author);
                                }
                                if let Some(rating) = skill.rating {
                                    println!("  Rating: {:.1}/5.0", rating);
                                }
                                println!("  URL: {}", skill.url);
                                println!();
                            }
                        }
                    }
                    Err(e) => {
                        println!("Failed to search registry: {}", e);
                    }
                }
            }
        }
        SkillSubcommands::Import { url } => {
            handle_clawhub_import(url, config).await?;
        }
        SkillSubcommands::Uninstall { name, force } => {
            let skills_dir = &config.skills_dir;
            let target_dir = skills_dir.join(name);
            
            if !target_dir.exists() {
                return Err(anyhow::anyhow!("Skill '{}' not found at {:?}", name, target_dir));
            }
            
            if !force {
                print!("Are you sure you want to uninstall '{}'? [y/N] ", name);
                std::io::Write::flush(&mut std::io::stdout())?;
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Uninstall cancelled.");
                    return Ok(());
                }
            }
            
            info!("Uninstalling skill '{}' from {:?}", name, target_dir);
            std::fs::remove_dir_all(&target_dir)?;
            println!("Skill '{}' uninstalled successfully.", name);
        }
        SkillSubcommands::List { detailed, updates } => {
            let mut registry = SkillRegistry::new();
            if let Err(e) = registry.load_from_dir(&config.skills_dir).await {
                 warn!("Failed to load skills: {}", e);
            }
            
            if *updates {
                println!("Checking for updates...");
                // TODO: 使用 VersionManager 检查更新
            }
            
            println!("Installed Skills:");
            for skill in registry.list_skills() {
                if *detailed {
                    println!("\n📦 {} (v{})", skill.name, skill.version);
                    println!("   {}", skill.description);
                    println!("   Path: {:?}", config.skills_dir.join(&skill.name));
                } else {
                    println!("- {} (v{}) - {}", skill.name, skill.version, skill.description);
                }
            }
        }
        SkillSubcommands::Test { name, args } => {
            let parsed_args: serde_json::Value = serde_json::from_str(args)
                .map_err(|e| anyhow::anyhow!("Invalid JSON arguments: {}", e))?;
            
            let mut registry = SkillRegistry::new();
            if let Err(e) = registry.load_from_dir(&config.skills_dir).await {
                 warn!("Failed to load skills: {}", e);
            }
            
            // Initialize MCP tools for testing if configured
            for (server_name, server_config) in &config.mcp_servers {
                info!("Initializing MCP server for test: {}", server_name);
                match crate::tools::mcp::McpClient::new(&server_config.command, &server_config.args).await {
                    Ok(client) => {
                        let client_arc = Arc::new(client);
                        match client_arc.list_tools().await {
                            Ok(tools) => {
                                for tool in tools {
                                    info!("Registering MCP tool: {}", tool.name);
                                    registry.register_mcp_tool(tool.name, client_arc.clone(), tool.description.clone(), tool.input_schema);
                                }
                            }
                            Err(e) => warn!("Failed to list tools from MCP server {}: {}", server_name, e),
                        }
                    }
                    Err(e) => warn!("Failed to connect to MCP server {}: {}", server_name, e),
                }
            }
            
            info!("Testing skill '{}' with args: {}", name, args);
            match registry.execute(name, parsed_args).await {
                Ok(output) => println!("Output:\n{}", output),
                Err(e) => println!("Error: {}", e),
            }
        }
        SkillSubcommands::Update { name, list, all } => {
            let mut version_manager = VersionManager::new(config.skills_dir.clone());
            version_manager.load().await?;
            
            if *list {
                let updates = version_manager.check_updates().await?;
                if updates.is_empty() {
                    println!("All skills are up to date!");
                } else {
                    println!("Available updates:");
                    for update in updates {
                        println!("  {}: {} -> {} ({:?})",
                            update.skill_name,
                            update.current_version,
                            update.latest_version,
                            update.diff
                        );
                    }
                }
            } else if let Some(skill_name) = name {
                println!("Updating skill: {}", skill_name);
                // TODO: 执行更新
            } else if *all {
                println!("Updating all skills...");
                // TODO: 批量更新
            }
        }
        SkillSubcommands::Info { name, docs } => {
            let skill_path = config.skills_dir.join(name);
            if !skill_path.exists() {
                return Err(anyhow::anyhow!("Skill '{}' not found", name));
            }
            
            let skill_md = skill_path.join("SKILL.md");
            if skill_md.exists() {
                let content = std::fs::read_to_string(&skill_md)?;
                if *docs {
                    println!("{}", content);
                } else {
                    // 显示摘要信息
                    println!("📦 {}", name);
                    // TODO: 解析并显示关键信息
                }
            }
        }
        SkillSubcommands::Wizard => {
            println!("Starting interactive skill installation wizard...");
            // TODO: 启动 InteractiveWizard
            println!("Wizard mode not yet fully implemented.");
        }
        SkillSubcommands::Dev { subcmd } => {
            handle_dev_subcommand(subcmd, config).await?;
        }
    }
    Ok(())
}

async fn handle_dev_subcommand(subcmd: &SkillDevSubcommands, _config: &Config) -> Result<()> {
    match subcmd {
        SkillDevSubcommands::Init { name, skill_type, path } => {
            let project_path = match path.clone() {
                Some(p) => p,
                None => std::env::current_dir()?.join(name),
            };
            
            let skill_type_enum = match skill_type.as_str() {
                "openclaw" => crate::skills::SkillType::OpenClaw(
                    crate::skills::Skill {
                        manifest: crate::skills::SkillManifest {
                            name: name.clone(),
                            description: String::new(),
                            version: "0.1.0".to_string(),
                            parameters: serde_json::json!({}),
                            entrypoint: String::new(),
                            env: std::collections::HashMap::new(),
                            requires: vec![],
                            runtime: None,
                            dependencies: None,
                            resources: None,
                            permissions: vec![],
                            conflicts: vec![],
                            min_crablet_version: None,
                            author: None,
                            triggers: vec![],
                        },
                        path: project_path.clone(),
                    },
                    String::new()
                ),
                "local" => crate::skills::SkillType::Local(crate::skills::Skill {
                    manifest: crate::skills::SkillManifest {
                        name: name.clone(),
                        description: String::new(),
                        version: "0.1.0".to_string(),
                        parameters: serde_json::json!({}),
                        entrypoint: String::new(),
                        env: std::collections::HashMap::new(),
                        requires: vec![],
                        runtime: None,
                        dependencies: None,
                        resources: None,
                        permissions: vec![],
                        conflicts: vec![],
                        min_crablet_version: None,
                        author: None,
                        triggers: vec![],
                    },
                    path: project_path.clone(),
                }),
                _ => {
                    return Err(anyhow::anyhow!("Unknown skill type: {}", skill_type));
                }
            };
            
            let result = DevTools::init(name, skill_type_enum, Some(project_path)).await?;
            
            println!("✅ Skill project '{}' initialized!", result.name);
            println!("\nNext steps:");
            for step in result.next_steps {
                println!("  {}", step);
            }
        }
        SkillDevSubcommands::Validate { path } => {
            let project_path = path.clone().unwrap_or_else(|| std::env::current_dir().unwrap());
            
            println!("🔍 Validating skill project at: {:?}", project_path);
            
            let result = DevTools::validate(&project_path).await?;
            
            if result.valid {
                println!("✅ Validation passed!");
            } else {
                println!("❌ Validation failed with {} errors:", result.errors.len());
                for error in &result.errors {
                    println!("  - {}", error);
                }
            }
            
            if !result.warnings.is_empty() {
                println!("\n⚠️  Warnings:");
                for warning in &result.warnings {
                    println!("  - {}", warning);
                }
            }
            
            if !result.suggestions.is_empty() {
                println!("\n💡 Suggestions:");
                for suggestion in &result.suggestions {
                    println!("  - {}", suggestion);
                }
            }
        }
        SkillDevSubcommands::Test { path, args } => {
            let project_path = path.clone().unwrap_or_else(|| std::env::current_dir().unwrap());
            
            println!("🧪 Running tests...");
            
            let result = DevTools::test(&project_path, args.as_deref()).await?;
            
            println!("\nTest Results:");
            println!("  Passed:  {}", result.passed);
            println!("  Failed:  {}", result.failed);
            println!("  Skipped: {}", result.skipped);
            println!("  Time:    {}ms", result.duration_ms);
        }
        SkillDevSubcommands::Build { path, output } => {
            let project_path = path.clone().unwrap_or_else(|| std::env::current_dir().unwrap());
            
            println!("🔨 Building skill package...");
            
            let result = DevTools::build(&project_path, Some(output.clone())).await?;
            
            println!("✅ Build complete!");
            println!("  Package: {:?}", result.package_path);
            println!("  Size:    {} bytes", result.size_bytes);
            println!("  SHA256:  {}", result.checksum);
        }
        SkillDevSubcommands::Publish { path, registry, dry_run } => {
            let project_path = path.clone().unwrap_or_else(|| std::env::current_dir().unwrap());
            
            if *dry_run {
                println!("🔍 Dry run - validating only...");
            } else {
                println!("🚀 Publishing skill...");
            }
            
            let result = DevTools::publish(&project_path, registry.clone(), *dry_run).await?;
            
            if result.success {
                println!("✅ {}", result.message);
                if let Some(url) = result.url {
                    println!("  URL: {}", url);
                }
            } else {
                println!("❌ Publish failed");
            }
        }
        SkillDevSubcommands::Docs { path, output } => {
            let project_path = path.clone().unwrap_or_else(|| std::env::current_dir().unwrap());
            
            println!("📚 Generating documentation...");
            
            let result = DevTools::docs(&project_path, Some(output.clone())).await?;
            
            println!("✅ Documentation generated!");
            println!("  Location: {:?}", result.output_dir);
            println!("  Files:    {}", result.files_generated.len());
        }
    }
    Ok(())
}

async fn handle_clawhub_import(url: &str, config: &Config) -> Result<()> {
    // 1. Fetch page to find Git URL
    // ClawHub pages usually have a link to the GitHub repo.
    // For MVP, we can try to extract user/repo from URL if it follows a pattern, or fetch HTML.
    // Assuming simple pattern for now or just treat as git url if it ends with .git
    
    // If it's a direct git URL or local path, just use it.
    let git_url = if url.ends_with(".git") || url.starts_with("/") {
        url.to_string()
    } else {
        // Mocking ClawHub resolution logic for MVP
        // In real impl, we would fetch the page and look for <meta name="go-import"> or similar
        // or look for "View on GitHub" link.
        println!("Resolving ClawHub URL: {}", url);
        // Fallback: assume the user provided a raw git url or we prompt them
        // For now, let's assume the user knows what they are doing and passed a git-compatible url
        // or we fail gracefully.
        
        if url.contains("github.com") {
            url.to_string()
        } else {
            return Err(anyhow::anyhow!("Could not resolve Git URL from ClawHub link. Please provide the Git repository URL directly."));
        }
    };
    
    let skills_dir = &config.skills_dir;
    let repo_name = git_url.split('/').next_back().unwrap_or("unknown").trim_end_matches(".git").to_string();
    let target_dir = skills_dir.join(&repo_name);
    
    if target_dir.exists() {
        return Err(anyhow::anyhow!("Skill '{}' already exists at {:?}", repo_name, target_dir));
    }
    
    info!("Cloning {} into {:?}", git_url, target_dir);
    let status = std::process::Command::new("git")
        .arg("clone")
        .arg(&git_url)
        .arg(&target_dir)
        .status()
        .map_err(|e| anyhow::anyhow!("Failed to execute git: {}", e))?;
        
    if !status.success() {
        return Err(anyhow::anyhow!("Git clone failed"));
    }
    
    println!("Skill '{}' imported from ClawHub successfully!", repo_name);
    if target_dir.join("SKILL.md").exists() {
         println!("Valid OpenClaw skill confirmed.");
    }
    
    Ok(())
}
