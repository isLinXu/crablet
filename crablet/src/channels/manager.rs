use std::collections::HashMap;
use std::sync::Arc;
use crate::channels::Channel;
use crate::config::Config;
use crate::cognitive::router::CognitiveRouter;

pub struct ChannelManager {
    channels: HashMap<String, Arc<dyn Channel>>,
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelManager {
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
        }
    }

    pub fn load_from_config(&mut self, config: &Config, _router: Arc<CognitiveRouter>) {
        // If config.channels is empty, we might want defaults or just nothing.
        // For now, only load what's in config.
        for name in &config.channels {
            match name.as_str() {
                "feishu" => {
                    let channel = crate::channels::domestic::feishu::FeishuChannel::new(config);
                    self.register(Arc::new(channel));
                }
                "dingtalk" => {
                    let channel = crate::channels::domestic::dingtalk::DingTalkChannel::new();
                    self.register(Arc::new(channel));
                }
                "telegram" => {
                    #[cfg(feature = "telegram")]
                    {
                        let channel = crate::channels::international::telegram::TelegramChannel::new(_router.clone());
                        self.channels.insert(name.to_string(), Arc::new(channel));
                    }
                }
                "webhook" => {
                    let channel = crate::channels::universal::http_webhook::HttpWebhookChannel::new();
                    self.register(Arc::new(channel));
                }
                _ => {
                    tracing::warn!("Unknown channel configured: {}", name);
                }
            }
        }
    }

    pub fn register(&mut self, channel: Arc<dyn Channel>) {
        self.channels.insert(channel.name().to_string(), channel);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Channel>> {
        self.channels.get(name).cloned()
    }
    
    pub async fn start_all(&self) {
        for (name, channel) in &self.channels {
            let name = name.clone();
            let channel = channel.clone();
            tracing::info!("Starting channel service: {}", name);
            tokio::spawn(async move {
                if let Err(e) = channel.start().await {
                    tracing::error!("Channel service {} exited with error: {}", name, e);
                } else {
                    tracing::info!("Channel service {} stopped.", name);
                }
            });
        }
    }
}
