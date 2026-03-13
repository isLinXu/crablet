pub mod args;
pub mod handlers;
pub mod context;

use anyhow::Result;
use clap::Parser;
use crate::config::Config;
use args::{Cli, Commands};
use context::AppContext;

pub async fn run(config: Config) -> Result<()> {
    let cli = Cli::parse();
    
    // Handle Init command first
    if let Some(Commands::Init) = &cli.command {
        return handlers::init::init_environment().await;
    }

    // Initialize App Context
    let app = std::sync::Arc::new(AppContext::new(config.clone()).await?);
    let router = app.router.clone();
    let lane_router = app.lane_router.clone();

    match &cli.command {
        Some(Commands::Init) => unreachable!(), // Handled above
        Some(Commands::Chat { session }) => {
            handlers::chat::handle_chat(&lane_router, &router, session.as_deref()).await
        }
        Some(Commands::Run { prompt, session }) => {
            handlers::chat::handle_run(&lane_router, prompt, session.as_deref()).await
        }
        #[cfg(feature = "discord")]
        Some(Commands::Discord) => {
            crate::channels::discord::run(app).await
        }
        Some(Commands::Status) => {
            handlers::status::handle_status()
        }
        Some(Commands::Config) => {
            handlers::config::handle_config(&config)
        }
        Some(Commands::Serve) => {
            handlers::serve::handle_serve(router, &config).await
        }
        #[cfg(feature = "knowledge")]
        Some(Commands::Knowledge { subcmd }) => {
            handlers::knowledge::handle_knowledge(subcmd, app.kg.clone(), app.vector_store.clone()).await
        }
        Some(Commands::Vision { subcmd }) => {
            handlers::vision::handle_vision(subcmd).await
        }
        #[cfg(feature = "audio")]
        Some(Commands::Audio { subcmd }) => {
            handlers::audio::handle_audio(subcmd).await
        }
        #[cfg(feature = "scripting")]
        Some(Commands::RunScript { path }) => {
            handlers::script::handle_run_script(path).await
        }
        #[cfg(feature = "web")]
        Some(Commands::ServeWeb { port }) => {
            handlers::web::handle_serve_web(router, *port, &config).await
        }
        Some(Commands::Skill { subcmd }) => {
            handlers::skill::handle_skill(subcmd, &config, &router).await
        }
        #[cfg(feature = "web")]
        Some(Commands::Gateway { host, port }) => {
            handlers::gateway::handle_gateway(host, *port, router.clone()).await
        }
        Some(Commands::Research { topic, depth }) => {
            handlers::research::handle_research(topic.clone(), *depth).await
        }
        Some(Commands::Debug { session_id }) => {
            handlers::debug::handle_debug(session_id, app.event_bus.clone()).await
        }
        Some(Commands::Audit { path, format }) => {
            handlers::audit::handle_audit(&router, path.clone(), format.clone()).await
        }
        Some(Commands::Analyze { path, goal }) => {
            handlers::analyze::handle_analyze(&router, path.clone(), goal.clone()).await
        }
        #[cfg(feature = "auto-working")]
        Some(Commands::Task { subcmd }) => {
            handlers::task::handle_task(subcmd).await
        }
        #[cfg(feature = "auto-working")]
        Some(Commands::Workflow { subcmd }) => {
            handlers::workflow::handle_workflow(subcmd).await
        }
        #[cfg(feature = "auto-working")]
        Some(Commands::Connector { subcmd }) => {
            handlers::connector::handle_connector(subcmd).await
        }
        #[cfg(feature = "auto-working")]
        Some(Commands::Rpa { subcmd }) => {
            handlers::rpa::handle_rpa(subcmd).await
        }
        None => {
            use clap::CommandFactory;
            Cli::command().print_help()?;
            Ok(())
        }
    }
}
