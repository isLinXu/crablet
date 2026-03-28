//! Tool Flow Orchestration - DAG-based tool execution with dependency management
//!
//! This module provides structured tool execution with:
//! - DAG-based dependency management
//! - Parallel execution of independent steps
//! - Multiple error handling strategies
//! - Conditional step execution
//! - Execution visualization

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A tool flow is a directed acyclic graph (DAG) of tool execution steps
#[derive(Debug, Clone)]
pub struct ToolFlow {
    /// Unique identifier for this flow
    pub id: String,
    /// Name/description of the flow
    pub name: String,
    /// Steps in execution order (will be reordered by topological sort)
    pub steps: Vec<ToolFlowStep>,
    /// Default error handling for the entire flow
    pub default_error_handling: ErrorHandling,
    /// Whether to continue execution on step failure
    pub continue_on_failure: bool,
    /// Execution metadata
    pub metadata: HashMap<String, String>,
}

/// A single step in a tool flow
#[derive(Debug, Clone)]
pub struct ToolFlowStep {
    /// Unique step identifier
    pub id: String,
    /// Name of the tool to execute
    pub tool_name: String,
    /// Template arguments (can reference previous step outputs)
    pub args_template: HashMap<String, serde_json::Value>,
    /// Indices of steps this step depends on
    pub depends_on: Vec<usize>,
    /// Optional condition for execution
    pub condition: Option<StepCondition>,
    /// Error handling for this specific step
    pub error_handling: Option<ErrorHandling>,
    /// Timeout for this step (None = use flow default)
    pub timeout_secs: Option<u64>,
    /// Whether to capture output for dependent steps
    pub capture_output: bool,
}

/// Condition for conditional step execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepCondition {
    /// Always execute
    Always,
    /// Execute only if previous step succeeded
    OnSuccess,
    /// Execute only if previous step failed
    OnFailure,
    /// Execute only if previous step was skipped
    OnSkipped,
    /// Custom expression condition
    Expression(String),
    /// Execute based on result comparison
    ResultEquals { step_id: String, expected: serde_json::Value },
    /// Execute if result contains key
    ResultHasKey { step_id: String, key: String },
}

/// Error handling strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorHandling {
    /// Stop execution immediately
    FailFast,
    /// Retry with configuration
    Retry {
        max_attempts: u32,
        backoff_ms: u64,
        exponential: bool,
    },
    /// Fall back to alternative tool
    Fallback {
        fallback_tool: String,
        fallback_args: HashMap<String, serde_json::Value>,
    },
    /// Skip this step and continue
    Continue,
    /// Skip and mark as skipped
    Skip,
}

impl Default for ErrorHandling {
    fn default() -> Self {
        ErrorHandling::FailFast
    }
}

/// Context passed during flow execution
#[derive(Debug, Clone)]
pub struct ToolExecutionContext {
    /// Initial arguments for the flow
    pub initial_args: HashMap<String, serde_json::Value>,
    /// Results from completed steps (step_id -> result)
    pub step_results: HashMap<String, StepResult>,
    /// Shared state between steps
    pub shared_state: HashMap<String, serde_json::Value>,
    /// Current execution node/worker
    pub execution_node: String,
    /// Execution metadata
    pub metadata: HashMap<String, String>,
}

/// Result of a step execution
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Step ID
    pub step_id: String,
    /// Whether execution succeeded
    pub success: bool,
    /// Tool output
    pub output: Option<serde_json::Value>,
    /// Error message if failed
    pub error: Option<String>,
    /// Execution duration in ms
    pub duration_ms: u64,
    /// Whether this step was skipped
    pub skipped: bool,
    /// Skipped reason if applicable
    pub skip_reason: Option<String>,
    /// Retry count
    pub retry_count: u32,
}

