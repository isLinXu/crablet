use crate::channels::Channel;
use anyhow::Result;
use async_trait::async_trait;

pub struct WeComChannel;

impl WeComChannel {
    pub fn new(_config: &crate::config::Config) -> Self {
        Self
    }
}

#[async_trait]
impl Channel for WeComChannel {
    async fn send(&self, _to: &str, _content: &str) -> Result<()> {
        Ok(())
    }

    async fn start(&self) -> Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "wecom"
    }
}
