//! RPA Workflow Engine
//!
//! Provides a unified workflow engine that can execute both browser and desktop automation steps.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::error::Result;
use crate::rpa::{RpaError, RpaResult};
use crate::rpa::browser::{BrowserAutomation, BrowserWorkflow};
use crate::rpa::desktop::{DesktopAutomation, DesktopWorkflow};

/// RPA Workflow Engine
pub struct RpaWorkflowEngine {
    browser: Arc<RwLock<Option<Arc<BrowserAutomation>>>>,
    desktop: Arc<RwLock<Option<Arc<DesktopAutomation>>>>,
}

impl RpaWorkflowEngine {
    /// Create a new workflow engine
    pub fn new() -> Result<Self> {
        Ok(Self {
            browser: Arc::new(RwLock::new(None)),
            desktop: Arc::new(RwLock::new(None)),
        })
    }
    
    /// Set browser automation
    pub fn set_browser(&self, browser: Option<Arc<BrowserAutomation>>) {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            let mut b = self.browser.write().await;
            *b = browser;
        });
    }
    
    /// Set desktop automation
    pub fn set_desktop(&self, desktop: Option<Arc<DesktopAutomation>>) {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            let mut d = self.desktop.write().await;
            *d = desktop;
        });
    }
    
    /// Execute a workflow definition
    pub async fn execute(&self, workflow: &WorkflowDefinition, context: &mut WorkflowContext) -> RpaResult<WorkflowResult> {
        info!("Executing workflow: {}", workflow.name);
        
        let start = std::time::Instant::now();
        
        for (i, step) in workflow.steps.iter().enumerate() {
            debug!("Executing step {}: {}", i + 1, step.name);
            
            // Check condition if present
            if let Some(condition) = &step.condition {
                if !self.evaluate_condition(condition, context).await? {
                    debug!("Condition not met, skipping step");
                    continue;
                }
            }
            
            // Execute step
            let result = match &step.step_type {
                StepType::Browser { workflow: browser_workflow } => {
                    self.execute_browser_workflow(browser_workflow, context).await
                }
                StepType::Desktop { workflow: desktop_workflow } => {
                    self.execute_desktop_workflow(desktop_workflow, context).await
                }
                StepType::Cognitive { prompt, system } => {
                    self.execute_cognitive(prompt, system, context).await
                }
                StepType::Http { request } => {
                    self.execute_http(request, context).await
                }
                StepType::File { operation } => {
                    self.execute_file_operation(operation, context).await
                }
                StepType::Condition { branches } => {
                    self.execute_condition(branches, context).await
                }
                StepType::Loop { condition, steps } => {
                    self.execute_loop(condition, steps, context).await
                }
                StepType::Parallel { branches } => {
                    self.execute_parallel(branches, context).await
                }
                StepType::Wait { duration } => {
                    tokio::time::sleep(*duration).await;
                    Ok(StepResult::success())
                }
                StepType::SetVariable { name, value } => {
                    let resolved_value = self.resolve_variables(value, &context.variables);
                    context.variables.insert(name.clone(), resolved_value);
                    Ok(StepResult::success())
                }
            };
            
            match result {
                Ok(step_result) => {
                    // Store outputs
                    for output in &step.outputs {
                        if let Some(value) = step_result.outputs.get(output) {
                            context.variables.insert(output.clone(), value.clone());
                        }
                    }
                }
                Err(e) => {
                    error!("Step {} failed: {}", step.name, e);
                    
                    // Check error handling
                    match &step.on_error {
                        ErrorAction::Continue => {
                            warn!("Continuing after error");
                        }
                        ErrorAction::Retry { max_retries: _, delay: _ } => {
                            // TODO: Implement retry logic
                            return Err(e);
                        }
                        ErrorAction::Fail => {
                            return Err(e);
                        }
                    }
                }
            }
        }
        
        info!("Workflow completed in {:?}", start.elapsed());
        
        Ok(WorkflowResult {
            success: true,
            execution_time: start.elapsed(),
            variables: context.variables.clone(),
        })
    }
    
    /// Execute browser workflow
    async fn execute_browser_workflow(&self, workflow: &BrowserWorkflow, _context: &WorkflowContext) -> RpaResult<StepResult> {
        let browser = self.browser.read().await;
        
        if let Some(browser) = browser.as_ref() {
            let result = browser.execute_workflow(workflow).await?;
            
            let mut outputs = HashMap::new();
            for (key, value) in result.variables {
                outputs.insert(key, value);
            }
            
            Ok(StepResult {
                success: result.success,
                outputs,
            })
        } else {
            Err(RpaError::BrowserError("Browser automation not initialized".to_string()))
        }
    }
    
    /// Execute desktop workflow
    async fn execute_desktop_workflow(&self, workflow: &DesktopWorkflow, _context: &WorkflowContext) -> RpaResult<StepResult> {
        let desktop = self.desktop.read().await;
        
        if let Some(_desktop) = desktop.as_ref() {
            // Clone the workflow to avoid borrowing issues
            let _workflow = workflow.clone();
            
            // We need to clone the desktop automation to execute
            // In real implementation, this would be handled differently
            // For now, return a simulated result
            Ok(StepResult::success())
        } else {
            Err(RpaError::DesktopError("Desktop automation not initialized".to_string()))
        }
    }
    
    /// Execute cognitive processing
    async fn execute_cognitive(&self, prompt: &str, _system: &Option<String>, context: &WorkflowContext) -> RpaResult<StepResult> {
        let resolved_prompt = self.resolve_variables(prompt, &context.variables);
        
        debug!("Cognitive processing: {}", resolved_prompt);
        
        // In real implementation, this would call the cognitive router
        // For now, return a simulated result
        let mut outputs = HashMap::new();
        outputs.insert("result".to_string(), format!("Processed: {}", resolved_prompt));
        
        Ok(StepResult {
            success: true,
            outputs,
        })
    }
    
    /// Execute HTTP request
    async fn execute_http(&self, request: &HttpRequest, context: &WorkflowContext) -> RpaResult<StepResult> {
        let resolved_url = self.resolve_variables(&request.url, &context.variables);
        
        debug!("HTTP {} {}", request.method, resolved_url);
        
        let client = reqwest::Client::new();
        let mut req = match request.method.as_str() {
            "GET" => client.get(&resolved_url),
            "POST" => client.post(&resolved_url),
            "PUT" => client.put(&resolved_url),
            "DELETE" => client.delete(&resolved_url),
            "PATCH" => client.patch(&resolved_url),
            _ => return Err(RpaError::ValidationError(format!("Invalid HTTP method: {}", request.method))),
        };
        
        // Add headers
        for (key, value) in &request.headers {
            let resolved_value = self.resolve_variables(value, &context.variables);
            req = req.header(key, resolved_value);
        }
        
        // Add body
        if let Some(body) = &request.body {
            let resolved_body = self.resolve_variables(body, &context.variables);
            req = req.body(resolved_body);
        }
        
        let response = req.send().await
            .map_err(|e| RpaError::WorkflowError(e.to_string()))?;
        
        let status = response.status();
        let text = response.text().await
            .map_err(|e| RpaError::WorkflowError(e.to_string()))?;
        
        let mut outputs = HashMap::new();
        outputs.insert("status".to_string(), status.as_u16().to_string());
        outputs.insert("body".to_string(), text);
        
        Ok(StepResult {
            success: status.is_success(),
            outputs,
        })
    }
    
    /// Execute file operation
    async fn execute_file_operation(&self, operation: &FileOperation, context: &WorkflowContext) -> RpaResult<StepResult> {
        match operation {
            FileOperation::Read { path } => {
                let resolved_path = self.resolve_variables(path, &context.variables);
                let content = tokio::fs::read_to_string(&resolved_path).await
                    .map_err(|e| RpaError::WorkflowError(e.to_string()))?;
                
                let mut outputs = HashMap::new();
                outputs.insert("content".to_string(), content);
                
                Ok(StepResult {
                    success: true,
                    outputs,
                })
            }
            FileOperation::Write { path, content } => {
                let resolved_path = self.resolve_variables(path, &context.variables);
                let resolved_content = self.resolve_variables(content, &context.variables);
                
                tokio::fs::write(&resolved_path, resolved_content).await
                    .map_err(|e| RpaError::WorkflowError(e.to_string()))?;
                
                Ok(StepResult::success())
            }
            FileOperation::Delete { path } => {
                let resolved_path = self.resolve_variables(path, &context.variables);
                
                tokio::fs::remove_file(&resolved_path).await
                    .map_err(|e| RpaError::WorkflowError(e.to_string()))?;
                
                Ok(StepResult::success())
            }
        }
    }
    
    /// Execute condition
    async fn execute_condition(&self, branches: &[ConditionBranch], context: &WorkflowContext) -> RpaResult<StepResult> {
        for branch in branches {
            if self.evaluate_condition(&branch.condition, context).await? {
                // Execute branch steps
                for _step in &branch.steps {
                    // TODO: Execute steps recursively
                }
                return Ok(StepResult::success());
            }
        }
        
        Ok(StepResult::success())
    }
    
    /// Execute loop
    async fn execute_loop(&self, condition: &str, steps: &[WorkflowStep], context: &mut WorkflowContext) -> RpaResult<StepResult> {
        let max_iterations = 1000; // Prevent infinite loops
        let mut iterations = 0;
        
        while self.evaluate_condition(condition, context).await? && iterations < max_iterations {
            for _step in steps {
                // TODO: Execute steps recursively
            }
            iterations += 1;
        }
        
        if iterations >= max_iterations {
            return Err(RpaError::WorkflowError("Loop exceeded maximum iterations".to_string()));
        }
        
        Ok(StepResult::success())
    }
    
    /// Execute parallel branches
    async fn execute_parallel(&self, branches: &[Vec<WorkflowStep>], _context: &WorkflowContext) -> RpaResult<StepResult> {
        let mut _handles: Vec<tokio::task::JoinHandle<()>> = vec![];
        
        for _branch in branches {
            // TODO: Execute branches in parallel
        }
        
        Ok(StepResult::success())
    }
    
    /// Evaluate condition expression
    async fn evaluate_condition(&self, condition: &str, context: &WorkflowContext) -> RpaResult<bool> {
        // Simple variable existence check
        if condition.starts_with("vars.") {
            let var_name = &condition[5..];
            return Ok(context.variables.contains_key(var_name));
        }
        
        // TODO: Implement proper expression evaluation
        Ok(true)
    }
    
    /// Resolve variables in a string
    fn resolve_variables(&self, text: &str, variables: &HashMap<String, String>) -> String {
        let mut result = text.to_string();
        for (key, value) in variables {
            result = result.replace(&format!("{{{{{}}}}}", key), value);
            result = result.replace(&format!("{{{{vars.{}}}}}" , key), value);
        }
        result
    }
}

/// Workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub triggers: Vec<WorkflowTrigger>,
    pub variables: Vec<VariableDefinition>,
    pub steps: Vec<WorkflowStep>,
}

/// Workflow trigger
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorkflowTrigger {
    Schedule { cron: String },
    Event { source: String, event_type: String },
    Webhook { path: String, method: String },
    Manual,
}

/// Variable definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDefinition {
    pub name: String,
    pub var_type: String,
    pub default: Option<String>,
    pub required: bool,
}

/// Workflow step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub name: String,
    #[serde(flatten)]
    pub step_type: StepType,
    pub condition: Option<String>,
    pub outputs: Vec<String>,
    #[serde(default)]
    pub on_error: ErrorAction,
}

/// Step type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepType {
    Browser { workflow: BrowserWorkflow },
    Desktop { workflow: DesktopWorkflow },
    Cognitive { prompt: String, system: Option<String> },
    Http { request: HttpRequest },
    File { operation: FileOperation },
    Condition { branches: Vec<ConditionBranch> },
    Loop { condition: String, steps: Vec<WorkflowStep> },
    Parallel { branches: Vec<Vec<WorkflowStep>> },
    Wait { duration: Duration },
    SetVariable { name: String, value: String },
}

/// HTTP request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequest {
    pub url: String,
    pub method: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

