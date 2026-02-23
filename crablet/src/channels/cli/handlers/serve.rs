use anyhow::Result;
use tracing::{info, error};
use std::sync::Arc;
use crate::config::Config;
use crate::cognitive::router::CognitiveRouter;
use crate::channels::manager::ChannelManager;

pub async fn handle_serve(router: Arc<CognitiveRouter>, config: &Config) -> Result<()> {
    println!("Starting Crablet Server (Channels)...");
    
    let mut manager = ChannelManager::new();
    
    // If channels config is empty, default to telegram for backward compatibility if token set
    #[cfg(feature = "telegram")]
    if config.channels.is_empty() && std::env::var("TELEGRAM_BOT_TOKEN").is_ok() {
        info!("No channels configured, defaulting to Telegram");
        let telegram = crate::channels::international::telegram::TelegramChannel::new(router.clone());
        manager.register(Arc::new(telegram));
    } else {
        manager.load_from_config(config, router.clone());
    }
    #[cfg(not(feature = "telegram"))]
    {
        manager.load_from_config(config, router.clone());
    }
    
    manager.start_all().await;
    
    // Keep alive
    match tokio::signal::ctrl_c().await {
        Ok(()) => info!("Shutting down..."),
        Err(err) => error!("Unable to listen for shutdown signal: {}", err),
    }
    
    Ok(())
}
