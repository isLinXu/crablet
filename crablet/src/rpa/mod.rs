//! RPA (Robotic Process Automation) Module
//!
//! This module provides browser and desktop automation capabilities for Crablet.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                        RPA System                                │
//! │                                                                  │
//! │   ┌─────────────────────┐    ┌─────────────────────┐           │
//! │   │   Browser           │    │   Desktop           │           │
//! │   │   Automation        │    │   Automation        │           │
//! │   │                     │    │                     │           │
//! │   │ • Playwright/CDP    │    │ • Mouse/Keyboard    │           │
//! │   │ • Form filling      │    │ • Screenshots       │           │
//! │   │ • Data extraction   │    │ • Window mgmt       │           │
//! │   │ • Screenshot        │    │ • Clipboard         │           │
//! │   └──────────┬──────────┘    └──────────┬──────────┘           │
//! │              │                          │                      │
//! │              └────────────┬─────────────┘                      │
//! │                           │                                    │
//! │                           ▼                                    │
//! │              ┌─────────────────────┐                           │
//! │              │   Workflow Engine   │                           │
//! │              │   • Visual editor   │                           │
//! │              │   • Step execution  │                           │
//! │              │   • Variable system │                           │
//! │              └─────────────────────┘                           │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

pub mod browser;
pub mod desktop;
pub mod workflow;

pub use browser::{BrowserAutomation, BrowserConfig, BrowserWorkflow, BrowserStep};
pub use desktop::{DesktopAutomation, DesktopWorkflow, DesktopStep};
pub use workflow::{RpaWorkflowEngine, WorkflowDefinition, WorkflowStep};

use std::sync::Arc;
use crate::error::Result;

/// RPA system coordinator
pub struct RpaSystem {
    browser: Option<Arc<BrowserAutomation>>,
    desktop: Option<Arc<DesktopAutomation>>,
    workflow_engine: Arc<RpaWorkflowEngine>,
}

impl RpaSystem {
    /// Create a new RPA system
    pub async fn new() -> Result<Self> {
        let workflow_engine = Arc::new(RpaWorkflowEngine::new()?);
        
        Ok(Self {
            browser: None,
            desktop: None,
            workflow_engine,
        })
    }
    
    /// Initialize browser automation
    pub async fn init_browser(&mut self, config: browser::BrowserConfig) -> Result<()> {
        let browser = BrowserAutomation::new(config).await?;
        self.browser = Some(Arc::new(browser));
        self.workflow_engine.set_browser(self.browser.clone());
        Ok(())
    }
    
    /// Initialize desktop automation
    pub fn init_desktop(&mut self) -> Result<()> {
        let desktop = DesktopAutomation::new()
            .map_err(|e| crate::error::CrabletError::RpaError(e.to_string()))?;
        self.desktop = Some(Arc::new(desktop));
        self.workflow_engine.set_desktop(self.desktop.clone());
        Ok(())
    }
    
    /// Get browser automation
    pub fn browser(&self) -> Option<Arc<BrowserAutomation>> {
        self.browser.clone()
    }
    
    /// Get desktop automation
    pub fn desktop(&self) -> Option<Arc<DesktopAutomation>> {
        self.desktop.clone()
    }
    
    /// Get workflow engine
    pub fn workflow_engine(&self) -> Arc<RpaWorkflowEngine> {
        self.workflow_engine.clone()
    }
}

/// RPA error types
#[derive(Debug, thiserror::Error)]
pub enum RpaError {
    #[error("Browser automation error: {0}")]
    BrowserError(String),
    
    #[error("Desktop automation error: {0}")]
    DesktopError(String),
    
    #[error("Workflow error: {0}")]
    WorkflowError(String),
    
    #[error("Element not found: {0}")]
    ElementNotFound(String),
    
    #[error("Timeout error: {0}")]
    TimeoutError(String),
    
    #[error("Navigation error: {0}")]
    NavigationError(String),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
}

/// RPA result type
pub type RpaResult<T> = std::result::Result<T, RpaError>;