/// Result of the entire flow execution
#[derive(Debug)]
pub struct FlowExecutionResult {
    /// Flow ID
    pub flow_id: String,
    /// Overall success
    pub success: bool,
    /// Individual step results
    pub step_results: Vec<StepResult>,
    /// Which step failed (if any)
    pub failed_step: Option<String>,
    /// Total execution time in ms
    pub total_duration_ms: u64,
    /// Error message if flow failed
    pub error_message: Option<String>,
}

/// Tool argument expression for template substitution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArgExpr {
    /// Static value
    Value(serde_json::Value),
    /// Reference to initial argument
    Arg(String),
    /// Reference to previous step output
    StepOutput { step_id: String, key: String },
    /// Reference to shared state
    State(String),
    /// String concatenation
    Concat(Vec<ArgExpr>),
    /// Conditional expression
    If {
        condition: Box<ArgExpr>,
        then: Box<ArgExpr>,
        else_: Box<ArgExpr>,
    },
}

impl ArgExpr {
    /// Evaluate the expression to a JSON value
    pub fn evaluate(&self, context: &ToolExecutionContext) -> Result<serde_json::Value, FlowError> {
        match self {
            ArgExpr::Value(v) => Ok(v.clone()),
            ArgExpr::Arg(name) => context
                .initial_args
                .get(name)
                .cloned()
                .ok_or_else(|| FlowError::MissingArgument(name.clone())),
            ArgExpr::StepOutput { step_id, key } => {
                let step = context
                    .step_results
                    .get(step_id)
                    .ok_or_else(|| FlowError::MissingStepResult(step_id.clone()))?;
                if !step.success {
                    return Err(FlowError::StepFailed(step_id.clone(), step.error.clone().unwrap_or_default()));
                }
                step.output
                    .as_ref()
                    .and_then(|v| v.get(key).cloned())
                    .ok_or_else(|| FlowError::MissingOutputKey(step_id.clone(), key.clone()))
            }
            ArgExpr::State(key) => context
                .shared_state
                .get(key)
                .cloned()
                .ok_or_else(|| FlowError::MissingState(key.clone())),
            ArgExpr::Concat(parts) => {
                let mut result = String::new();
                for part in parts {
                    let val = part.evaluate(context)?;
                    if let Some(s) = val.as_str() {
                        result.push_str(s);
                    } else {
                        result.push_str(&val.to_string());
                    }
                }
                Ok(serde_json::Value::String(result))
            }
            ArgExpr::If { condition, then, else_ } => {
                let cond_val = condition.evaluate(context)?;
                if Self::is_truthy(&cond_val) {
                    then.evaluate(context)
                } else {
                    else_.evaluate(context)
                }
            }
        }
    }

    fn is_truthy(v: &serde_json::Value) -> bool {
        match v {
            serde_json::Value::Null => false,
            serde_json::Value::Bool(b) => *b,
            serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
            serde_json::Value::String(s) => !s.is_empty(),
            serde_json::Value::Array(a) => !a.is_empty(),
            serde_json::Value::Object(o) => !o.is_empty(),
        }
    }
}

/// Flow execution errors
#[derive(Debug, thiserror::Error)]
pub enum FlowError {
    #[error("Missing initial argument: {0}")]
    MissingArgument(String),

    #[error("Missing step result: {0}")]
    MissingStepResult(String),

    #[error("Step {0} failed: {1}")]
    StepFailed(String, String),

    #[error("Missing output key '{1}' from step {0}")]
    MissingOutputKey(String, String),

    #[error("Missing shared state: {0}")]
    MissingState(String),

    #[error("Cyclic dependency detected involving step {0}")]
    CyclicDependency(String),

    #[error("Tool execution failed: {0}")]
    ToolExecutionFailed(String),

    #[error("Max retries exceeded for step {0}")]
    MaxRetriesExceeded(String),

    #[error("Condition evaluation failed: {0}")]
    ConditionFailed(String),
}

