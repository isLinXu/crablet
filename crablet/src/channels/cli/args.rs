use clap::{Parser, Subcommand};

#[cfg(feature = "auto-working")]
#[derive(Subcommand, Clone)]
pub enum TaskSubcommands {
    /// List all scheduled tasks
    List {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,
    },
    /// Schedule a new task
    Schedule {
        /// Task name
        name: String,
        /// Cron expression or schedule type
        #[arg(short, long)]
        cron: Option<String>,
        /// Run once at specific time (ISO 8601)
        #[arg(short, long)]
        at: Option<String>,
        /// Task action type (cognitive, rpa, workflow)
        #[arg(short, long, default_value = "cognitive")]
        action_type: String,
        /// Action payload (JSON or prompt)
        #[arg(short, long)]
        payload: String,
    },
    /// Cancel a scheduled task
    Cancel {
        /// Task ID
        id: String,
    },
    /// Show task details
    Show {
        /// Task ID
        id: String,
    },
    /// View task execution history
    History {
        /// Task ID (optional, shows all if not provided)
        id: Option<String>,
        /// Limit number of results
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
    /// Pause a task
    Pause {
        /// Task ID
        id: String,
    },
    /// Resume a paused task
    Resume {
        /// Task ID
        id: String,
    },
    /// Run a task immediately (one-time execution)
    Run {
        /// Task name or ID
        name: String,
        /// Pass-through arguments (JSON)
        #[arg(short, long, default_value = "{}")]
        args: String,
    },
    /// Show task queue status
    Queue {
        /// Show pending tasks
        #[arg(short, long)]
        pending: bool,
        /// Show running tasks
        #[arg(short, long)]
        running: bool,
        /// Show failed tasks
        #[arg(short, long)]
        failed: bool,
    },
}

#[cfg(feature = "auto-working")]
#[derive(Subcommand, Clone)]
pub enum WorkflowSubcommands {
    /// List available workflows
    List,
    /// Show workflow details
    Show {
        /// Workflow name or path
        name: String,
    },
    /// Run a workflow
    Run {
        /// Workflow name or path
        name: String,
        /// Workflow parameters (JSON)
        #[arg(short, long, default_value = "{}")]
        params: String,
        /// Run asynchronously
        #[arg(short, long)]
        background: bool,
    },
    /// Validate a workflow file
    Validate {
        /// Path to workflow file
        path: String,
    },
    /// Create a new workflow from template
    Create {
        /// Workflow name
        name: String,
        /// Template type (browser, data, notification)
        #[arg(short, long, default_value = "browser")]
        template: String,
    },
    /// Export workflow execution results
    Export {
        /// Execution ID
        execution_id: String,
        /// Output file path
        #[arg(short, long, default_value = "workflow_result.json")]
        output: String,
    },
}

#[cfg(feature = "auto-working")]
#[derive(Subcommand, Clone)]
pub enum ConnectorSubcommands {
    /// List configured connectors
    List {
        /// Show only active connectors
        #[arg(short, long)]
        active: bool,
    },
    /// Add a new connector
    Add {
        /// Connector type (email, webhook, filesystem, database, calendar)
        connector_type: String,
        /// Connector name
        name: String,
        /// Configuration file path (JSON)
        #[arg(short, long)]
        config: Option<String>,
    },
    /// Remove a connector
    Remove {
        /// Connector ID or name
        id: String,
    },
    /// Test connector connection
    Test {
        /// Connector ID or name
        id: String,
    },
    /// Show connector details and health
    Status {
        /// Connector ID or name
        id: String,
    },
    /// Start a connector
    Start {
        /// Connector ID or name
        id: String,
    },
    /// Stop a connector
    Stop {
        /// Connector ID or name
        id: String,
    },
    /// View connector event logs
    Logs {
        /// Connector ID or name
        id: String,
        /// Number of lines to show
        #[arg(short, long, default_value = "50")]
        lines: usize,
        /// Follow logs (tail mode)
        #[arg(short, long)]
        follow: bool,
    },
}

#[cfg(feature = "auto-working")]
#[derive(Subcommand, Clone)]
pub enum RpaSubcommands {
    /// List browser sessions
    BrowserSessions,
    /// Start a new browser session
    BrowserStart {
        /// Headless mode
        #[arg(short, long, default_value = "true")]
        headless: bool,
        /// Browser viewport width
        #[arg(short, long, default_value = "1920")]
        width: u32,
        /// Browser viewport height
        #[arg(short, long, default_value = "1080")]
        height: u32,
    },
    /// Close a browser session
    BrowserClose {
        /// Session ID
        id: String,
    },
    /// Execute browser automation
    BrowserExec {
        /// Session ID (creates new if not provided)
        #[arg(short, long)]
        session: Option<String>,
        /// Workflow file or inline steps (JSON)
        workflow: String,
    },
    /// Take a screenshot
    Screenshot {
        /// URL to screenshot
        url: String,
        /// Output file path
        #[arg(short, long, default_value = "screenshot.png")]
        output: String,
        /// Full page screenshot
        #[arg(short, long)]
        full_page: bool,
    },
    /// List RPA workflows
    Workflows,
    /// Execute desktop automation
    Desktop {
        /// Desktop workflow file or steps (JSON)
        workflow: String,
    },
}

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
    
