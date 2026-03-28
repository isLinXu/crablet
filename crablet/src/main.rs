use crablet::channels::cli;
use crablet::config::Config;
use crablet as sqlx;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env file
    if let Err(e) = dotenvy::dotenv() {
        eprintln!("Warning: Failed to load .env file: {}", e);
    }

    // Load Config early
    let config = if let Ok(cfg) = Config::load() {
        cfg
    } else {
        tracing::warn!("Failed to load config, using defaults.");
        Config {
            database_url: "sqlite:crablet.db?mode=rwc".to_string(),
            skills_dir: std::path::PathBuf::from("skills"),
            model_name: "gpt-4o-mini".to_string(),
            llm_vendor: None,
            log_level: "info".to_string(),
            mcp_servers: std::collections::HashMap::new(),
            channels: vec![],
            semantic_cache_threshold: 0.92,
            system2_threshold: 0.3,
            system3_threshold: 0.7,
            enable_adaptive_routing: false,
            bandit_exploration: 0.55,
            enable_hierarchical_reasoning: true,
            deliberate_threshold: 0.58,
            meta_reasoning_threshold: 0.82,
            mcts_simulations: 24,
            mcts_exploration_weight: 1.2,
            graph_rag_entity_mode: "hybrid".to_string(),
            openai_api_key: std::env::var("OPENAI_API_KEY").ok(),
            port: 3000,
            providers: std::collections::HashMap::new(),
            ollama_model: "qwen2.5:14b".to_string(),
            serper_api_key: None,
            feishu_app_id: None,
            feishu_app_secret: None,
            wecom_corp_id: None,
            wecom_corp_secret: None,
            wecom_agent_id: None,
            oidc_issuer: None,
            oidc_client_id: None,
            oidc_client_secret: None,
            jwt_secret: None,
        }
    };

    // Initialize Telemetry (Logging + Tracing)
    crablet::telemetry::init_telemetry(&config.log_level)?;
    
    info!("🦀 Crablet v0.1.0 starting up...");
    
    // Migration Check (Proper)
    if let Ok(pool) = sqlx::sqlite::SqlitePool::connect(&config.database_url).await {
        info!("Running database migrations...");
        let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        let migrator = sqlx::migrate::Migrator::new(migrations_dir.as_path()).await?;
        match migrator.run(&pool).await {
            Ok(_) => info!("Migrations applied successfully."),
            Err(e) => {
                tracing::error!("Migration failed: {}", e);
                // Decide if we should exit or continue. For now, exit is safer.
                return Err(anyhow::anyhow!("Database migration failed: {}", e));
            }
        }
        pool.close().await;
    }

    // Initialize LLM Client for Health Check
    let llm_client = crablet::cognitive::create_llm_client(&config).await?;
    
    // Health Check
    info!("Running startup health checks...");
    if let Ok(report) = crablet::health::startup_health_check(&config, llm_client.clone()).await {
        info!("Health Report: {:?}", report);
        if report.status == "unhealthy" {
            tracing::warn!("System is starting in unhealthy state!");
        }
    } else {
        tracing::error!("Failed to run health checks");
    }

    // Agent Factory

    // Start CLI
    if let Err(e) = cli::run(config).await {
        tracing::error!("Application error: {}", e);
    }
    
    // Cleanup Telemetry
    crablet::telemetry::shutdown_telemetry();
    
    Ok(())
}