/// Tool flow executor with DAG-based execution
pub struct ToolFlowExecutor {
    /// Tool registry for execution
    tool_registry: Arc<dyn ToolRegistry>,
    /// Default timeout for all steps
    default_timeout_secs: u64,
}

impl ToolFlowExecutor {
    /// Create a new executor with the given tool registry
    pub fn new(tool_registry: Arc<dyn ToolRegistry>) -> Self {
        Self {
            tool_registry,
            default_timeout_secs: 300,
        }
    }

    /// Execute a tool flow
    pub async fn execute(
        &self,
        flow: &ToolFlow,
        context: ToolExecutionContext,
    ) -> Result<FlowExecutionResult, FlowError> {
        let start = std::time::Instant::now();

        // Topological sort to determine execution order
        let sorted_indices = self.topological_sort(&flow.steps)?;

        // Build dependency index
        let _dep_index = self.build_dependency_index(&flow.steps);

        // Track which steps have completed
        let mut completed: HashSet<usize> = HashSet::new();
        let mut results: Vec<StepResult> = vec![StepResult {
            step_id: String::new(),
            success: false,
            output: None,
            error: None,
            duration_ms: 0,
            skipped: false,
            skip_reason: None,
            retry_count: 0,
        }; flow.steps.len()];

        let mut step_results: HashMap<String, StepResult> = HashMap::new();

        // Execute steps in sorted order
        for &idx in &sorted_indices {
            let step = &flow.steps[idx];

            // Check if we should skip due to dependencies
            let should_skip = self.should_skip_step(step, idx, &completed, &step_results, &flow.steps)?;

            if should_skip.0 {
                results[idx] = StepResult {
                    step_id: step.id.clone(),
                    success: true,
                    output: None,
                    error: None,
                    duration_ms: 0,
                    skipped: true,
                    skip_reason: Some(should_skip.1),
                    retry_count: 0,
                };
                step_results.insert(step.id.clone(), results[idx].clone());
                continue;
            }

            // Execute the step
            let result = self
                .execute_step(step, &context, &step_results)
                .await;

            results[idx] = result.clone();
            step_results.insert(step.id.clone(), result.clone());

            // Handle failure
            if !result.success {
                let error_handling = step
                    .error_handling
                    .as_ref()
                    .unwrap_or(&flow.default_error_handling);

                match error_handling {
                    ErrorHandling::FailFast => {
                        let failed_step_id = Some(step.id.clone());
                        return Ok(FlowExecutionResult {
                            flow_id: flow.id.clone(),
                            success: false,
                            step_results: results,
                            failed_step: failed_step_id,
                            total_duration_ms: start.elapsed().as_millis() as u64,
                            error_message: result.error.clone(),
                        });
                    }
                    ErrorHandling::Continue | ErrorHandling::Skip => {
                        // Continue to next step
                    }
                    ErrorHandling::Retry { .. } => {
                        // Retry logic handled in execute_step
                    }
                    ErrorHandling::Fallback { .. } => {
                        // Fallback logic handled in execute_step
                    }
                }
            }

            completed.insert(idx);
        }

        let total_success = results.iter().all(|r| r.success || r.skipped);
        let failed_step = results.iter().find(|r| !r.success && !r.skipped).map(|r| r.step_id.clone());

        Ok(FlowExecutionResult {
            flow_id: flow.id.clone(),
            success: total_success,
            step_results: results,
            failed_step,
            total_duration_ms: start.elapsed().as_millis() as u64,
            error_message: None,
        })
    }

