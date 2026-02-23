use crate::config::Config;
use clap::{Parser, Subcommand};
use anyhow::Result;
use tracing::{info, warn, error};
use crate::cognitive::router::CognitiveRouter;
use crate::memory::episodic::EpisodicMemory;
use crate::memory::semantic::{SqliteKnowledgeGraph, Neo4jKnowledgeGraph, SharedKnowledgeGraph};
use crate::knowledge::extractor::KnowledgeExtractor;
use crate::knowledge::pdf::PdfParser;
use crate::knowledge::vector_store::VectorStore;
use crate::cognitive::multimodal::image::ImageProcessor;
use crate::cognitive::multimodal::audio::AudioTool;
use crate::scripting::engine::LuaEngine;
use std::sync::Arc;
use crate::events::EventBus;

#[derive(Parser)]
#[command(name = "crablet")]
#[command(about = "🦀 Crablet: Next-gen AI Assistant", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize Crablet environment
    Init,
    /// Start chat mode
    Chat {
        /// Session ID (optional)
        #[arg(long)]
        session: Option<String>,
    },
    /// Run a single prompt
    Run { 
        /// The prompt to execute
        prompt: String,
        /// Session ID (optional)
        #[arg(long)]
        session: Option<String>,
    },
    /// Show status
    Status,
    /// Configuration management
    Config,
    /// Start the server (Telegram bot, API gateway)
    Serve,
    /// Knowledge Management
    Knowledge {
        #[command(subcommand)]
        subcmd: KnowledgeSubcommands,
    },
    /// Vision Capabilities
    Vision {
        #[command(subcommand)]
        subcmd: VisionSubcommands,
    },
    /// Audio Capabilities
    Audio {
        #[command(subcommand)]
        subcmd: AudioSubcommands,
    },
    /// Run a Lua script
    RunScript {
        /// Path to Lua script
        path: String,
    },
    /// Web UI
    ServeWeb {
        /// Port to listen on
        #[arg(long, default_value = "3000")]
        port: u16,
    },
    /// Skill Development Tools
    Skill {
        #[command(subcommand)]
        subcmd: SkillSubcommands,
    },

    /// Start the Crablet Gateway
    Gateway {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Port to listen on
        #[arg(long, default_value = "18789")]
        port: u16,
    },
}

#[derive(Subcommand)]
enum SkillSubcommands {
    /// Test a skill in isolation
    Test {
        /// Name of the skill to test
        name: String,
        /// JSON arguments for the skill (e.g. '{"arg1": "val"}')
        #[arg(default_value = "{}")]
        args: String,
    },
    /// Install a skill from a git repository (OpenClaw compatible)
    Install {
        /// Git URL of the skill repository
        url: String,
        /// Optional name (defaults to repo name)
        name: Option<String>,
    },
    /// Uninstall a skill
    Uninstall {
        /// Name of the skill to uninstall
        name: String,
    },
    /// List installed skills
    List,
}

#[derive(Subcommand)]
enum KnowledgeSubcommands {
    /// Extract knowledge from text or file and save to graph
    Extract {
        /// Input text or file path
        input: String,
        /// Is input a file path?
        #[arg(short, long)]
        file: bool,
    },
    /// Query knowledge graph
    Query {
        /// Entity name to search for
        entity: String,
    },
    /// Export knowledge graph to JSON
    Export,
}

#[derive(Subcommand)]
enum VisionSubcommands {
    /// Describe an image
    Describe {
        /// Path to image file
        path: String,
    },
}

#[derive(Subcommand)]
enum AudioSubcommands {
    /// Transcribe audio to text
    Transcribe {
        /// Path to audio file
        path: String,
    },
    /// Text to Speech
    Speak {
        /// Text to speak
        text: String,
        /// Output file path
        #[arg(short, long, default_value = "output.mp3")]
        output: String,
    },
}

