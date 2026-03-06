use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "crablet")]
#[command(about = "🦀 Crablet: Next-gen AI Assistant", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
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
    #[cfg(feature = "knowledge")]
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
    #[cfg(feature = "audio")]
    Audio {
        #[command(subcommand)]
        subcmd: AudioSubcommands,
    },
    /// Run a Lua script
    #[cfg(feature = "scripting")]
    RunScript {
        /// Path to Lua script
        path: String,
    },
    /// Web UI
    #[cfg(feature = "web")]
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
    #[cfg(feature = "web")]
    Gateway {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Port to listen on
        #[arg(long, default_value = "18789")]
        port: u16,
    },
    
    /// Run Discord bot
    #[cfg(feature = "discord")]
    Discord,
    
    /// Deep Research Mode
    Research {
        /// Topic to research
        topic: String,
        /// Maximum number of search iterations
        #[arg(short, long, default_value = "3")]
        depth: usize,
    },
    
    /// Debug a session
    Debug {
        /// Session ID to replay
        #[arg(index = 1)]
        session_id: String,
    },
    
    /// Perform a security audit on a codebase
    Audit {
        /// Path to the codebase
        path: String,
        /// Report format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },

    /// Analyze a data file (CSV/JSON) using DataAnalystAgent
    Analyze {
        /// Path to the data file
        path: String,
        /// Goal of the analysis
        #[arg(short, long, default_value = "Analyze the data distribution and summary statistics")]
        goal: String,
    },
}

#[derive(Subcommand, Clone)]
pub enum SkillSubcommands {
    /// Test a skill in isolation
    Test {
        /// Name of the skill to test
        name: String,
        /// JSON arguments for the skill (e.g. '{"arg1": "val"}')
        #[arg(default_value = "{}")]
        args: String,
    },
    /// Install a skill from a git repository or ClawHub URL
    Install {
        /// Skill name or Git URL
        name_or_url: String,
        /// Optional name override
        name: Option<String>,
    },
    /// Uninstall a skill
    Uninstall {
        /// Name of the skill to uninstall
        name: String,
    },
    /// Search for skills in the registry
    Search {
        /// Query string
        query: String,
    },
    /// List installed skills
    List,
    /// Import skill from ClawHub URL
    Import {
        /// ClawHub skill URL
        url: String,
    },
}

#[cfg(feature = "knowledge")]
#[derive(Subcommand)]
pub enum KnowledgeSubcommands {
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
    /// List documents in knowledge base
    List,
    /// Export knowledge graph to JSON
    Export,
}

#[derive(Subcommand)]
pub enum VisionSubcommands {
    /// Describe an image
    Describe {
        /// Path to image file
        path: String,
    },
}

#[cfg(feature = "audio")]
#[derive(Subcommand)]
pub enum AudioSubcommands {
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