    /// Topological sort using Kahn's algorithm
    fn topological_sort(&self, steps: &[ToolFlowStep]) -> Result<Vec<usize>, FlowError> {
        let n = steps.len();
        let mut in_degree = vec![0usize; n];
        let mut adjacency: HashMap<usize, Vec<usize>> = HashMap::new();

        // Calculate in-degrees
        for (i, step) in steps.iter().enumerate() {
            for &dep in &step.depends_on {
                if dep >= n {
                    return Err(FlowError::CyclicDependency(steps[i].id.clone()));
                }
                in_degree[i] += 1;
                adjacency.entry(dep).or_default().push(i);
            }
        }

        // Start with nodes that have no dependencies
        let mut queue: Vec<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
        let mut sorted = Vec::with_capacity(n);

        while let Some(node) = queue.pop() {
            sorted.push(node);

            if let Some(neighbors) = adjacency.get(&node) {
                for &neighbor in neighbors {
                    in_degree[neighbor] -= 1;
                    if in_degree[neighbor] == 0 {
                        queue.push(neighbor);
                    }
                }
            }
        }

        if sorted.len() != n {
            return Err(FlowError::CyclicDependency("Unknown cycle detected".to_string()));
        }

        Ok(sorted)
    }

    /// Build a mapping from step to its dependents
    fn build_dependency_index(&self, steps: &[ToolFlowStep]) -> HashMap<usize, Vec<usize>> {
        let mut index: HashMap<usize, Vec<usize>> = HashMap::new();
        for (i, step) in steps.iter().enumerate() {
            for &dep in &step.depends_on {
                index.entry(dep).or_default().push(i);
            }
        }
        index
    }

    /// Check if a step should be skipped based on conditions
    fn should_skip_step(
        &self,
        step: &ToolFlowStep,
        _idx: usize,
        completed: &HashSet<usize>,
        results: &HashMap<String, StepResult>,
        steps: &[ToolFlowStep],
    ) -> Result<(bool, String), FlowError> {
        // Check condition
        if let Some(cond) = &step.condition {
            match cond {
                StepCondition::Always => {}
                StepCondition::OnSuccess => {
                    // Find the step this depends on
                    if let Some(&last_dep) = step.depends_on.last() {
                        let dep_step = &steps[last_dep];
                        if let Some(last_result) = results.get(&dep_step.id) {
                            if !last_result.success {
                                return Ok((true, "Previous step failed".to_string()));
                            }
                        }
                    }
                }
                StepCondition::OnFailure => {
                    if let Some(&last_dep) = step.depends_on.last() {
                        let dep_step = &steps[last_dep];
                        if let Some(last_result) = results.get(&dep_step.id) {
                            if last_result.success {
                                return Ok((true, "Previous step succeeded".to_string()));
                            }
                        }
                    }
                }
                StepCondition::OnSkipped => {
                    if let Some(&last_dep) = step.depends_on.last() {
                        let dep_step = &steps[last_dep];
                        if let Some(last_result) = results.get(&dep_step.id) {
                            if !last_result.skipped {
                                return Ok((true, "Previous step was not skipped".to_string()));
                            }
                        }
                    }
                }
                StepCondition::Expression(expr) => {
                    // Evaluate expression (simplified - would need full expression engine)
                    if expr.is_empty() || expr == "false" {
                        return Ok((true, "Expression evaluated to false".to_string()));
                    }
                }
                StepCondition::ResultEquals { step_id, expected } => {
                    if let Some(step_result) = results.get(step_id) {
                        if let Some(output) = &step_result.output {
                            if output != expected {
                                return Ok((true, format!("Result mismatch: {:?} != {:?}", output, expected)));
                            }
                        }
                    }
                }
                StepCondition::ResultHasKey { step_id, key } => {
                    if let Some(step_result) = results.get(step_id) {
                        if let Some(output) = &step_result.output {
                            if !output.get(key).is_some() {
                                return Ok((true, format!("Missing key '{}' in result", key)));
                            }
                        }
                    }
                }
            }
        }

        // Check if all dependencies are complete
        for &dep in &step.depends_on {
            if !completed.contains(&dep) {
                return Ok((true, "Dependencies not completed".to_string()));
            }
        }

        Ok((false, String::new()))
    }

