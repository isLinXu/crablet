use teloxide::prelude::*;
use crate::cognitive::router::CognitiveRouter;
use std::sync::Arc;
use anyhow::Result;
use tracing::{info, error};
use async_trait::async_trait;
use crate::channels::Channel;

pub struct TelegramChannel {
    bot: Bot,
    router: Arc<CognitiveRouter>,
}

impl TelegramChannel {
    pub fn new(router: Arc<CognitiveRouter>) -> Self {
        Self {
            bot: Bot::from_env(),
            router,
        }
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    async fn send(&self, to: &str, content: &str) -> Result<()> {
        let chat_id = to.parse::<i64>().map_err(|_| anyhow::anyhow!("Invalid chat_id"))?;
        self.bot.send_message(ChatId(chat_id), content).await?;
        Ok(())
    }

    async fn start(&self) -> Result<()> {
        info!("Starting Telegram bot...");

        // Check if TELEGRAM_BOT_TOKEN is set
        if std::env::var("TELEGRAM_BOT_TOKEN").is_err() {
            error!("TELEGRAM_BOT_TOKEN not set. Telegram bot disabled.");
            return Ok(());
        }

        // We need to clone the router for the handler
        let router = self.router.clone();

        let handler = Update::filter_message()
            .endpoint(move |bot: Bot, msg: Message, router: Arc<CognitiveRouter>| async move {
                if let Some(text) = msg.text() {
                    // Show "typing..." status
                    let _ = bot.send_chat_action(msg.chat.id, teloxide::types::ChatAction::Typing).await;
                    
                    // Process message through Cognitive Router
                    let session_id = format!("telegram_{}", msg.chat.id);
                    match router.process(text, &session_id).await {
                        Ok((response, _traces)) => {
                            // Send plain text response for now to avoid Markdown parsing errors
                            if let Err(e) = bot.send_message(msg.chat.id, response).await {
                                error!("Failed to send Telegram message: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Error processing message: {}", e);
                            let _ = bot.send_message(msg.chat.id, format!("Error: {}", e)).await;
                        }
                    }
                }
                respond(())
            });

        Dispatcher::builder(self.bot.clone(), handler)
            .dependencies(dptree::deps![router])
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;

        Ok(())
    }

    fn name(&self) -> &str {
        "telegram"
    }
}

// Backward compatibility function
pub async fn run(router: CognitiveRouter) -> Result<()> {
    let channel = TelegramChannel::new(Arc::new(router));
    channel.start().await
}
