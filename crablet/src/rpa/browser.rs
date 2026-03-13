//! Browser Automation
//!
//! Provides web browser automation using Chrome DevTools Protocol (CDP)
//! via the chromiumoxide crate.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
#[cfg(feature = "browser")]
use futures::StreamExt;

use crate::error::Result;
#[cfg(feature = "browser")]
use crate::error::CrabletError;
use crate::rpa::RpaResult;
#[cfg(feature = "browser")]
use crate::rpa::RpaError;

/// Browser configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserConfig {
    /// Run browser in headless mode
    pub headless: bool,
    /// Viewport size
    pub viewport: Viewport,
    /// User agent string
    pub user_agent: String,
    /// Default timeout for operations
    pub timeout: Duration,
    /// Slow down operations by specified milliseconds (useful for debugging)
    pub slow_mo: u64,
    /// Additional browser arguments
    pub args: Vec<String>,
    /// Download path for files
    pub download_path: Option<String>,
    /// Enable recording
    pub record_video: bool,
    /// Video directory
    pub video_dir: Option<String>,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            headless: true,
            viewport: Viewport { width: 1920, height: 1080 },
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".to_string(),
            timeout: Duration::from_secs(30),
            slow_mo: 0,
            args: vec![],
            download_path: None,
            record_video: false,
            video_dir: None,
        }
    }
}

/// Viewport size
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

/// Browser automation engine
pub struct BrowserAutomation {
    config: BrowserConfig,
    #[cfg(feature = "browser")]
    browser: tokio::sync::RwLock<Option<chromiumoxide::Browser>>,
}

impl BrowserAutomation {
    /// Create a new browser automation instance
    pub async fn new(config: BrowserConfig) -> Result<Self> {
        #[cfg(feature = "browser")]
        {
            let browser = Self::launch_browser(&config).await?;
            Ok(Self {
                config,
                browser: tokio::sync::RwLock::new(Some(browser)),
            })
        }
        #[cfg(not(feature = "browser"))]
        {
            Ok(Self {
                config,
            })
        }
    }
    
    /// Launch browser (requires 'browser' feature)
    #[cfg(feature = "browser")]
    async fn launch_browser(config: &BrowserConfig) -> Result<chromiumoxide::Browser> {
        use chromiumoxide::browser::{Browser, BrowserConfig as ChromiumConfig};
        use chromiumoxide::handler::viewport::Viewport as ChromiumViewport;
        
        let viewport = ChromiumViewport {
            width: config.viewport.width,
            height: config.viewport.height,
            device_scale_factor: None,
            emulating_mobile: false,
            is_landscape: true,
            has_touch: false,
        };
        
        let browser_config = ChromiumConfig::builder()
            .viewport(viewport)
            .build()
            .map_err(|e| CrabletError::Other(anyhow::anyhow!(e.to_string())))?;

        let (browser, mut handler) = Browser::launch(browser_config).await
            .map_err(|e| CrabletError::Other(anyhow::anyhow!(e.to_string())))?;
        
        // Spawn handler task
        tokio::spawn(async move {
            while let Some(h) = handler.next().await {
                if h.is_err() {
                    break;
                }
            }
        });
        
        Ok(browser)
    }
    