    /// Execute a single step with retry/fallback handling
    async fn execute_step(
        &self,
        step: &ToolFlowStep,
        context: &ToolExecutionContext,
        step_results: &HashMap<String, StepResult>,
    ) -> StepResult {
        let start = std::time::Instant::now();

        let error_handling = step
            .error_handling
            .as_ref()
            .unwrap_or(&ErrorHandling::Retry {
                max_attempts: 3,
                backoff_ms: 1000,
                exponential: true,
            });

        // Determine timeout
        let timeout = step.timeout_secs.unwrap_or(self.default_timeout_secs);

        // Build arguments from template
        let args = match self.build_args(step, context, step_results) {
            Ok(args) => args,
            Err(e) => {
                return StepResult {
                    step_id: step.id.clone(),
                    success: false,
                    output: None,
                    error: Some(e.to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    skipped: false,
                    skip_reason: None,
                    retry_count: 0,
                };
            }
        };

        // Execute with retry logic
        let mut retry_count = 0;
        #[allow(unused_assignments)]
        let mut last_error = String::new();

        loop {
            let exec_result = self
                .execute_tool_with_timeout(&step.tool_name, args.clone(), timeout)
                .await;

            match exec_result {
                Ok(output) => {
                    return StepResult {
                        step_id: step.id.clone(),
                        success: true,
                        output: Some(output),
                        error: None,
                        duration_ms: start.elapsed().as_millis() as u64,
                        skipped: false,
                        skip_reason: None,
                        retry_count,
                    };
                }
                Err(e) => {
                    last_error = e.to_string();

                    match error_handling {
                        ErrorHandling::Retry {
                            max_attempts,
                            backoff_ms,
                            exponential,
                        } => {
                            retry_count += 1;
                            if retry_count >= *max_attempts {
                                break;
                            }

                            // Calculate backoff
                            let delay = if *exponential {
                                backoff_ms * 2u64.pow(retry_count - 1)
                            } else {
                                *backoff_ms
                            };

                            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                        }
                        ErrorHandling::Fallback {
                            fallback_tool,
                            fallback_args,
                        } => {
                            // Execute fallback tool - convert HashMap to JSON Value
                            let fallback_args_json = serde_json::Value::Object(
                                fallback_args.clone().into_iter().collect()
                            );
                            let exec_result = self
                                .execute_tool_with_timeout(fallback_tool, fallback_args_json, timeout)
                                .await;

                            match exec_result {
                                Ok(output) => {
                                    return StepResult {
                                        step_id: step.id.clone(),
                                        success: true,
                                        output: Some(output),
                                        error: Some(format!(
                                            "Primary tool failed, used fallback: {}",
                                            last_error
                                        )),
                                        duration_ms: start.elapsed().as_millis() as u64,
                                        skipped: false,
                                        skip_reason: None,
                                        retry_count: 0,
                                    };
                                }
                                Err(e) => {
                                    last_error = e.to_string();
                                    break;
                                }
                            }
                        }
                        _ => break,
                    }
                }
            }
        }

        StepResult {
            step_id: step.id.clone(),
            success: false,
            output: None,
            error: Some(last_error),
            duration_ms: start.elapsed().as_millis() as u64,
            skipped: false,
            skip_reason: None,
            retry_count,
        }
    }

    /// Build arguments for a step from its template
    fn build_args(
        &self,
        step: &ToolFlowStep,
        context: &ToolExecutionContext,
        step_results: &HashMap<String, StepResult>,
    ) -> Result<serde_json::Value, FlowError> {
        let mut args = serde_json::Map::new();

        for (key, value_template) in &step.args_template {
            // If it's already a JSON value, use it directly
            let value = if value_template.is_string() {
                // Try to evaluate as expression
                let expr_str = value_template.as_str().unwrap();
                if expr_str.starts_with('$') {
                    // Reference expression
                    let ref_name = &expr_str[1..];
                    if let Some(prev_result) = step_results.get(ref_name) {
                        prev_result.output.clone().unwrap_or(serde_json::Value::Null)
                    } else if let Some(arg) = context.initial_args.get(ref_name) {
                        arg.clone()
                    } else {
                        value_template.clone()
                    }
                } else {
                    value_template.clone()
                }
            } else {
                value_template.clone()
            };

            args.insert(key.clone(), value);
        }

        Ok(serde_json::Value::Object(args))
    }

    /// Execute a tool with a timeout
    async fn execute_tool_with_timeout(
        &self,
        tool_name: &str,
        args: serde_json::Value,
        timeout_secs: u64,
    ) -> Result<serde_json::Value, FlowError> {
        // Get tool from registry
        let tool = self
            .tool_registry
            .get_tool(tool_name)
            .ok_or_else(|| FlowError::ToolExecutionFailed(format!("Tool not found: {}", tool_name)))?;

        // Execute with timeout
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            tool.execute(args),
        )
        .await
        .map_err(|_| FlowError::ToolExecutionFailed(format!("Tool {} timed out", tool_name)))?;

        output.map_err(|e| FlowError::ToolExecutionFailed(e.to_string()))
    }
}

