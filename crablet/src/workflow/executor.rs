use super::types::WorkflowNode;
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, warn};

#[derive(Debug, Default)]
pub struct NodeExecutorRegistry;

impl NodeExecutorRegistry {
    pub fn new() -> Self {
        Self
    }

    /// Dispatch execution to the correct handler based on `node.node_type`.
    pub async fn execute(
        &self,
        node: &WorkflowNode,
        inputs: &HashMap<String, Value>,
    ) -> Result<HashMap<String, Value>, String> {
        debug!(
            "Executing node '{}' of type '{}'",
            node.id, node.node_type
        );
        match node.node_type.as_str() {
            "start" => execute_start(node, inputs),
            "end" => execute_end(node, inputs),
            "condition" => execute_condition(node, inputs),
            "loop" => execute_loop(node, inputs),
            "llm" => execute_llm(node, inputs).await,
            "agent" => execute_agent(node, inputs).await,
            "knowledge" => execute_knowledge(node, inputs).await,
            "code" => execute_code(node, inputs),
            "template" => execute_template(node, inputs),
            "http" => execute_http(node, inputs).await,
            "variable" => execute_variable(node, inputs),
            other => {
                warn!("Unknown node type '{}', passing through inputs", other);
                Ok(inputs.clone())
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Control flow nodes
// ──────────────────────────────────────────────────────────────

fn execute_start(
    _node: &WorkflowNode,
    inputs: &HashMap<String, Value>,
) -> Result<HashMap<String, Value>, String> {
    // Start node: pass through all workflow inputs as outputs
    Ok(inputs.clone())
}

fn execute_end(
    _node: &WorkflowNode,
    inputs: &HashMap<String, Value>,
) -> Result<HashMap<String, Value>, String> {
    // End node: collect all inputs as the final result
    Ok(inputs.clone())
}

fn execute_condition(
    node: &WorkflowNode,
    inputs: &HashMap<String, Value>,
) -> Result<HashMap<String, Value>, String> {
    // Evaluate a boolean condition expression from node config or input
    let condition_result = if let Some(config) = &node.data.config {
        if let Some(expr) = config.get("condition").and_then(|v| v.as_str()) {
            evaluate_condition_expr(expr, inputs)
        } else {
            // Fallback: check "condition" key in inputs
            inputs
                .get("condition")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        }
    } else {
        inputs
            .get("condition")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    };

    let mut outputs = HashMap::new();
    if condition_result {
        outputs.insert("true".to_string(), inputs.get("input").cloned().unwrap_or(Value::Null));
    } else {
        outputs.insert("false".to_string(), inputs.get("input").cloned().unwrap_or(Value::Null));
    }
    outputs.insert("result".to_string(), Value::Bool(condition_result));
    Ok(outputs)
}

/// Simple condition evaluator supporting "==" / "!=" / ">" / "<" comparisons
fn evaluate_condition_expr(expr: &str, inputs: &HashMap<String, Value>) -> bool {
    // Try equality: `field == value` / `field != value` / `field > value` / `field < value`
    for op in &["==", "!=", ">=", "<=", ">", "<"] {
        if let Some((lhs, rhs)) = expr.split_once(op) {
            let lhs = lhs.trim();
            let rhs = rhs.trim();
            let lval = inputs.get(lhs).cloned().unwrap_or_else(|| {
                // Treat as literal string / number
                serde_json::from_str(lhs).unwrap_or(Value::String(lhs.to_string()))
            });
            let rval: Value =
                serde_json::from_str(rhs).unwrap_or(Value::String(rhs.to_string()));
            return match *op {
                "==" => lval == rval,
                "!=" => lval != rval,
                ">" => lval.as_f64().unwrap_or(0.0) > rval.as_f64().unwrap_or(0.0),
                "<" => lval.as_f64().unwrap_or(0.0) < rval.as_f64().unwrap_or(0.0),
                ">=" => lval.as_f64().unwrap_or(0.0) >= rval.as_f64().unwrap_or(0.0),
                "<=" => lval.as_f64().unwrap_or(0.0) <= rval.as_f64().unwrap_or(0.0),
                _ => false,
            };
        }
    }
    // Plain boolean input name
    inputs
        .get(expr.trim())
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

fn execute_loop(
    _node: &WorkflowNode,
    inputs: &HashMap<String, Value>,
) -> Result<HashMap<String, Value>, String> {
    // Return the first element of "items" array for DAG processing
    let items = inputs
        .get("items")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut outputs = HashMap::new();
    if let Some(first) = items.first() {
        outputs.insert("item".to_string(), first.clone());
    } else {
        outputs.insert("item".to_string(), Value::Null);
    }
    outputs.insert("items".to_string(), Value::Array(items));
    outputs.insert("count".to_string(), Value::Number(
        serde_json::Number::from(outputs["items"].as_array().map_or(0, |a| a.len())),
    ));
    Ok(outputs)
}

// ──────────────────────────────────────────────────────────────
// AI nodes
// ──────────────────────────────────────────────────────────────

async fn execute_llm(
    node: &WorkflowNode,
    inputs: &HashMap<String, Value>,
) -> Result<HashMap<String, Value>, String> {
    let prompt = inputs
        .get("prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let system_prompt = inputs
        .get("system_prompt")
        .or_else(|| node.data.config.as_ref().and_then(|c| c.get("system_prompt")))
        .and_then(|v| v.as_str())
        .unwrap_or("You are a helpful assistant.")
        .to_string();
    let model = node
        .data
        .config
        .as_ref()
        .and_then(|c| c.get("model"))
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

    debug!("LLM node '{}': model={}, prompt_len={}", node.id, model, prompt.len());

    // Runtime LLM call goes here when integrated with CognitiveRouter.
    // For now produce a structured placeholder that clearly communicates intent.
    let text = format!(
        "[LLM:{model}] system={system_prompt} | prompt={prompt}"
    );

    let mut outputs = HashMap::new();
    outputs.insert("text".to_string(), Value::String(text));
    outputs.insert("model".to_string(), Value::String(model));
    outputs.insert("prompt".to_string(), Value::String(prompt));
    Ok(outputs)
}

async fn execute_agent(
    node: &WorkflowNode,
    inputs: &HashMap<String, Value>,
) -> Result<HashMap<String, Value>, String> {
    let task = inputs
        .get("task")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let agent_name = node
        .data
        .config
        .as_ref()
        .and_then(|c| c.get("agent_name"))
        .and_then(|v| v.as_str())
        .unwrap_or("default")
        .to_string();

    debug!("Agent node '{}': agent={}, task_len={}", node.id, agent_name, task.len());

    let mut outputs = HashMap::new();
    outputs.insert(
        "result".to_string(),
        Value::String(format!("[Agent:{agent_name}] task={task}")),
    );
    outputs.insert("agent_name".to_string(), Value::String(agent_name));
    Ok(outputs)
}

async fn execute_knowledge(
    node: &WorkflowNode,
    inputs: &HashMap<String, Value>,
) -> Result<HashMap<String, Value>, String> {
    let query = inputs
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let top_k = node
        .data
        .config
        .as_ref()
        .and_then(|c| c.get("top_k"))
        .and_then(|v| v.as_u64())
        .unwrap_or(5) as usize;

    debug!("Knowledge node '{}': query='{}', top_k={}", node.id, query, top_k);

    // Placeholder result set; real implementation will call vector store
    let results = Value::Array(vec![serde_json::json!({
        "id": "doc-placeholder",
        "score": 1.0,
        "content": format!("Relevant context for: {query}"),
    })]);

    let mut outputs = HashMap::new();
    outputs.insert("results".to_string(), results);
    outputs.insert("query".to_string(), Value::String(query));
    outputs.insert("count".to_string(), Value::Number(serde_json::Number::from(1u64)));
    Ok(outputs)
}

// ──────────────────────────────────────────────────────────────
// Processing nodes
// ──────────────────────────────────────────────────────────────

fn execute_code(
    node: &WorkflowNode,
    inputs: &HashMap<String, Value>,
) -> Result<HashMap<String, Value>, String> {
    let code = node
        .data
        .config
        .as_ref()
        .and_then(|c| c.get("code"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if code.is_empty() {
        let mut outputs = HashMap::new();
        outputs.insert("result".to_string(), inputs.get("input").cloned().unwrap_or(Value::Null));
        return Ok(outputs);
    }

    // Safe code execution sandbox – currently evaluates simple math expressions
    // and constant assignments.  Full scripting support requires embedding a
    // JS/Lua/Python interpreter; this is intentionally constrained.
    let result = evaluate_simple_expression(&code, inputs);

    let mut outputs = HashMap::new();
    outputs.insert("result".to_string(), result);
    Ok(outputs)
}

/// Evaluate very simple arithmetic / string expressions from code strings.
fn evaluate_simple_expression(code: &str, inputs: &HashMap<String, Value>) -> Value {
    let trimmed = code.trim();
    // Return statement
    let expr = trimmed
        .trim_start_matches("return")
        .trim()
        .trim_end_matches(';');

    // Variable reference
    if let Some(val) = inputs.get(expr) {
        return val.clone();
    }
    // JSON literal
    if let Ok(val) = serde_json::from_str(expr) {
        return val;
    }
    // Simple arithmetic: `a + b`, `a - b`, `a * b`, `a / b`
    for op in &["+", "-", "*", "/"] {
        if let Some((lhs, rhs)) = expr.split_once(op) {
            let l = resolve_numeric(lhs.trim(), inputs);
            let r = resolve_numeric(rhs.trim(), inputs);
            let result = match *op {
                "+" => l + r,
                "-" => l - r,
                "*" => l * r,
                "/" => {
                    if r == 0.0 {
                        return Value::String("division by zero".to_string());
                    }
                    l / r
                }
                _ => unreachable!(),
            };
            return serde_json::json!(result);
        }
    }
    Value::String(expr.to_string())
}

fn resolve_numeric(s: &str, inputs: &HashMap<String, Value>) -> f64 {
    if let Some(val) = inputs.get(s) {
        return val.as_f64().unwrap_or(0.0);
    }
    s.parse::<f64>().unwrap_or(0.0)
}

fn execute_template(
    node: &WorkflowNode,
    inputs: &HashMap<String, Value>,
) -> Result<HashMap<String, Value>, String> {
    let template = node
        .data
        .config
        .as_ref()
        .and_then(|c| c.get("template"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Simple Mustache-style `{{variable}}` substitution
    let rendered = render_template(&template, inputs);

    let mut outputs = HashMap::new();
    outputs.insert("result".to_string(), Value::String(rendered));
    Ok(outputs)
}

/// Render `{{key}}` placeholders in a template string from `vars`.
fn render_template(template: &str, vars: &HashMap<String, Value>) -> String {
    let mut result = template.to_string();
    for (key, val) in vars {
        let placeholder = format!("{{{{{}}}}}", key);
        let replacement = match val {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        result = result.replace(&placeholder, &replacement);
    }
    result
}

// ──────────────────────────────────────────────────────────────
// Integration nodes
// ──────────────────────────────────────────────────────────────

async fn execute_http(
    node: &WorkflowNode,
    inputs: &HashMap<String, Value>,
) -> Result<HashMap<String, Value>, String> {
    let url = inputs
        .get("url")
        .or_else(|| node.data.config.as_ref().and_then(|c| c.get("url")))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "HTTP node requires 'url' input".to_string())?
        .to_string();

    let method = node
        .data
        .config
        .as_ref()
        .and_then(|c| c.get("method"))
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();

    let headers: HashMap<String, String> = node
        .data
        .config
        .as_ref()
        .and_then(|c| c.get("headers"))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let body = inputs
        .get("body")
        .or_else(|| node.data.config.as_ref().and_then(|c| c.get("body")))
        .cloned();

    debug!("HTTP node '{}': {} {}", node.id, method, url);

    // Build request using reqwest
    let client = reqwest::Client::new();
    let mut req_builder = match method.as_str() {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        "PATCH" => client.patch(&url),
        _ => client.get(&url),
    };

    for (k, v) in &headers {
        req_builder = req_builder.header(k, v);
    }

    if let Some(b) = body {
        req_builder = req_builder
            .header("content-type", "application/json")
            .body(b.to_string());
    }

    let response = req_builder
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let status = response.status().as_u16();
    let body_text = response
        .text()
        .await
        .unwrap_or_else(|_| String::new());

    // Try to parse body as JSON
    let body_value: Value = serde_json::from_str(&body_text)
        .unwrap_or(Value::String(body_text));

    let mut outputs = HashMap::new();
    outputs.insert("status".to_string(), Value::Number(serde_json::Number::from(status)));
    outputs.insert("body".to_string(), body_value);
    Ok(outputs)
}

// ──────────────────────────────────────────────────────────────
// Data nodes
// ──────────────────────────────────────────────────────────────

fn execute_variable(
    node: &WorkflowNode,
    inputs: &HashMap<String, Value>,
) -> Result<HashMap<String, Value>, String> {
    // Support get / set / delete operations on a variable
    let operation = node
        .data
        .config
        .as_ref()
        .and_then(|c| c.get("operation"))
        .and_then(|v| v.as_str())
        .unwrap_or("get");

    let var_name = node
        .data
        .config
        .as_ref()
        .and_then(|c| c.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("value");

    let result = match operation {
        "set" => {
            let new_val = inputs.get("value").cloned().unwrap_or(Value::Null);
            new_val
        }
        "delete" => Value::Null,
        _ /* "get" */ => inputs
            .get(var_name)
            .or_else(|| inputs.get("value"))
            .cloned()
            .unwrap_or(Value::Null),
    };

    let mut outputs = HashMap::new();
    outputs.insert("result".to_string(), result);
    outputs.insert("name".to_string(), Value::String(var_name.to_string()));
    Ok(outputs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workflow::types::{Position, WorkflowNodeData};

    fn make_node(id: &str, node_type: &str, config: Option<serde_json::Value>) -> WorkflowNode {
        WorkflowNode {
            id: id.to_string(),
            node_type: node_type.to_string(),
            position: Position { x: 0.0, y: 0.0 },
            data: WorkflowNodeData {
                label: id.to_string(),
                description: None,
                config: config.and_then(|v| v.as_object().cloned()).map(|o| {
                    o.into_iter().collect()
                }),
                inputs: None,
                outputs: None,
            },
        }
    }

    #[tokio::test]
    async fn test_start_node() {
        let reg = NodeExecutorRegistry::new();
        let node = make_node("start", "start", None);
        let mut inputs = HashMap::new();
        inputs.insert("greeting".to_string(), Value::String("hello".to_string()));
        let outputs = reg.execute(&node, &inputs).await.unwrap();
        assert_eq!(outputs["greeting"], Value::String("hello".to_string()));
    }

    #[tokio::test]
    async fn test_condition_node_true() {
        let reg = NodeExecutorRegistry::new();
        let node = make_node("cond", "condition", Some(serde_json::json!({
            "condition": "score > 5"
        })));
        let mut inputs = HashMap::new();
        inputs.insert("score".to_string(), serde_json::json!(10));
        inputs.insert("input".to_string(), Value::String("data".to_string()));
        let outputs = reg.execute(&node, &inputs).await.unwrap();
        assert_eq!(outputs.get("result"), Some(&Value::Bool(true)));
        assert!(outputs.contains_key("true"));
    }

    #[tokio::test]
    async fn test_condition_node_false() {
        let reg = NodeExecutorRegistry::new();
        let node = make_node("cond", "condition", Some(serde_json::json!({
            "condition": "score > 5"
        })));
        let mut inputs = HashMap::new();
        inputs.insert("score".to_string(), serde_json::json!(3));
        inputs.insert("input".to_string(), Value::String("data".to_string()));
        let outputs = reg.execute(&node, &inputs).await.unwrap();
        assert_eq!(outputs.get("result"), Some(&Value::Bool(false)));
        assert!(outputs.contains_key("false"));
    }

    #[tokio::test]
    async fn test_template_node() {
        let reg = NodeExecutorRegistry::new();
        let node = make_node("tmpl", "template", Some(serde_json::json!({
            "template": "Hello {{name}}, you are {{age}} years old."
        })));
        let mut inputs = HashMap::new();
        inputs.insert("name".to_string(), Value::String("Alice".to_string()));
        inputs.insert("age".to_string(), serde_json::json!(30));
        let outputs = reg.execute(&node, &inputs).await.unwrap();
        assert_eq!(
            outputs["result"],
            Value::String("Hello Alice, you are 30 years old.".to_string())
        );
    }

    #[tokio::test]
    async fn test_code_node_arithmetic() {
        let reg = NodeExecutorRegistry::new();
        let node = make_node("code", "code", Some(serde_json::json!({
            "code": "a + b"
        })));
        let mut inputs = HashMap::new();
        inputs.insert("a".to_string(), serde_json::json!(3));
        inputs.insert("b".to_string(), serde_json::json!(4));
        let outputs = reg.execute(&node, &inputs).await.unwrap();
        assert_eq!(outputs["result"].as_f64().unwrap(), 7.0);
    }

    #[tokio::test]
    async fn test_variable_node_get() {
        let reg = NodeExecutorRegistry::new();
        let node = make_node("var", "variable", Some(serde_json::json!({
            "operation": "get",
            "name": "my_var"
        })));
        let mut inputs = HashMap::new();
        inputs.insert("my_var".to_string(), Value::String("test_val".to_string()));
        let outputs = reg.execute(&node, &inputs).await.unwrap();
        assert_eq!(outputs["result"], Value::String("test_val".to_string()));
    }

    #[tokio::test]
    async fn test_loop_node() {
        let reg = NodeExecutorRegistry::new();
        let node = make_node("loop", "loop", None);
        let mut inputs = HashMap::new();
        inputs.insert("items".to_string(), serde_json::json!([1, 2, 3]));
        let outputs = reg.execute(&node, &inputs).await.unwrap();
        assert_eq!(outputs["item"], serde_json::json!(1));
        assert_eq!(outputs["count"], serde_json::json!(3));
    }

    #[tokio::test]
    async fn test_unknown_node_passthrough() {
        let reg = NodeExecutorRegistry::new();
        let node = make_node("x", "custom_type", None);
        let mut inputs = HashMap::new();
        inputs.insert("key".to_string(), Value::String("val".to_string()));
        let outputs = reg.execute(&node, &inputs).await.unwrap();
        assert_eq!(outputs["key"], Value::String("val".to_string()));
    }
}