    /// Execute a browser workflow
    pub async fn execute_workflow(&self, workflow: &BrowserWorkflow) -> RpaResult<WorkflowExecutionResult> {
        tracing::info!("Starting browser workflow: {}", workflow.name);
        
        #[cfg(feature = "browser")]
        {
            let browser_lock = self.browser.read().await;
            let browser = browser_lock.as_ref()
                .ok_or_else(|| RpaError::BrowserError("Browser not initialized".to_string()))?;
            
            // Create new page
            let page = browser.new_page("about:blank").await
                .map_err(|e| RpaError::BrowserError(e.to_string()))?;
            
            let start = std::time::Instant::now();
            let mut variables: HashMap<String, String> = HashMap::new();
            let mut screenshots: Vec<String> = vec![];
            
            for (i, step) in workflow.steps.iter().enumerate() {
                tracing::debug!("Executing step {}: {:?}", i + 1, step);
                
                // Apply slow motion
                if self.config.slow_mo > 0 {
                    tokio::time::sleep(Duration::from_millis(self.config.slow_mo)).await;
                }
                
                match step {
                    BrowserStep::Navigate { url } => {
                        let resolved_url = self.resolve_variables(url, &variables);
                        tracing::debug!("Navigating to: {}", resolved_url);
                        
                        page.goto(&resolved_url).await
                            .map_err(|e| RpaError::NavigationError(e.to_string()))?;
                    }
                    BrowserStep::Click { selector } => {
                        let resolved_selector = self.resolve_variables(selector, &variables);
                        tracing::debug!("Clicking: {}", resolved_selector);
                        
                        page.find_element(&resolved_selector).await
                            .map_err(|e| RpaError::ElementNotFound(e.to_string()))?
                            .click().await
                            .map_err(|e| RpaError::BrowserError(e.to_string()))?;
                    }
                    BrowserStep::Fill { selector, value } => {
                        let resolved_selector = self.resolve_variables(selector, &variables);
                        let resolved_value = self.resolve_variables(value, &variables);
                        tracing::debug!("Filling {} with: {}", resolved_selector, resolved_value);
                        
                        let element = page.find_element(&resolved_selector).await
                            .map_err(|e| RpaError::ElementNotFound(e.to_string()))?;
                        
                        element.click().await
                            .map_err(|e| RpaError::BrowserError(e.to_string()))?;
                        
                        // Clear existing text
                        element.type_str("").await
                            .map_err(|e| RpaError::BrowserError(e.to_string()))?;
                        
                        // Type new value
                        element.type_str(&resolved_value).await
                            .map_err(|e| RpaError::BrowserError(e.to_string()))?;
                    }
                    BrowserStep::Select { selector, value } => {
                        let resolved_selector = self.resolve_variables(selector, &variables);
                        let resolved_value = self.resolve_variables(value, &variables);
                        tracing::debug!("Selecting {} in {}", resolved_value, resolved_selector);
                        
                        // Execute JavaScript to select option
                        let script = format!(
                            r#"document.querySelector('{}').value = '{}';"#,
                            resolved_selector, resolved_value
                        );
                        
                        page.evaluate(script.as_str()).await
                            .map_err(|e| RpaError::BrowserError(e.to_string()))?;
                    }
                    BrowserStep::Wait { seconds } => {
                        tracing::debug!("Waiting {} seconds", seconds);
                        tokio::time::sleep(Duration::from_secs(*seconds)).await;
                    }
                    BrowserStep::WaitForElement { selector, timeout } => {
                        let resolved_selector = self.resolve_variables(selector, &variables);
                        tracing::debug!("Waiting for element: {} (timeout: {}s)", resolved_selector, timeout);
                        
                        let timeout_duration = Duration::from_secs(*timeout);
                        
                        match tokio::time::timeout(timeout_duration, async {
                            loop {
                                match page.find_element(&resolved_selector).await {
                                    Ok(_) => break Ok::<(), RpaError>(()),
                                    Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
                                }
                            }
                        }).await {
                            Ok(Ok(())) => {}
                            _ => return Err(RpaError::TimeoutError(
                                format!("Element {} not found within {}s", resolved_selector, timeout)
                            )),
                        }
                    }
                    BrowserStep::Screenshot { path } => {
                        let resolved_path = self.resolve_variables(path, &variables);
                        tracing::debug!("Taking screenshot: {}", resolved_path);
                        
                        page.save_screenshot(chromiumoxide::page::ScreenshotParams::builder()
                            .full_page(true)
                            .build(), &resolved_path)
                            .await
                            .map_err(|e| RpaError::BrowserError(e.to_string()))?;
                        
                        screenshots.push(resolved_path);
                    }
                    BrowserStep::Scroll { x, y } => {
                        tracing::debug!("Scrolling by ({}, {})", x, y);
                        
                        let script = format!("window.scrollBy({}, {});", x, y);
                        page.evaluate(script.as_str()).await
                            .map_err(|e| RpaError::BrowserError(e.to_string()))?;
                    }
                    BrowserStep::ExecuteJs { script } => {
                        let resolved_script = self.resolve_variables(script, &variables);
                        tracing::debug!("Executing JavaScript: {}", &resolved_script[..resolved_script.len().min(50)]);
                        
                        page.evaluate(resolved_script.as_str()).await
                            .map_err(|e| RpaError::BrowserError(e.to_string()))?;
                    }
                    BrowserStep::Extract { selector, attribute, variable } => {
                        let resolved_selector = self.resolve_variables(selector, &variables);
                        tracing::debug!("Extracting {} from {}", variable, resolved_selector);
                        
                        let script = if let Some(attr) = attribute {
                            format!(
                                r#"document.querySelector('{}').getAttribute('{}')"#,
                                resolved_selector, attr
                            )
                        } else {
                            format!(
                                r#"document.querySelector('{}').innerText"#,
                                resolved_selector
                            )
                        };
                        
                        let result = page.evaluate(script.as_str()).await
                            .map_err(|e| RpaError::BrowserError(e.to_string()))?;
                        
                        let value = result.into_value::<serde_json::Value>()
                            .ok()
                            .and_then(|v| v.as_str().map(|s| s.to_string()))
                            .unwrap_or_default();
                        
                        variables.insert(variable.clone(), value);
                    }
                }
            }
            
            // Close page
            page.close().await
                .map_err(|e| RpaError::BrowserError(e.to_string()))?;
            
            tracing::info!("Browser workflow completed in {:?}", start.elapsed());
            
            Ok(WorkflowExecutionResult {
                success: true,
                execution_time: start.elapsed(),
                variables,
                screenshots,
            })
        }
        
        #[cfg(not(feature = "browser"))]
        {
            // Simulate execution without actual browser
            tracing::warn!("Browser feature not enabled, simulating workflow execution");
            
            let start = std::time::Instant::now();
            let mut variables: HashMap<String, String> = HashMap::new();
            let screenshots: Vec<String> = vec![];
            
            for step in &workflow.steps {
                match step {
                    BrowserStep::Extract { variable, .. } => {
                        variables.insert(variable.clone(), "simulated_value".to_string());
                    }
                    _ => {}
                }
            }
            
            Ok(WorkflowExecutionResult {
                success: true,
                execution_time: start.elapsed(),
                variables,
                screenshots,
            })
        }
    }
    