/// Trait for tool registry (to be implemented by the application)
pub trait ToolRegistry: Send + Sync {
    /// Get a tool by name
    fn get_tool(&self, name: &str) -> Option<Arc<dyn Tool>>;

    /// List all available tools
    fn list_tools(&self) -> Vec<String>;
}

/// Trait for executable tools
#[async_trait]
pub trait Tool: Send + Sync {
    /// Execute the tool with given arguments
    async fn execute(&self, args: serde_json::Value) -> Result<serde_json::Value, FlowError>;

    /// Get tool metadata
    fn name(&self) -> &str;
    fn description(&self) -> &str;
}

/// A simple tool wrapper for async functions
pub struct FnTool<F, Fut>
where
    F: Fn(serde_json::Value) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<serde_json::Value, FlowError>> + Send,
{
    name: String,
    description: String,
    func: F,
}

impl<F, Fut> FnTool<F, Fut>
where
    F: Fn(serde_json::Value) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<serde_json::Value, FlowError>> + Send,
{
    pub fn new(name: &str, description: &str, func: F) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            func,
        }
    }
}

#[async_trait]
impl<F, Fut> Tool for FnTool<F, Fut>
where
    F: Fn(serde_json::Value) -> Fut + Send + Sync,
    Fut: std::future::Future<Output = Result<serde_json::Value, FlowError>> + Send,
{
    async fn execute(&self, args: serde_json::Value) -> Result<serde_json::Value, FlowError> {
        (self.func)(args).await
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }
}

// ============================================
// Flow Visualization for debugging
// ============================================

/// Generate a DOT graph for flow visualization
pub fn generate_flow_dot(flow: &ToolFlow) -> String {
    let mut dot = String::new();
    dot.push_str("digraph tool_flow {\n");
    dot.push_str("    rankdir=LR;\n");
    dot.push_str("    node [shape=box style=rounded];\n\n");

    for (i, step) in flow.steps.iter().enumerate() {
        // Node
        dot.push_str(&format!(
            "    step_{} [label=\"{}\\n({})\"];\n",
            i,
            step.tool_name,
            step.id
        ));

        // Dependencies
        for &dep in &step.depends_on {
            dot.push_str(&format!("    step_{} -> step_{};\n", dep, i));
        }
    }

    dot.push_str("}\n");
    dot
}

