use anyhow::Result;
use std::sync::Arc;
use tracing::{info, error};
use crate::channels::cli::context::AppContext;

#[cfg(feature = "discord")]
use serenity::{
    async_trait,
    model::{channel::Message, gateway::Ready},
    prelude::*,
};

#[cfg(feature = "discord")]
struct Handler {
    app: Arc<AppContext>,
}

#[cfg(feature = "discord")]
#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        info!("Discord Message from {}: {}", msg.author.name, msg.content);
        
        // Simple session ID based on channel and user
        let session_id = format!("discord-{}-{}", msg.channel_id, msg.author.id);

        // Dispatch to Lane Router (System 2)
        match self.app.lane_router.dispatch(&session_id, msg.content.clone()).await {
            Ok((response, _traces)) => {
                if let Err(why) = msg.channel_id.say(&ctx.http, response).await {
                    error!("Error sending message: {:?}", why);
                }
            }
            Err(e) => {
                error!("Error processing message: {:?}", e);
                let _ = msg.channel_id.say(&ctx.http, "I encountered an error processing your request.").await;
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

pub async fn run(app: Arc<AppContext>) -> Result<()> {
    #[cfg(not(feature = "discord"))]
    {
        return Err(anyhow::anyhow!("Discord feature is not enabled"));
    }

    #[cfg(feature = "discord")]
    {
        let token = std::env::var("DISCORD_TOKEN").map_err(|_| anyhow::anyhow!("DISCORD_TOKEN environment variable not set"))?;

        // Set gateway intents, which decides what events the bot will be notified about
        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT;

        let mut client = Client::builder(&token, intents)
            .event_handler(Handler { app })
            .await
            .map_err(|e| anyhow::anyhow!("Error creating client: {:?}", e))?;

        info!("Starting Discord bot...");
        if let Err(why) = client.start().await {
            error!("Client error: {:?}", why);
            return Err(anyhow::anyhow!("Client error: {:?}", why));
        }

        Ok(())
    }
}
