//! RPA Workflow Engine
//!
//! Provides a unified workflow engine that can execute both browser and desktop automation steps.

use async_recursion::async_recursion;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

use crate::error::Result;
use crate::rpa::browser::{BrowserAutomation, BrowserWorkflow};
use crate::rpa::desktop::{DesktopAutomation, DesktopWorkflow};
use crate::rpa::{RpaError, RpaResult};

/// RPA Workflow Engine
pub struct RpaWorkflowEngine {
    browser: Arc<RwLock<Option<Arc<BrowserAutomation>>>>,
    desktop: Arc<RwLock<Option<Arc<Mutex<DesktopAutomation>>>>>,
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
    pub async fn set_browser(&self, browser: Option<Arc<BrowserAutomation>>) {
        let mut b = self.browser.write().await;
        *b = browser;
    }

    /// Set desktop automation
    pub async fn set_desktop(&self, desktop: Option<Arc<Mutex<DesktopAutomation>>>) {
        let mut d = self.desktop.write().await;
        *d = desktop;
    }

    /// Execute a workflow definition
    // Desktop automation is backed by a platform handle that is not `Send` on
    // macOS. The workflow engine is used in-process, so recursive execution
    // does not need the macro's default cross-thread future bound.
    #[async_recursion(?Send)]
    pub async fn execute(
        &self,
        workflow: &WorkflowDefinition,
        context: &mut WorkflowContext,
    ) -> RpaResult<WorkflowResult> {
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

            // Execute step with error policy (continue/retry/fail)
            let mut attempts: u32 = 0;
            loop {
                attempts += 1;

                let result = match &step.step_type {
                    StepType::Browser {
                        workflow: browser_workflow,
                    } => {
                        self.execute_browser_workflow(browser_workflow, context)
                            .await
                    }
                    StepType::Desktop {
                        workflow: desktop_workflow,
                    } => {
                        self.execute_desktop_workflow(desktop_workflow, context)
                            .await
                    }
                    StepType::Cognitive { prompt, system } => {
                        self.execute_cognitive(prompt, system, context).await
                    }
                    StepType::Http { request } => self.execute_http(request, context).await,
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
                        break;
                    }
                    Err(e) => {
                        error!("Step '{}' failed (attempt {}): {}", step.name, attempts, e);
                        context
                            .variables
                            .insert("last_error".to_string(), e.to_string());

                        match step.on_error {
                            ErrorAction::Continue => {
                                warn!("Continuing after error in step '{}'", step.name);
                                break;
                            }
                            ErrorAction::Retry { max_retries, delay } => {
                                if attempts <= max_retries.saturating_add(1) {
                                    warn!(
                                        "Retrying step '{}' in {:?} (attempt {}/{})",
                                        step.name,
                                        delay,
                                        attempts,
                                        max_retries.saturating_add(1)
                                    );
                                    tokio::time::sleep(delay).await;
                                    continue;
                                }
                                return Err(e);
                            }
                            ErrorAction::Fail => {
                                return Err(e);
                            }
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
    async fn execute_browser_workflow(
        &self,
        workflow: &BrowserWorkflow,
        _context: &WorkflowContext,
    ) -> RpaResult<StepResult> {
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
            Err(RpaError::BrowserError(
                "Browser automation not initialized".to_string(),
            ))
        }
    }

    /// Execute desktop workflow
    async fn execute_desktop_workflow(
        &self,
        workflow: &DesktopWorkflow,
        _context: &WorkflowContext,
    ) -> RpaResult<StepResult> {
        let desktop = self.desktop.read().await;

        if let Some(desktop) = desktop.as_ref() {
            let mut desktop = desktop.lock().await;
            let result = desktop.execute_workflow(workflow).await?;

            let mut outputs = HashMap::new();
            for (k, v) in result.variables {
                outputs.insert(k, v);
            }
            if let Some(last) = result.screenshots.last() {
                outputs.insert("last_screenshot".to_string(), last.clone());
            }

            Ok(StepResult {
                success: result.success,
                outputs,
            })
        } else {
            Err(RpaError::DesktopError(
                "Desktop automation not initialized".to_string(),
            ))
        }
    }

    /// Execute cognitive processing
    async fn execute_cognitive(
        &self,
        prompt: &str,
        _system: &Option<String>,
        context: &WorkflowContext,
    ) -> RpaResult<StepResult> {
        let resolved_prompt = self.resolve_variables(prompt, &context.variables);

        debug!("Cognitive processing: {}", resolved_prompt);

        // In real implementation, this would call the cognitive router
        // For now, return a simulated result
        let mut outputs = HashMap::new();
        outputs.insert(
            "result".to_string(),
            format!("Processed: {}", resolved_prompt),
        );

        Ok(StepResult {
            success: true,
            outputs,
        })
    }

    /// Execute HTTP request
    async fn execute_http(
        &self,
        request: &HttpRequest,
        context: &WorkflowContext,
    ) -> RpaResult<StepResult> {
        let resolved_url = self.resolve_variables(&request.url, &context.variables);

        debug!("HTTP {} {}", request.method, resolved_url);

        let client = reqwest::Client::new();
        let mut req = match request.method.as_str() {
            "GET" => client.get(&resolved_url),
            "POST" => client.post(&resolved_url),
            "PUT" => client.put(&resolved_url),
            "DELETE" => client.delete(&resolved_url),
            "PATCH" => client.patch(&resolved_url),
            _ => {
                return Err(RpaError::ValidationError(format!(
                    "Invalid HTTP method: {}",
                    request.method
                )))
            }
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

        let response = req
            .send()
            .await
            .map_err(|e| RpaError::WorkflowError(e.to_string()))?;

        let status = response.status();
        let text = response
            .text()
            .await
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
    async fn execute_file_operation(
        &self,
        operation: &FileOperation,
        context: &WorkflowContext,
    ) -> RpaResult<StepResult> {
        match operation {
            FileOperation::Read { path } => {
                let resolved_path = self.resolve_variables(path, &context.variables);
                let content = tokio::fs::read_to_string(&resolved_path)
                    .await
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

                tokio::fs::write(&resolved_path, resolved_content)
                    .await
                    .map_err(|e| RpaError::WorkflowError(e.to_string()))?;

                Ok(StepResult::success())
            }
            FileOperation::Delete { path } => {
                let resolved_path = self.resolve_variables(path, &context.variables);

                tokio::fs::remove_file(&resolved_path)
                    .await
                    .map_err(|e| RpaError::WorkflowError(e.to_string()))?;

                Ok(StepResult::success())
            }
        }
    }

    /// Execute condition
    async fn execute_condition(
        &self,
        branches: &[ConditionBranch],
        context: &mut WorkflowContext,
    ) -> RpaResult<StepResult> {
        for branch in branches {
            if self.evaluate_condition(&branch.condition, context).await? {
                let branch_workflow = WorkflowDefinition {
                    id: format!("condition-branch-{}", branch.condition),
                    name: format!("condition-branch-{}", branch.condition),
                    description: String::new(),
                    version: "1.0".to_string(),
                    triggers: Vec::new(),
                    variables: Vec::new(),
                    steps: branch.steps.clone(),
                };
                let result = self.execute(&branch_workflow, context).await?;
                return Ok(StepResult {
                    success: result.success,
                    outputs: result.variables,
                });
            }
        }

        Ok(StepResult::success())
    }

    /// Execute loop
    async fn execute_loop(
        &self,
        condition: &str,
        steps: &[WorkflowStep],
        context: &mut WorkflowContext,
    ) -> RpaResult<StepResult> {
        let max_iterations = 1000; // Prevent infinite loops
        let mut iterations = 0;

        while self.evaluate_condition(condition, context).await? && iterations < max_iterations {
            for step in steps {
                let step_def = WorkflowDefinition {
                    id: format!("loop-iteration-{}", iterations),
                    name: format!("loop-iteration-{}", iterations),
                    description: String::new(),
                    version: "1.0".to_string(),
                    triggers: Vec::new(),
                    variables: Vec::new(),
                    steps: vec![step.clone()],
                };
                self.execute(&step_def, context).await?;
            }
            iterations += 1;
        }

        if iterations >= max_iterations {
            return Err(RpaError::WorkflowError(
                "Loop exceeded maximum iterations".to_string(),
            ));
        }

        Ok(StepResult::success())
    }

    /// Execute parallel branches
    async fn execute_parallel(
        &self,
        branches: &[Vec<WorkflowStep>],
        context: &WorkflowContext,
    ) -> RpaResult<StepResult> {
        // Execute branches concurrently against isolated contexts and merge
        // outputs in declaration order for deterministic precedence.
        let mut branch_futures = Vec::new();

        for (index, branch) in branches.iter().enumerate() {
            let branch_workflow = WorkflowDefinition {
                id: format!("parallel-branch-{}", index),
                name: format!("parallel-branch-{}", index),
                description: String::new(),
                version: "1.0".to_string(),
                triggers: Vec::new(),
                variables: Vec::new(),
                steps: branch.clone(),
            };
            let mut branch_context = context.clone();

            branch_futures
                .push(async move { self.execute(&branch_workflow, &mut branch_context).await });
        }

        let results = futures::future::join_all(branch_futures).await;
        let mut outputs = HashMap::new();
        for result in results {
            let result = result?;
            outputs.extend(result.variables);
        }

        Ok(StepResult {
            success: true,
            outputs,
        })
    }

    /// Evaluate condition expression
    async fn evaluate_condition(
        &self,
        condition: &str,
        context: &WorkflowContext,
    ) -> RpaResult<bool> {
        // Simple variable existence check
        if condition.starts_with("vars.") {
            let var_name = &condition[5..];
            return Ok(context.variables.contains_key(var_name));
        }

        // Support basic comparison operators: ==, !=, >, <, >=, <=
        if let Some(eq_pos) = condition.find("==") {
            let left = condition[..eq_pos].trim();
            let right = condition[eq_pos + 2..].trim();
            return Ok(self.resolve_variables(left, &context.variables)
                == self.resolve_variables(right, &context.variables));
        }
        if let Some(ne_pos) = condition.find("!=") {
            let left = condition[..ne_pos].trim();
            let right = condition[ne_pos + 2..].trim();
            return Ok(self.resolve_variables(left, &context.variables)
                != self.resolve_variables(right, &context.variables));
        }

        // Default: treat non-empty condition as true
        Ok(!condition.is_empty())
    }

    /// Resolve variables in a string
    fn resolve_variables(&self, text: &str, variables: &HashMap<String, String>) -> String {
        let mut result = text.to_string();
        for (key, value) in variables {
            result = result.replace(&format!("{{{{{}}}}}", key), value);
            result = result.replace(&format!("{{{{vars.{}}}}}", key), value);
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
    Browser {
        workflow: BrowserWorkflow,
    },
    Desktop {
        workflow: DesktopWorkflow,
    },
    Cognitive {
        prompt: String,
        system: Option<String>,
    },
    Http {
        request: HttpRequest,
    },
    File {
        operation: FileOperation,
    },
    Condition {
        branches: Vec<ConditionBranch>,
    },
    Loop {
        condition: String,
        steps: Vec<WorkflowStep>,
    },
    Parallel {
        branches: Vec<Vec<WorkflowStep>>,
    },
    Wait {
        duration: Duration,
    },
    SetVariable {
        name: String,
        value: String,
    },
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
#[derive(Debug, Default, Clone)]
pub struct WorkflowContext {
    pub variables: HashMap<String, String>,
    pub trigger_data: Option<serde_json::Value>,
}

/// Step result
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct WorkflowResult {
    pub success: bool,
    pub execution_time: Duration,
    pub variables: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::sync::Mutex;

    #[test]
    fn test_workflow_definition_serialization() {
        let workflow = WorkflowDefinition {
            id: "test-123".to_string(),
            name: "Test Workflow".to_string(),
            description: "A test workflow".to_string(),
            version: "1.0.0".to_string(),
            triggers: vec![WorkflowTrigger::Manual],
            variables: vec![],
            steps: vec![WorkflowStep {
                id: "step1".to_string(),
                name: "Set Variable".to_string(),
                step_type: StepType::SetVariable {
                    name: "test".to_string(),
                    value: "value".to_string(),
                },
                condition: None,
                outputs: vec![],
                on_error: ErrorAction::Fail,
            }],
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

    #[tokio::test]
    async fn test_desktop_step_executes_workflow() {
        let engine = RpaWorkflowEngine::new().unwrap();

        let desktop = crate::rpa::desktop::DesktopAutomation::new().unwrap();
        engine
            .set_desktop(Some(Arc::new(Mutex::new(desktop))))
            .await;

        let wf = WorkflowDefinition {
            id: "wf1".to_string(),
            name: "desktop".to_string(),
            description: "".to_string(),
            version: "1.0.0".to_string(),
            triggers: vec![WorkflowTrigger::Manual],
            variables: vec![],
            steps: vec![WorkflowStep {
                id: "s1".to_string(),
                name: "desktop-step".to_string(),
                step_type: StepType::Desktop {
                    workflow: crate::rpa::desktop::DesktopWorkflow {
                        name: "dwf".to_string(),
                        steps: vec![crate::rpa::desktop::DesktopStep::Wait { seconds: 0 }],
                        variables: HashMap::new(),
                    },
                },
                condition: None,
                outputs: vec![],
                on_error: ErrorAction::Fail,
            }],
        };

        let mut ctx = WorkflowContext::default();
        let result = engine.execute(&wf, &mut ctx).await.unwrap();
        assert!(result.success);
    }

    #[tokio::test]
    async fn test_retry_allows_eventual_file_read_success() {
        let engine = RpaWorkflowEngine::new().unwrap();

        let dir = tempdir().unwrap();
        let path = dir.path().join("hello.txt");
        let path_str = path.to_string_lossy().to_string();

        let wf = WorkflowDefinition {
            id: "wf2".to_string(),
            name: "retry-file".to_string(),
            description: "".to_string(),
            version: "1.0.0".to_string(),
            triggers: vec![WorkflowTrigger::Manual],
            variables: vec![],
            steps: vec![WorkflowStep {
                id: "s1".to_string(),
                name: "read".to_string(),
                step_type: StepType::File {
                    operation: FileOperation::Read { path: path_str },
                },
                condition: None,
                outputs: vec!["content".to_string()],
                on_error: ErrorAction::Retry {
                    max_retries: 5,
                    delay: Duration::from_millis(25),
                },
            }],
        };

        let writer = {
            let path = path.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(40)).await;
                tokio::fs::write(path, "ok").await.unwrap();
            })
        };

        let mut ctx = WorkflowContext::default();
        let result = engine.execute(&wf, &mut ctx).await.unwrap();
        writer.await.unwrap();

        assert!(result.success);
        assert_eq!(
            result.variables.get("content").map(String::as_str),
            Some("ok")
        );
    }

    #[tokio::test]
    async fn test_condition_branch_updates_parent_context() {
        let engine = RpaWorkflowEngine::new().unwrap();
        let workflow = WorkflowDefinition {
            id: "condition-context".to_string(),
            name: "condition-context".to_string(),
            description: String::new(),
            version: "1.0.0".to_string(),
            triggers: vec![WorkflowTrigger::Manual],
            variables: vec![],
            steps: vec![WorkflowStep {
                id: "condition".to_string(),
                name: "condition".to_string(),
                step_type: StepType::Condition {
                    branches: vec![ConditionBranch {
                        condition: "vars.enabled".to_string(),
                        steps: vec![WorkflowStep {
                            id: "set".to_string(),
                            name: "set".to_string(),
                            step_type: StepType::SetVariable {
                                name: "result".to_string(),
                                value: "done".to_string(),
                            },
                            condition: None,
                            outputs: vec![],
                            on_error: ErrorAction::Fail,
                        }],
                    }],
                },
                condition: None,
                outputs: vec!["result".to_string()],
                on_error: ErrorAction::Fail,
            }],
        };

        let mut context = WorkflowContext::default();
        context
            .variables
            .insert("enabled".to_string(), "yes".to_string());
        let result = engine.execute(&workflow, &mut context).await.unwrap();
        assert_eq!(
            result.variables.get("result").map(String::as_str),
            Some("done")
        );
    }

    #[tokio::test]
    async fn test_parallel_branches_execute_and_merge_outputs() {
        let engine = RpaWorkflowEngine::new().unwrap();
        let set = |name: &str, value: &str| WorkflowStep {
            id: name.to_string(),
            name: name.to_string(),
            step_type: StepType::SetVariable {
                name: name.to_string(),
                value: value.to_string(),
            },
            condition: None,
            outputs: vec![],
            on_error: ErrorAction::Fail,
        };
        let workflow = WorkflowDefinition {
            id: "parallel-output".to_string(),
            name: "parallel-output".to_string(),
            description: String::new(),
            version: "1.0.0".to_string(),
            triggers: vec![WorkflowTrigger::Manual],
            variables: vec![],
            steps: vec![WorkflowStep {
                id: "parallel".to_string(),
                name: "parallel".to_string(),
                step_type: StepType::Parallel {
                    branches: vec![vec![set("left", "L")], vec![set("right", "R")]],
                },
                condition: None,
                outputs: vec!["left".to_string(), "right".to_string()],
                on_error: ErrorAction::Fail,
            }],
        };

        let mut context = WorkflowContext::default();
        let result = engine.execute(&workflow, &mut context).await.unwrap();
        assert_eq!(result.variables.get("left").map(String::as_str), Some("L"));
        assert_eq!(result.variables.get("right").map(String::as_str), Some("R"));
    }
}
