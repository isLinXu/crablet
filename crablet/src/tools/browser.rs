use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use crate::plugins::Plugin;
use tracing::info;

pub struct BrowserPlugin;

#[async_trait]
impl Plugin for BrowserPlugin {
    fn name(&self) -> &str {
        "browse_web"
    }

    fn description(&self) -> &str {
        "Browse a website and extract content. Args: { \"url\": \"https://...\" }"
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        #[cfg(not(feature = "browser"))]
        return Err(anyhow::anyhow!("Browser feature is not enabled"));

        #[cfg(feature = "browser")]
        {
            let url_str = args.get("url").and_then(|v| v.as_str()).unwrap_or("");
            if url_str.is_empty() {
                return Err(anyhow::anyhow!("Missing 'url' argument"));
            }
            let url = url_str.to_string();

            // Run blocking browser operations in a separate thread
            let output = tokio::task::spawn_blocking(move || -> Result<String> {
                use headless_chrome::{Browser, LaunchOptions};
                
                let options = LaunchOptions::default();
                let browser = Browser::new(options).map_err(|e| anyhow::anyhow!("Failed to launch browser: {}", e))?;
                
                let tab = browser.new_tab().map_err(|e| anyhow::anyhow!("Failed to create tab: {}", e))?;
                
                tab.navigate_to(&url).map_err(|e| anyhow::anyhow!("Navigation failed: {}", e))?;
                tab.wait_until_navigated().map_err(|e| anyhow::anyhow!("Navigation wait failed: {}", e))?;
                
                // Extract text from body
                let element = tab.wait_for_element("body").map_err(|e| anyhow::anyhow!("Failed to find body: {}", e))?;
                let text = element.get_inner_text().map_err(|e| anyhow::anyhow!("Failed to get text: {}", e))?;
                
                Ok(text)
            }).await??;
            
            // Truncate if too long
            let len = output.len();
            if len > 5000 {
                info!("Browser output truncated ({} chars)", len);
                Ok(output[..5000].to_string() + "\n... (truncated)")
            } else {
                Ok(output)
            }
        }
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}