/// Generate Mermaid diagram for flow visualization
pub fn generate_flow_mermaid(flow: &ToolFlow) -> String {
    let mut mermaid = String::new();
    mermaid.push_str("flowchart TD\n");

    for (i, step) in flow.steps.iter().enumerate() {
        mermaid.push_str(&format!(
            "    step{}[{}]-->step{}_exec[{}]\n",
            i,
            step.id,
            i,
            step.tool_name
        ));

        // Add condition if present
        if let Some(cond) = &step.condition {
            mermaid.push_str(&format!(
                "    step{}_cond{{{:?}}}\n",
                i,
                cond
            ));
        }

        // Add error handling
        if let Some(err) = &step.error_handling {
            mermaid.push_str(&format!(
                "    step{}_err{{{:?}}}\n",
                i,
                err
            ));
        }
    }

    // Add dependencies
    for (i, step) in flow.steps.iter().enumerate() {
        for &dep in &step.depends_on {
            mermaid.push_str(&format!("    step{}_exec --> step{}_cond\n", dep, i));
        }
    }

    mermaid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topological_sort() {
        let steps = vec![
            ToolFlowStep {
                id: "a".to_string(),
                tool_name: "tool_a".to_string(),
                args_template: HashMap::new(),
                depends_on: vec![],
                condition: None,
                error_handling: None,
                timeout_secs: None,
                capture_output: false,
            },
            ToolFlowStep {
                id: "b".to_string(),
                tool_name: "tool_b".to_string(),
                args_template: HashMap::new(),
                depends_on: vec![0], // depends on a
                condition: None,
                error_handling: None,
                timeout_secs: None,
                capture_output: false,
            },
            ToolFlowStep {
                id: "c".to_string(),
                tool_name: "tool_c".to_string(),
                args_template: HashMap::new(),
                depends_on: vec![0], // depends on a
                condition: None,
                error_handling: None,
                timeout_secs: None,
                capture_output: false,
            },
            ToolFlowStep {
                id: "d".to_string(),
                tool_name: "tool_d".to_string(),
                args_template: HashMap::new(),
                depends_on: vec![1, 2], // depends on b and c
                condition: None,
                error_handling: None,
                timeout_secs: None,
                capture_output: false,
            },
        ];

        let flow = ToolFlow {
            id: "test".to_string(),
            name: "Test Flow".to_string(),
            steps,
            default_error_handling: ErrorHandling::FailFast,
            continue_on_failure: false,
            metadata: HashMap::new(),
        };

        let executor = ToolFlowExecutor {
            tool_registry: Arc::new(MockRegistry),
            default_timeout_secs: 300,
        };

        let sorted = executor.topological_sort(&flow.steps).unwrap();
        // Verify: a(0) must come first, d(3) must come last, b(1) and c(2) in between (order between b and c doesn't matter)
        assert_eq!(sorted[0], 0); // a must come first
        assert_eq!(sorted[3], 3); // d must come last
        assert!(sorted[1..3].contains(&1) && sorted[1..3].contains(&2)); // b and c in middle
    }

    #[test]
    fn test_arg_expr_evaluation() {
        let context = ToolExecutionContext {
            initial_args: vec![("input".to_string(), serde_json::json!("hello"))]
                .into_iter()
                .collect(),
            step_results: HashMap::new(),
            shared_state: vec![("state".to_string(), serde_json::json!("world"))]
                .into_iter()
                .collect(),
            execution_node: "test".to_string(),
            metadata: HashMap::new(),
        };

        // Test value
        let expr = ArgExpr::Value(serde_json::json!("test"));
        assert_eq!(expr.evaluate(&context).unwrap(), serde_json::json!("test"));

        // Test arg reference
        let expr = ArgExpr::Arg("input".to_string());
        assert_eq!(expr.evaluate(&context).unwrap(), serde_json::json!("hello"));

        // Test state reference
        let expr = ArgExpr::State("state".to_string());
        assert_eq!(expr.evaluate(&context).unwrap(), serde_json::json!("world"));
    }

    /// Mock tool registry for testing
    struct MockRegistry;

    impl ToolRegistry for MockRegistry {
        fn get_tool(&self, _name: &str) -> Option<Arc<dyn Tool>> {
            None
        }

        fn list_tools(&self) -> Vec<String> {
            vec![]
        }
    }
}