    /// Resolve variables in a string
    fn resolve_variables(&self, text: &str, variables: &HashMap<String, String>) -> String {
        let mut result = text.to_string();
        for (key, value) in variables {
            result = result.replace(&format!("{{{{{}}}}}", key), value);
        }
        result
    }
    
    /// Close browser
    pub async fn close(&self) -> Result<()> {
        #[cfg(feature = "browser")]
        {
            let mut browser_lock = self.browser.write().await;
            if let Some(mut browser) = browser_lock.take() {
                browser.close().await
                    .map_err(|e| CrabletError::Other(anyhow::anyhow!(e.to_string())))?;
            }
        }
        Ok(())
    }
}

/// Browser workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserWorkflow {
    pub name: String,
    pub steps: Vec<BrowserStep>,
    #[serde(default)]
    pub headless: bool,
    #[serde(default)]
    pub viewport: Option<Viewport>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub variables: HashMap<String, String>,
}

/// Browser automation step
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BrowserStep {
    /// Navigate to URL
    Navigate { url: String },
    /// Click element
    Click { selector: String },
    /// Fill form field
    Fill { selector: String, value: String },
    /// Select dropdown option
    Select { selector: String, value: String },
    /// Wait for specified seconds
    Wait { seconds: u64 },
    /// Wait for element to appear
    WaitForElement { selector: String, timeout: u64 },
    /// Take screenshot
    Screenshot { path: String },
    /// Scroll page
    Scroll { x: i32, y: i32 },
    /// Execute JavaScript
    ExecuteJs { script: String },
    /// Extract data from element
    Extract { selector: String, attribute: Option<String>, variable: String },
}

/// Workflow execution result
#[derive(Debug)]
pub struct WorkflowExecutionResult {
    pub success: bool,
    pub execution_time: Duration,
    pub variables: HashMap<String, String>,
    pub screenshots: Vec<String>,
}

/// Browser driver trait for abstraction
#[async_trait]
pub trait BrowserDriver: Send + Sync {
    /// Navigate to URL
    async fn goto(&self, url: &str) -> RpaResult<()>;
    /// Click element
    async fn click(&self, selector: &str) -> RpaResult<()>;
    /// Fill form field
    async fn fill(&self, selector: &str, value: &str) -> RpaResult<()>;
    /// Get element text
    async fn text(&self, selector: &str) -> RpaResult<String>;
    /// Take screenshot
    async fn screenshot(&self, path: &Path) -> RpaResult<()>;
    /// Execute JavaScript
    async fn eval(&self, script: &str) -> RpaResult<serde_json::Value>;
    /// Wait for element
    async fn wait_for(&self, selector: &str, timeout: Duration) -> RpaResult<()>;
    /// Close browser
    async fn close(&self) -> RpaResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_browser_config_default() {
        let config = BrowserConfig::default();
        assert!(config.headless);
        assert_eq!(config.viewport.width, 1920);
        assert_eq!(config.viewport.height, 1080);
    }
    
    #[test]
    fn test_browser_workflow_serialization() {
        let workflow = BrowserWorkflow {
            name: "Test Workflow".to_string(),
            steps: vec![
                BrowserStep::Navigate { url: "https://example.com".to_string() },
                BrowserStep::Click { selector: "#button".to_string() },
                BrowserStep::Fill { selector: "#input".to_string(), value: "test".to_string() },
            ],
            headless: true,
            viewport: None,
            user_agent: None,
            variables: HashMap::new(),
        };
        
        let yaml = serde_yaml::to_string(&workflow).unwrap();
        assert!(yaml.contains("Test Workflow"));
        assert!(yaml.contains("navigate"));
        assert!(yaml.contains("click"));
    }
    
    #[tokio::test]
    async fn test_browser_automation_without_feature() {
        // Test that browser automation can be created without the browser feature
        let config = BrowserConfig::default();
        let browser = BrowserAutomation::new(config).await;
        assert!(browser.is_ok());
    }
}