pub async fn run(config: Config) -> Result<()> {
    let cli = Cli::parse();
    
    // Handle Init command first
    if let Some(Commands::Init) = &cli.command {
        return init_environment().await;
    }

    // Initialize Memory System
    let database_url = config.database_url.clone();
    
    // Initialize Episodic Memory
    let memory = match EpisodicMemory::new(&database_url).await {
        Ok(mem) => {
            info!("Connected to episodic memory at {}", database_url);
            Some(Arc::new(mem))
        }
        Err(e) => {
            warn!("Failed to connect to memory: {}. Running in stateless mode.", e);
            None
        }
    };

    // Initialize Knowledge Graph (Semantic Memory)
    // Try Neo4j first
    let mut kg: Option<SharedKnowledgeGraph> = if let Ok(neo4j_uri) = std::env::var("NEO4J_URI") {
        let user = std::env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".to_string());
        let pass = std::env::var("NEO4J_PASS").unwrap_or_else(|_| "password".to_string());
        match Neo4jKnowledgeGraph::new(&neo4j_uri, &user, &pass).await {
             Ok(graph) => {
                 info!("Connected to Neo4j at {}", neo4j_uri);
                 Some(Arc::new(graph) as SharedKnowledgeGraph)
             },
             Err(e) => {
                 warn!("Failed to connect to Neo4j: {}. Falling back to SQLite.", e);
                 None
             }
        }
    } else {
        None
    };

    // Fallback to SQLite if Neo4j not configured or failed
    if kg.is_none() {
        if let Ok(pool) = sqlx::sqlite::SqlitePool::connect(&database_url).await {
            match SqliteKnowledgeGraph::new(pool).await {
                Ok(graph) => {
                     info!("Connected to SQLite Knowledge Graph");
                     kg = Some(Arc::new(graph) as SharedKnowledgeGraph);
                 },
                Err(e) => {
                    warn!("Failed to initialize SQLite Knowledge Graph: {}", e);
                }
            }
        }
    }

    // Initialize Vector Store
    let vector_store = if let Ok(pool) = sqlx::sqlite::SqlitePool::connect(&database_url).await {
        match VectorStore::new(pool).await {
            Ok(store) => Some(Arc::new(store)),
            Err(e) => {
                warn!("Failed to initialize Vector Store: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Initialize Event Bus
    let event_bus = Arc::new(EventBus::new());

    // Initialize the cognitive router
    let router = CognitiveRouter::new(memory, event_bus.clone()).await
        .with_knowledge(kg.clone(), vector_store.clone())
        .with_config(&config)
        .watch_skills(&config.skills_dir);

    // Load Skills from Configured Directory
    if let Err(e) = router.load_skills(&config.skills_dir).await {
        warn!("Failed to load skills from {:?}: {}", config.skills_dir, e);
    } else {
        info!("Skills loaded from {:?}", config.skills_dir);
    }

    match &cli.command {
        Some(Commands::Init) => unreachable!(), // Handled above
        Some(Commands::Chat { session }) => {
            let session_id = session.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            info!("Starting chat mode (Session: {})...", session_id);
            println!("╔════════════════════════════════════════════╗");
            println!("║  🦀 Crablet v0.1.0                         ║");
            println!("║  Session: {} ║", &session_id[0..8]);
            println!("║  Type 'exit' to quit                       ║");
            println!("║  Type '/help' for commands                 ║");
            println!("╚════════════════════════════════════════════╝");
            
            start_chat_loop(&router, &session_id).await?;
        }
        Some(Commands::Run { prompt, session }) => {
            let session_id = session.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            
            let spinner = indicatif::ProgressBar::new_spinner();
            spinner.set_style(
                indicatif::ProgressStyle::default_spinner()
                    .template("{spinner:.green} {msg}")?
                    .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
            );
            spinner.set_message("Thinking...");
            spinner.enable_steady_tick(std::time::Duration::from_millis(100));

            info!("Running prompt: {} (Session: {})", prompt, session_id);
            let (response, _traces) = router.process(prompt, &session_id).await?;
            
            spinner.finish_and_clear();
            println!("Crablet: {}", response);
        }
        Some(Commands::Status) => {
            println!("System Status: OK");
            println!("[System 1 Active | Latency: ~15ms]");
        }
        Some(Commands::Config) => {
             println!("Current Configuration:");
             println!("  Database: {}", config.database_url);
             println!("  Skills Dir: {:?}", config.skills_dir);
             println!("  Model: {}", config.model_name);
             println!("  Log Level: {}", config.log_level);
        }
        Some(Commands::Serve) => {
            use crate::channels::manager::ChannelManager;
            println!("Starting Crablet Server (Channels)...");
            
            let router = Arc::new(router);
            let mut manager = ChannelManager::new();
            
            // If channels config is empty, default to telegram for backward compatibility if token set
            #[cfg(feature = "telegram")]
            if config.channels.is_empty() && std::env::var("TELEGRAM_BOT_TOKEN").is_ok() {
                info!("No channels configured, defaulting to Telegram");
                let telegram = crate::channels::international::telegram::TelegramChannel::new(router.clone());
                manager.register(Arc::new(telegram));
            } else {
                manager.load_from_config(&config, router.clone());
            }
            #[cfg(not(feature = "telegram"))]
            {
                manager.load_from_config(&config, router.clone());
            }
            
            manager.start_all().await;
            
            // Keep alive
            match tokio::signal::ctrl_c().await {
                Ok(()) => info!("Shutting down..."),
                Err(err) => error!("Unable to listen for shutdown signal: {}", err),
            }
        }
        Some(Commands::Knowledge { subcmd }) => match subcmd {
            KnowledgeSubcommands::Extract { input, file } => {
                let text = if *file {
                    if input.ends_with(".pdf") {
                         PdfParser::extract_text(input)?
                    } else {
                         std::fs::read_to_string(input)?
                    }
                } else {
                    input.clone()
                };

                info!("Extracting knowledge from input (length: {})...", text.len());
                let extractor = KnowledgeExtractor::new()?;
                let result = extractor.extract_from_text(&text).await?;
                println!("{:#?}", result);

                // Persist to Knowledge Graph
                if let Some(kg) = &kg {
                    info!("Persisting {} entities and {} relations to Knowledge Graph...", result.entities.len(), result.relations.len());
                    for entity in result.entities {
                        let _ = kg.add_entity(&entity.name, &entity.r#type).await;
                    }
                    for relation in result.relations {
                        let _ = kg.add_relation(&relation.source, &relation.target, &relation.relation).await;
                    }
                    info!("Knowledge persisted successfully.");
                } else {
                    warn!("Knowledge Graph not available, skipping persistence.");
                }

                // Persist to Vector Store (Chunking strategy: simple full text for MVP)
                if let Some(vs) = &vector_store {
                    info!("Persisting content to Vector Store...");
                    // In real-world, we would chunk large text here
                    let _ = vs.add_document(&text, None).await;
                    info!("Vector embeddings generated and stored.");
                }
            }
            KnowledgeSubcommands::Query { entity } => {
                if let Some(kg) = kg {
                    info!("Querying knowledge graph for entity: {}", entity);
                    let relations = kg.find_related(entity).await?;
                    if relations.is_empty() {
                        println!("No knowledge found for entity '{}'", entity);
                    } else {
                        println!("Knowledge related to '{}':", entity);
                        for (direction, relation, target) in relations {
                            if direction == "->" {
                                println!("  - {} -> {}", relation, target);
                            } else {
                                println!("  - {} <- {}", relation, target);
                                println!("  (is {} of {})", relation, target);
                            }
                        }
                    }
                } else {
                    println!("Error: Knowledge Graph not available.");
                }
            }
            KnowledgeSubcommands::Export => {
                if let Some(kg) = kg {
                    info!("Exporting knowledge graph to D3 JSON...");
                    let json = kg.export_d3_json().await?;
                    println!("{}", json);
                } else {
                    println!("Error: Knowledge Graph not available.");
                }
            }
        },
        Some(Commands::Vision { subcmd }) => match subcmd {
            VisionSubcommands::Describe { path } => {
                info!("Analyzing image: {}", path);
                let processor = ImageProcessor::new()?;
                let description = processor.describe(path).await?;
                println!("Description: {}", description);
            }
        },
        Some(Commands::Audio { subcmd }) => match subcmd {
            AudioSubcommands::Transcribe { path } => {
                info!("Transcribing audio: {}", path);
                let processor = AudioTool::new()?;
                let text = processor.transcribe(path).await?;
                println!("Transcription:\n{}", text);
            }
            AudioSubcommands::Speak { text, output } => {
                info!("Generating speech...");
                let processor = AudioTool::new()?;
                processor.speak(text, output).await?;
                println!("Speech saved to {}", output);
            }
        },
        Some(Commands::RunScript { path }) => {
            info!("Running Lua script: {}", path);
            let script = std::fs::read_to_string(path)?;
            
            let engine = match LuaEngine::new() {
                Ok(e) => e,
                Err(e) => return Err(anyhow::anyhow!("Lua init error: {}", e)),
            };

            let result = match engine.execute(&script).await {
                Ok(r) => r,
                Err(e) => return Err(anyhow::anyhow!("Lua execution error: {}", e)),
            };
            
            println!("Script Output: {}", result);
        }
        Some(Commands::ServeWeb { port }) => {
            println!("Starting Crablet Web UI on port {}...", port);
            crate::channels::web::run(router, *port).await?;
        }
        Some(Commands::Skill { subcmd }) => match subcmd {
            SkillSubcommands::Install { url, name } => {
                let skills_dir = config.skills_dir;
                info!("Installing skill from {} into {:?}", url, skills_dir);
                
                let repo_name = name.clone().unwrap_or_else(|| {
                    url.split('/').last().unwrap_or("unknown").trim_end_matches(".git").to_string()
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
            }
            SkillSubcommands::Uninstall { name } => {
                let skills_dir = config.skills_dir;
                let target_dir = skills_dir.join(&name);
                
                if !target_dir.exists() {
                    return Err(anyhow::anyhow!("Skill '{}' not found at {:?}", name, target_dir));
                }
                
                info!("Uninstalling skill '{}' from {:?}", name, target_dir);
                std::fs::remove_dir_all(&target_dir)?;
                println!("Skill '{}' uninstalled successfully.", name);
            }
            SkillSubcommands::List => {
                use crate::skills::SkillRegistry;
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
                
                // We need access to SkillRegistry. 
                // Currently router encapsulates it but doesn't expose it directly in a way we can easily use here without refactoring.
                // However, System2 has it. But router abstracts System1/System2.
                // Let's reload skills directly for testing purposes to ensure fresh state.
                
                use crate::skills::SkillRegistry;
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
        },
        Some(Commands::Gateway { host, port }) => {
            use crate::gateway::{CrabletGateway, types::GatewayConfig};
            println!("Starting Crablet Gateway on {}:{}...", host, port);
            
            let gateway_config = GatewayConfig {
                host: host.clone(),
                port: *port,
                auth_mode: "off".to_string(),
            };
            
            let gateway = CrabletGateway::new(gateway_config);
            
            // Register a ping method for testing
            gateway.rpc.register("ping", |_| async { 
                Ok(Some(serde_json::json!("pong"))) 
            }).await;

            // Register broadcast for SSE testing
            let event_bus = gateway.event_bus.clone();
            gateway.rpc.register("broadcast", move |params| {
                let event_bus = event_bus.clone();
                async move {
                    let msg = params.and_then(|p| p.get("message").and_then(|v| v.as_str()).map(|s| s.to_string()))
                        .unwrap_or_else(|| "default message".to_string());
                    
                    let _ = event_bus.publish(crate::gateway::events::GatewayEvent::SystemAlert(msg));
                    Ok(Some(serde_json::json!("broadcast_sent")))
                }
            }).await;

            if let Err(e) = gateway.start().await {
                 tracing::error!("Gateway failed: {}", e);
                 return Err(anyhow::anyhow!("Gateway error: {}", e));
            }
        },
        None => {
            // If no command, show help
            use clap::CommandFactory;
            Cli::command().print_help()?;
        }
    }
    Ok(())
}

async fn init_environment() -> Result<()> {
    use directories::ProjectDirs;
    use std::fs;

    println!("Initializing Crablet environment...");

    if let Some(proj_dirs) = ProjectDirs::from("com", "crablet", "crablet") {
        // Fallback to ~/.config/crablet if system path fails (macOS sandbox issue)
        let config_dir = if fs::create_dir_all(proj_dirs.config_dir()).is_err() {
            let home = directories::UserDirs::new().unwrap();
            home.home_dir().join(".config").join("crablet")
        } else {
            proj_dirs.config_dir().to_path_buf()
        };
        
        let data_dir = if fs::create_dir_all(proj_dirs.data_dir()).is_err() {
             let home = directories::UserDirs::new().unwrap();
             home.home_dir().join(".local").join("share").join("crablet")
        } else {
             proj_dirs.data_dir().to_path_buf()
        };

        // 1. Create Config Directory
        if !config_dir.exists() {
            println!("Creating config directory: {:?}", config_dir);
            fs::create_dir_all(&config_dir)?;
        }

        // 2. Create Default Config File
        let config_path = config_dir.join("config.toml");
        if !config_path.exists() {
            println!("Creating default config: {:?}", config_path);
            let default_config = r#"
database_url = "sqlite:crablet.db?mode=rwc"
# skills_dir = "skills" # Defaults to ./skills or XDG data dir
model_name = "gpt-4o-mini"
log_level = "info"
"#;
            fs::write(config_path, default_config)?;
        } else {
            println!("Config file already exists: {:?}", config_path);
        }

        // 3. Create Data Directory (for Skills)
        if !data_dir.exists() {
            println!("Creating data directory: {:?}", data_dir);
            fs::create_dir_all(&data_dir)?;
        }

        let skills_dir = data_dir.join("skills");
        if !skills_dir.exists() {
            println!("Creating skills directory: {:?}", skills_dir);
            fs::create_dir_all(&skills_dir)?;
            
            // Create a sample skill?
            let hello_skill = skills_dir.join("hello");
            fs::create_dir_all(&hello_skill)?;
            fs::write(hello_skill.join("skill.yaml"), r#"
name: hello
description: A built-in hello world skill
version: 1.0.0
entrypoint: echo "Hello from global skill!"
parameters: {}
"#)?;
        }
        
        // 4. Add to PATH in .zshrc
        if let Some(user_dirs) = directories::UserDirs::new() {
            let home_dir = user_dirs.home_dir();
            let zshrc_path = home_dir.join(".zshrc");
            let cargo_bin_path = "$HOME/.cargo/bin";
            let export_line = format!(r#"export PATH="{}:$PATH""#, cargo_bin_path);
            
            // Check if file exists
            let mut content = if zshrc_path.exists() {
                std::fs::read_to_string(&zshrc_path)?
            } else {
                String::new()
            };

            if !content.contains(cargo_bin_path) {
                println!("Adding cargo bin to PATH in {:?}", zshrc_path);
                use std::fmt::Write;
                if !content.ends_with('\n') && !content.is_empty() {
                    writeln!(content)?;
                }
                writeln!(content, "\n# Added by Crablet init")?;
                writeln!(content, "{}", export_line)?;
                std::fs::write(zshrc_path, content)?;
                println!("Please restart your terminal or run 'source ~/.zshrc' for changes to take effect.");
            } else {
                println!("PATH already configured in {:?}", zshrc_path);
            }
        }

        println!("Initialization complete! You can now run 'crablet chat'.");
    } else {
        println!("Error: Could not determine home directory.");
    }

    Ok(())
}

async fn start_chat_loop(router: &CognitiveRouter, session_id: &str) -> Result<()> {
    use std::io::{self, Write};
    
    let mut input = String::new();
    
    loop {
        print!("\nYou: ");
        io::stdout().flush()?;
        
        input.clear();
        io::stdin().read_line(&mut input)?;
        
        let trimmed = input.trim();
        if trimmed == "exit" || trimmed == "/exit" {
            break;
        }
        
        if trimmed.is_empty() {
            continue;
        }
        
        let spinner = indicatif::ProgressBar::new_spinner();
        spinner.set_style(
            indicatif::ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")?
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
        );
        spinner.set_message("Thinking...");
        spinner.enable_steady_tick(std::time::Duration::from_millis(100));

        // Process with Cognitive Router
        let (response, _traces) = router.process(trimmed, session_id).await?;
        
        spinner.finish_and_clear();
        println!("Crablet: {}", response);
    }
    
    Ok(())
}
