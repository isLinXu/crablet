use crablet as sqlx;
use crablet::channels::cli;
use crablet::config::Config;
use std::sync::Arc;
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
            openai_api_key: std::env::var("OPENAI_API_KEY").ok(),
            ..Default::default()
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
    // 桌面端首次启动时用户可能尚未配置 API Key，此时不应 panic，
    // 而是跳过 LLM 健康检查、使用 MockClient 占位，让 Web Gateway 正常启动。
    // 用户在 Settings 中配置 API Key 后，sidecar 会重启并走正常路径。
    let llm_client = match crablet::cognitive::create_llm_client(&config).await {
        Ok(client) => client,
        Err(e) => {
            tracing::warn!(
                "LLM client initialization failed (API Key not configured?): {}. \
                 Starting with MockClient — LLM features will be unavailable until API Key is set.",
                e
            );
            Arc::new(crablet::cognitive::llm::MockClient) as Arc<dyn crablet::cognitive::llm::LlmClient>
        }
    };

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