    /// Auto-Working Task Management
    #[cfg(feature = "auto-working")]
    Task {
        #[command(subcommand)]
        subcmd: TaskSubcommands,
    },
    
    /// Workflow Management
    #[cfg(feature = "auto-working")]
    Workflow {
        #[command(subcommand)]
        subcmd: WorkflowSubcommands,
    },
    
    /// Connector Management
    #[cfg(feature = "auto-working")]
    Connector {
        #[command(subcommand)]
        subcmd: ConnectorSubcommands,
    },
    
    /// RPA Automation
    #[cfg(feature = "auto-working")]
    Rpa {
        #[command(subcommand)]
        subcmd: RpaSubcommands,
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
        /// Interactive mode with guided setup
        #[arg(short, long)]
        interactive: bool,
        /// Skip signature verification
        #[arg(long)]
        skip_verify: bool,
        /// Force reinstall if already exists
        #[arg(short, long)]
        force: bool,
        /// Use isolated environment
        #[arg(long)]
        isolated: bool,
    },
    /// Uninstall a skill
    Uninstall {
        /// Name of the skill to uninstall
        name: String,
        /// Force uninstall without confirmation
        #[arg(short, long)]
        force: bool,
    },
    /// Search for skills in the registry
    Search {
        /// Query string
        query: String,
        /// Filter by category
        #[arg(short, long)]
        category: Option<String>,
        /// Maximum results
        #[arg(short, long, default_value = "10")]
        limit: usize,
        /// Semantic search (natural language)
        #[arg(short, long)]
        semantic: bool,
    },
    /// List installed skills
    List {
        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
        /// Show update status
        #[arg(short, long)]
        updates: bool,
    },
    /// Import skill from ClawHub URL
    Import {
        /// ClawHub skill URL
        url: String,
    },
    /// Check for skill updates
    Update {
        /// Skill name (if not provided, checks all)
        name: Option<String>,
        /// List available updates without installing
        #[arg(short, long)]
        list: bool,
        /// Apply all updates automatically
        #[arg(short, long)]
        all: bool,
    },
    /// Show skill information
    Info {
        /// Skill name
        name: String,
        /// Show full documentation
        #[arg(short, long)]
        docs: bool,
    },
    /// Interactive skill installation wizard
    Wizard,
    /// Developer tools for skill development
    Dev {
        #[command(subcommand)]
        subcmd: SkillDevSubcommands,
    },
}

#[derive(Subcommand, Clone)]
pub enum SkillDevSubcommands {
    /// Initialize a new skill project
    Init {
        /// Skill name
        name: String,
        /// Skill type
        #[arg(short, long, default_value = "openclaw")]
        skill_type: String,
        /// Project directory (default: skill name)
        #[arg(short, long)]
        path: Option<std::path::PathBuf>,
    },
    /// Validate skill project
    Validate {
        /// Project path (default: current directory)
        #[arg(short, long)]
        path: Option<std::path::PathBuf>,
    },
    /// Run tests for skill
    Test {
        /// Project path (default: current directory)
        #[arg(short, long)]
        path: Option<std::path::PathBuf>,
        /// Test arguments
        #[arg(short, long)]
        args: Option<String>,
    },
    /// Build skill package
    Build {
        /// Project path (default: current directory)
        #[arg(short, long)]
        path: Option<std::path::PathBuf>,
        /// Output directory
        #[arg(short, long, default_value = "dist")]
        output: std::path::PathBuf,
    },
    /// Publish skill to registry
    Publish {
        /// Project path (default: current directory)
        #[arg(short, long)]
        path: Option<std::path::PathBuf>,
        /// Registry URL
        #[arg(short, long)]
        registry: Option<String>,
        /// Dry run (don't actually publish)
        #[arg(long)]
        dry_run: bool,
    },
    /// Generate documentation
    Docs {
        /// Project path (default: current directory)
        #[arg(short, long)]
        path: Option<std::path::PathBuf>,
        /// Output directory
        #[arg(short, long, default_value = "docs/generated")]
        output: std::path::PathBuf,
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