/// File operation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum FileOperation {
    Read { path: String },
    Write { path: String, content: String },
    Delete { path: String },
}

/// Condition branch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionBranch {
    pub condition: String,
    pub steps: Vec<WorkflowStep>,
}

/// Error action
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorAction {
    Continue,
    Retry { max_retries: u32, delay: Duration },
    Fail,
}

impl Default for ErrorAction {
    fn default() -> Self {
        ErrorAction::Fail
    }
}

/// Workflow context
#[derive(Debug, Default)]
pub struct WorkflowContext {
    pub variables: HashMap<String, String>,
    pub trigger_data: Option<serde_json::Value>,
}

/// Step result
#[derive(Debug)]
pub struct StepResult {
    pub success: bool,
    pub outputs: HashMap<String, String>,
}

impl StepResult {
    /// Create a successful result
    pub fn success() -> Self {
        Self {
            success: true,
            outputs: HashMap::new(),
        }
    }
}

/// Workflow result
#[derive(Debug)]
pub struct WorkflowResult {
    pub success: bool,
    pub execution_time: Duration,
    pub variables: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_workflow_definition_serialization() {
        let workflow = WorkflowDefinition {
            id: "test-123".to_string(),
            name: "Test Workflow".to_string(),
            description: "A test workflow".to_string(),
            version: "1.0.0".to_string(),
            triggers: vec![WorkflowTrigger::Manual],
            variables: vec![],
            steps: vec![
                WorkflowStep {
                    id: "step1".to_string(),
                    name: "Set Variable".to_string(),
                    step_type: StepType::SetVariable {
                        name: "test".to_string(),
                        value: "value".to_string(),
                    },
                    condition: None,
                    outputs: vec![],
                    on_error: ErrorAction::Fail,
                }
            ],
        };
        
        let yaml = serde_yaml::to_string(&workflow).unwrap();
        assert!(yaml.contains("Test Workflow"));
        assert!(yaml.contains("set_variable"));
    }
    
    #[test]
    fn test_http_request_serialization() {
        let request = HttpRequest {
            url: "https://api.example.com/data".to_string(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            body: None,
        };
        
        let yaml = serde_yaml::to_string(&request).unwrap();
        assert!(yaml.contains("https://api.example.com/data"));
        assert!(yaml.contains("GET"));
    }
}
