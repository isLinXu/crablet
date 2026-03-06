use anyhow::Result;
use tracing::{info, warn};
use std::sync::Arc;
use crate::config::Config;
use crate::cognitive::router::CognitiveRouter;
use crate::channels::cli::args::SkillSubcommands;
use crate::skills::SkillRegistry;

pub async fn handle_skill(subcmd: &SkillSubcommands, config: &Config, _router: &CognitiveRouter) -> Result<()> {
    match subcmd {
        SkillSubcommands::Install { name_or_url, name } => {
            let mut registry = SkillRegistry::new();
            
            // If it looks like a URL (starts with http/https/git), treat as direct install
            if name_or_url.starts_with("http") || name_or_url.starts_with("git@") {
                 let url = name_or_url;
                 // Check if it's a ClawHub URL
                if url.contains("clawhub.dev") {
                    return handle_clawhub_import(url, config).await;
                }
                
                let skills_dir = &config.skills_dir;
                info!("Installing skill from {} into {:?}", url, skills_dir);
                
                let repo_name = name.clone().unwrap_or_else(|| {
                    url.split('/').next_back().unwrap_or("unknown").trim_end_matches(".git").to_string()
                });
                
                let target_dir = skills_dir.join(&repo_name);
                if target_dir.exists() {
                    return Err(anyhow::anyhow!("Skill '{}' already exists at {:?}", repo_name, target_dir));
                }
                
                // Use git command
                let status = std::process::Command::new("git")
                    .arg("clone")
                    .arg(url)
                    .arg(&target_dir)
                    .status()
                    .map_err(|e| anyhow::anyhow!("Failed to execute git: {}", e))?;
                    
                if !status.success() {
                    return Err(anyhow::anyhow!("Git clone failed"));
                }
                
                println!("Skill '{}' installed successfully!", repo_name);
                
                // Verify OpenClaw compatibility
                if target_dir.join("SKILL.md").exists() {
                     println!("Detected OpenClaw skill format (SKILL.md found).");
                }
            } else {
                // Treat as registry name
                let name = name_or_url;
                info!("Searching registry for skill '{}'...", name);
                
                let target_dir = config.skills_dir.clone();
                registry.install(name, target_dir).await?;
                println!("Skill '{}' installed successfully from registry!", name);
            }
        }
        SkillSubcommands::Search { query } => {
            let registry = SkillRegistry::new();
            println!("Searching registry for '{}'...", query);
            match registry.search(query).await {
                Ok(results) => {
                    if results.is_empty() {
                        println!("No skills found matching '{}'.", query);
                    } else {
                        println!("Found {} skills:", results.len());
                        for skill in results {
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
        SkillSubcommands::Import { url } => {
            handle_clawhub_import(url, config).await?;
        }
        SkillSubcommands::Uninstall { name } => {
            let skills_dir = &config.skills_dir;
            let target_dir = skills_dir.join(name);
            
            if !target_dir.exists() {
                return Err(anyhow::anyhow!("Skill '{}' not found at {:?}", name, target_dir));
            }
            
            info!("Uninstalling skill '{}' from {:?}", name, target_dir);
            std::fs::remove_dir_all(&target_dir)?;
            println!("Skill '{}' uninstalled successfully.", name);
        }
        SkillSubcommands::List => {
            let mut registry = SkillRegistry::new();
            if let Err(e) = registry.load_from_dir(&config.skills_dir).await {
                 warn!("Failed to load skills: {}", e);
            }
            
            println!("Installed Skills:");
            for skill in registry.list_skills() {
                println!("- {} (v{}) - {}", skill.name, skill.version, skill.description);
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
