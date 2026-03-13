//! 技能链可视化与调试工具
//!
//! 提供功能:
//! - Mermaid/Graphviz 图导出
//! - 执行追踪与回放
//! - 性能分析
//! - 交互式调试

use std::collections::HashMap;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use chrono::{DateTime, Utc};

use super::chain::{SkillChain, StepType};
use super::composite::{CompositeSkill, CompositionType};

/// 图导出格式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphFormat {
    Mermaid,
    GraphvizDot,
    PlantUML,
    CytoscapeJSON,
}

/// 图节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub node_type: String,
    pub shape: String,
    pub color: String,
    pub metadata: HashMap<String, Value>,
}

/// 图边
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: Option<String>,
    pub style: String,
    pub metadata: HashMap<String, Value>,
}

/// 技能链图表示
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillChainGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub metadata: HashMap<String, Value>,
}

/// 图导出器
pub struct GraphExporter;

impl GraphExporter {
    /// 导出技能链为图
    pub fn export_chain(chain: &SkillChain, format: GraphFormat) -> Result<String> {
        let graph = Self::chain_to_graph(chain)?;
        
        match format {
            GraphFormat::Mermaid => Self::to_mermaid(&graph),
            GraphFormat::GraphvizDot => Self::to_graphviz(&graph),
            GraphFormat::PlantUML => Self::to_plantuml(&graph),
            GraphFormat::CytoscapeJSON => Self::to_cytoscape(&graph),
        }
    }

    /// 导出组合技能为图
    pub fn export_composite(composite: &CompositeSkill, format: GraphFormat) -> Result<String> {
        let graph = Self::composite_to_graph(composite)?;
        
        match format {
            GraphFormat::Mermaid => Self::to_mermaid(&graph),
            GraphFormat::GraphvizDot => Self::to_graphviz(&graph),
            GraphFormat::PlantUML => Self::to_plantuml(&graph),
            GraphFormat::CytoscapeJSON => Self::to_cytoscape(&graph),
        }
    }

    /// 技能链转图
    fn chain_to_graph(chain: &SkillChain) -> Result<SkillChainGraph> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        // 添加节点
        for step in &chain.steps {
            let (shape, color) = Self::get_step_style(&step.step_type);
            
            nodes.push(GraphNode {
                id: step.id.clone(),
                label: step.name.clone(),
                node_type: format!("{:?}", step.step_type).to_lowercase(),
                shape,
                color,
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("timeout".to_string(), json!(step.timeout_secs));
                    if let Some(ref skill) = step.skill_node {
                        meta.insert("skill".to_string(), json!(skill.skill_name.clone()));
                    }
                    meta
                },
            });
        }

        // 添加边
        for (idx, conn) in chain.connections.iter().enumerate() {
            edges.push(GraphEdge {
                id: format!("edge_{}", idx),
                source: conn.from.clone(),
                target: conn.to.clone(),
                label: conn.label.clone(),
                style: if conn.condition.is_some() { "dashed" } else { "solid" }.to_string(),
                metadata: {
                    let mut meta = HashMap::new();
                    if let Some(ref cond) = conn.condition {
                        meta.insert("condition".to_string(), json!(cond));
                    }
                    meta
                },
            });
        }

        let mut metadata = HashMap::new();
        metadata.insert("name".to_string(), json!(chain.name.clone()));
        metadata.insert("version".to_string(), json!(chain.version.clone()));

        Ok(SkillChainGraph {
            nodes,
            edges,
            metadata,
        })
    }

    /// 组合技能转图
    fn composite_to_graph(composite: &CompositeSkill) -> Result<SkillChainGraph> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        // 添加节点
        for (idx, node) in composite.nodes.iter().enumerate() {
            let color = match composite.composition_type {
                CompositionType::Sequential => "#4CAF50",
                CompositionType::Parallel => "#2196F3",
                CompositionType::Conditional => "#FF9800",
                CompositionType::Loop => "#9C27B0",
                CompositionType::Map => "#E91E63",
                CompositionType::Reduce => "#795548",
            };

            nodes.push(GraphNode {
                id: node.id.clone(),
                label: format!("{}\n({})", node.id, node.skill_name),
                node_type: "skill".to_string(),
                shape: "box".to_string(),
                color: color.to_string(),
                metadata: {
                    let mut meta = HashMap::new();
                    meta.insert("skill_name".to_string(), json!(node.skill_name.clone()));
                    meta.insert("timeout".to_string(), json!(node.timeout_secs));
                    meta
                },
            });

            // 添加顺序边
            if idx > 0 {
                edges.push(GraphEdge {
                    id: format!("edge_{}", idx - 1),
                    source: composite.nodes[idx - 1].id.clone(),
                    target: node.id.clone(),
                    label: None,
                    style: "solid".to_string(),
                    metadata: HashMap::new(),
                });
            }
        }

        let mut metadata = HashMap::new();
        metadata.insert("name".to_string(), json!(composite.name.clone()));
        metadata.insert("composition_type".to_string(), json!(format!("{:?}", composite.composition_type)));

        Ok(SkillChainGraph {
            nodes,
            edges,
            metadata,
        })
    }

    /// 获取步骤样式
    fn get_step_style(step_type: &StepType) -> (String, String) {
        match step_type {
            StepType::Skill => ("box".to_string(), "#4CAF50".to_string()),
            StepType::Condition => ("diamond".to_string(), "#FF9800".to_string()),
            StepType::ParallelStart => ("hexagon".to_string(), "#2196F3".to_string()),
            StepType::ParallelJoin => ("ellipse".to_string(), "#2196F3".to_string()),
            StepType::SubChain => ("folder".to_string(), "#9C27B0".to_string()),
            StepType::Wait => ("circle".to_string(), "#757575".to_string()),
            StepType::HumanApproval => (" Stadium".to_string(), "#F44336".to_string()),
            StepType::EmitEvent => ("cloud".to_string(), "#00BCD4".to_string()),
        }
    }

    /// 导出为 Mermaid
    fn to_mermaid(graph: &SkillChainGraph) -> Result<String> {
        let mut output = String::from("flowchart TD\n");
        
        // 添加节点样式定义
        output.push_str("    %% Styles\n");
        output.push_str("    classDef skill fill:#4CAF50,stroke:#333,stroke-width:2px;\n");
        output.push_str("    classDef condition fill:#FF9800,stroke:#333,stroke-width:2px;\n");
        output.push_str("    classDef parallel fill:#2196F3,stroke:#333,stroke-width:2px;\n");
        output.push_str("    classDef error fill:#F44336,stroke:#333,stroke-width:2px;\n");
        output.push_str("\n");

        // 添加节点
        for node in &graph.nodes {
            let shape_start = match node.shape.as_str() {
                "diamond" => "{",
                "circle" => "((",
                "ellipse" => "((",
                "hexagon" => "{{",
                _ => "[",
            };
            let shape_end = match node.shape.as_str() {
                "diamond" => "}",
                "circle" => "))",
                "ellipse" => "))",
                "hexagon" => "}}",
                _ => "]",
            };
            
            output.push_str(&format!(
                "    {}{}{}\n",
                node.id,
                shape_start,
                shape_end.replace("]", &format!("{}]", node.label))
                    .replace("}", &format!("{}}}", node.label))
                    .replace(")", &format!("{}))", node.label))
                    .replace("}}", &format!("{}}}", node.label))
            ));
            
            // 添加类
            output.push_str(&format!("    class {} {};\n", node.id, node.node_type));
        }

        output.push_str("\n");

        // 添加边
        for edge in &graph.edges {
            let style = if edge.style == "dashed" { "-.->" } else { "-->" };
            if let Some(ref label) = edge.label {
                output.push_str(&format!(
                    "    {} {}|{}| {}\n",
                    edge.source, style, label, edge.target
                ));
            } else {
                output.push_str(&format!(
                    "    {} {} {}\n",
                    edge.source, style, edge.target
                ));
            }
        }

        Ok(output)
    }

    /// 导出为 Graphviz DOT
    fn to_graphviz(graph: &SkillChainGraph) -> Result<String> {
        let mut output = String::from("digraph SkillChain {\n");
        output.push_str("    rankdir=TB;\n");
        output.push_str("    node [shape=box, style=filled, fontname=\"Arial\"];\n");
        output.push_str("    edge [fontname=\"Arial\"];\n\n");

        // 添加节点
        for node in &graph.nodes {
            let shape = match node.shape.as_str() {
                "diamond" => "diamond",
                "circle" => "circle",
                "ellipse" => "ellipse",
                "hexagon" => "hexagon",
                _ => "box",
            };
            
            output.push_str(&format!(
                "    \"{}\" [label=\"{}\", shape={}, fillcolor=\"{}\"];\n",
                node.id, node.label.replace("\n", "\\n"), shape, node.color
            ));
        }

        output.push_str("\n");

        // 添加边
        for edge in &graph.edges {
            let style = if edge.style == "dashed" { ", style=dashed" } else { "" };
            if let Some(ref label) = edge.label {
                output.push_str(&format!(
                    "    \"{}\" -> \"{}\" [label=\"{}\"{}];\n",
                    edge.source, edge.target, label, style
                ));
            } else {
                output.push_str(&format!(
                    "    \"{}\" -> \"{}\" [{}];\n",
                    edge.source, edge.target, style.trim_start_matches(", ")
                ));
            }
        }

        output.push_str("}\n");
        Ok(output)
    }

    /// 导出为 PlantUML
    fn to_plantuml(graph: &SkillChainGraph) -> Result<String> {
        let mut output = String::from("@startuml\n");
        output.push_str("skinparam handwritten false\n");
        output.push_str("skinparam backgroundColor #FEFEFE\n\n");

        // 添加标题
        if let Some(name) = graph.metadata.get("name") {
            output.push_str(&format!("title {}\n\n", name.as_str().unwrap_or("Skill Chain")));
        }

        // 添加节点
        for node in &graph.nodes {
            let uml_type = match node.shape.as_str() {
                "diamond" => "partition",
                "circle" => "circle",
                _ => "rectangle",
            };
            
            output.push_str(&format!(
                "{} \"{}\" as {} #{}\n",
                uml_type, node.label.replace("\n", "\\n"), node.id, node.color.trim_start_matches('#')
            ));
        }

        output.push_str("\n");

        // 添加连接
        for edge in &graph.edges {
            if let Some(ref label) = edge.label {
                output.push_str(&format!(
                    "{} --> {} : {}\n",
                    edge.source, edge.target, label
                ));
            } else {
                output.push_str(&format!(
                    "{} --> {}\n",
                    edge.source, edge.target
                ));
            }
        }

        output.push_str("\n@enduml\n");
        Ok(output)
    }

    /// 导出为 Cytoscape JSON
    fn to_cytoscape(graph: &SkillChainGraph) -> Result<String> {
        let cyto_graph = serde_json::json!({
            "elements": {
                "nodes": graph.nodes.iter().map(|n| {
                    serde_json::json!({
                        "data": {
                            "id": n.id,
                            "label": n.label,
                            "type": n.node_type,
                            "shape": n.shape,
                            "color": n.color,
                            "metadata": n.metadata
                        }
                    })
                }).collect::<Vec<_>>(),
                "edges": graph.edges.iter().map(|e| {
                    serde_json::json!({
                        "data": {
                            "id": e.id,
                            "source": e.source,
                            "target": e.target,
                            "label": e.label,
                            "style": e.style,
                            "metadata": e.metadata
                        }
                    })
                }).collect::<Vec<_>>()
            },
            "metadata": graph.metadata
        });

        serde_json::to_string_pretty(&cyto_graph)
            .map_err(|e| anyhow!("Failed to serialize: {}", e))
    }
}

/// 执行追踪器
pub struct ExecutionTracer {
    traces: Vec<ExecutionTrace>,
}

/// 执行追踪记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    pub trace_id: String,
    pub chain_id: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub events: Vec<TraceEvent>,
    pub variables: HashMap<String, Value>,
}

/// 追踪事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: TraceEventType,
    pub step_id: Option<String>,
    pub details: HashMap<String, Value>,
}

/// 追踪事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TraceEventType {
    StepStart,
    StepComplete,
    StepError,
    VariableSet,
    BranchTaken,
    ParallelStart,
    ParallelJoin,
    CompensationStart,
    CompensationComplete,
}

impl ExecutionTracer {
    pub fn new() -> Self {
        Self {
            traces: Vec::new(),
        }
    }

    /// 开始追踪
    pub fn start_trace(&mut self, chain_id: &str) -> String {
        let trace_id = format!("trace_{}", uuid::Uuid::new_v4());
        
        self.traces.push(ExecutionTrace {
            trace_id: trace_id.clone(),
            chain_id: chain_id.to_string(),
            start_time: Utc::now(),
            end_time: None,
            events: Vec::new(),
            variables: HashMap::new(),
        });

        trace_id
    }

    /// 记录事件
    pub fn log_event(&mut self, trace_id: &str, event: TraceEvent) -> Result<()> {
        if let Some(trace) = self.traces.iter_mut().find(|t| t.trace_id == trace_id) {
            trace.events.push(event);
            Ok(())
        } else {
            Err(anyhow!("Trace not found: {}", trace_id))
        }
    }

    /// 设置变量
    pub fn set_variable(&mut self, trace_id: &str, key: &str, value: Value) -> Result<()> {
        if let Some(trace) = self.traces.iter_mut().find(|t| t.trace_id == trace_id) {
            trace.variables.insert(key.to_string(), value);
            Ok(())
        } else {
            Err(anyhow!("Trace not found: {}", trace_id))
        }
    }

    /// 结束追踪
    pub fn end_trace(&mut self, trace_id: &str) -> Result<()> {
        if let Some(trace) = self.traces.iter_mut().find(|t| t.trace_id == trace_id) {
            trace.end_time = Some(Utc::now());
            Ok(())
        } else {
            Err(anyhow!("Trace not found: {}", trace_id))
        }
    }

    /// 导出追踪为 JSON
    pub fn export_trace(&self, trace_id: &str) -> Result<String> {
        let trace = self.traces.iter()
            .find(|t| t.trace_id == trace_id)
            .ok_or_else(|| anyhow!("Trace not found: {}", trace_id))?;

        serde_json::to_string_pretty(trace)
            .map_err(|e| anyhow!("Failed to serialize: {}", e))
    }

    /// 生成执行回放
    pub fn generate_playback(&self, trace_id: &str) -> Result<ExecutionPlayback> {
        let trace = self.traces.iter()
            .find(|t| t.trace_id == trace_id)
            .ok_or_else(|| anyhow!("Trace not found: {}", trace_id))?;

        let frames: Vec<PlaybackFrame> = trace.events.iter().map(|event| {
            PlaybackFrame {
                timestamp: event.timestamp,
                step_id: event.step_id.clone(),
                event_type: format!("{:?}", event.event_type),
                variables: trace.variables.clone(),
                details: event.details.clone(),
            }
        }).collect();

        Ok(ExecutionPlayback {
            trace_id: trace_id.to_string(),
            chain_id: trace.chain_id.clone(),
            frames,
        })
    }
}

impl Default for ExecutionTracer {
    fn default() -> Self {
        Self::new()
    }
}

/// 执行回放
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPlayback {
    pub trace_id: String,
    pub chain_id: String,
    pub frames: Vec<PlaybackFrame>,
}

/// 回放帧
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackFrame {
    pub timestamp: DateTime<Utc>,
    pub step_id: Option<String>,
    pub event_type: String,
    pub variables: HashMap<String, Value>,
    pub details: HashMap<String, Value>,
}

/// 性能分析器
pub struct PerformanceAnalyzer;

/// 性能报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub chain_id: String,
    pub total_duration_ms: u64,
    pub step_performance: Vec<StepPerformance>,
    pub bottlenecks: Vec<Bottleneck>,
    pub recommendations: Vec<String>,
}

/// 步骤性能
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepPerformance {
    pub step_id: String,
    pub skill_name: String,
    pub execution_count: u32,
    pub avg_duration_ms: u64,
    pub min_duration_ms: u64,
    pub max_duration_ms: u64,
    pub total_duration_ms: u64,
    pub error_rate: f64,
}

/// 瓶颈分析
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bottleneck {
    pub step_id: String,
    pub severity: BottleneckSeverity,
    pub description: String,
    pub impact_ms: u64,
}

/// 瓶颈严重程度
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BottleneckSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl PerformanceAnalyzer {
    /// 分析执行追踪
    pub fn analyze(traces: &[ExecutionTrace]) -> PerformanceReport {
        let mut step_stats: HashMap<String, Vec<u64>> = HashMap::new();
        let mut step_errors: HashMap<String, u32> = HashMap::new();
        let mut step_names: HashMap<String, String> = HashMap::new();

        for trace in traces {
            for event in &trace.events {
                if let Some(ref step_id) = event.step_id {
                    if let Some(duration) = event.details.get("duration_ms").and_then(|v| v.as_u64()) {
                        step_stats.entry(step_id.clone()).or_default().push(duration);
                    }
                    
                    if matches!(event.event_type, TraceEventType::StepError) {
                        *step_errors.entry(step_id.clone()).or_default() += 1;
                    }

                    if let Some(skill_name) = event.details.get("skill_name").and_then(|v| v.as_str()) {
                        step_names.insert(step_id.clone(), skill_name.to_string());
                    }
                }
            }
        }

        let mut step_performance = Vec::new();
        let mut bottlenecks = Vec::new();
        let mut total_duration = 0u64;

        for (step_id, durations) in &step_stats {
            if durations.is_empty() {
                continue;
            }

            let count = durations.len() as u64;
            let total: u64 = durations.iter().sum();
            let avg = total / count;
            let min = *durations.iter().min().unwrap_or(&0);
            let max = *durations.iter().max().unwrap_or(&0);

            total_duration += total;

            let error_count = step_errors.get(step_id).copied().unwrap_or(0);
            let error_rate = error_count as f64 / count as f64;

            step_performance.push(StepPerformance {
                step_id: step_id.clone(),
                skill_name: step_names.get(step_id).cloned().unwrap_or_default(),
                execution_count: count as u32,
                avg_duration_ms: avg,
                min_duration_ms: min,
                max_duration_ms: max,
                total_duration_ms: total,
                error_rate,
            });

            // 识别瓶颈
            if avg > 5000 {
                bottlenecks.push(Bottleneck {
                    step_id: step_id.clone(),
                    severity: if avg > 30000 { BottleneckSeverity::Critical } else { BottleneckSeverity::High },
                    description: format!("Step takes {}ms on average", avg),
                    impact_ms: total,
                });
            }
        }

        // 生成建议
        let mut recommendations = Vec::new();
        
        if bottlenecks.iter().any(|b| matches!(b.severity, BottleneckSeverity::Critical)) {
            recommendations.push("Consider optimizing critical bottleneck steps".to_string());
        }

        if step_performance.iter().any(|s| s.error_rate > 0.1) {
            recommendations.push("High error rate detected - review error handling".to_string());
        }

        PerformanceReport {
            chain_id: traces.first().map(|t| t.chain_id.clone()).unwrap_or_default(),
            total_duration_ms: total_duration,
            step_performance,
            bottlenecks,
            recommendations,
        }
    }
}

/// 调试器
pub struct ChainDebugger {
    breakpoints: Vec<Breakpoint>,
    watch_variables: Vec<String>,
}

/// 断点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breakpoint {
    pub id: String,
    pub chain_id: String,
    pub step_id: String,
    pub condition: Option<String>,
    pub enabled: bool,
}

/// 调试状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugState {
    pub execution_id: String,
    pub current_step: String,
    pub variables: HashMap<String, Value>,
    pub call_stack: Vec<String>,
}

impl ChainDebugger {
    pub fn new() -> Self {
        Self {
            breakpoints: Vec::new(),
            watch_variables: Vec::new(),
        }
    }

    /// 添加断点
    pub fn add_breakpoint(&mut self, chain_id: &str, step_id: &str, condition: Option<String>) -> String {
        let id = format!("bp_{}", uuid::Uuid::new_v4());
        
        self.breakpoints.push(Breakpoint {
            id: id.clone(),
            chain_id: chain_id.to_string(),
            step_id: step_id.to_string(),
            condition,
            enabled: true,
        });

        id
    }

    /// 移除断点
    pub fn remove_breakpoint(&mut self, breakpoint_id: &str) -> bool {
        let len = self.breakpoints.len();
        self.breakpoints.retain(|bp| bp.id != breakpoint_id);
        self.breakpoints.len() < len
    }

    /// 检查是否应该中断
    pub fn should_break(&self, chain_id: &str, step_id: &str, variables: &HashMap<String, Value>) -> bool {
        self.breakpoints.iter().any(|bp| {
            bp.enabled && 
            bp.chain_id == chain_id && 
            bp.step_id == step_id &&
            Self::evaluate_condition(&bp.condition, variables)
        })
    }

    /// 评估条件
    fn evaluate_condition(condition: &Option<String>, _variables: &HashMap<String, Value>) -> bool {
        // 简化实现 - 没有条件或空条件默认触发
        condition.as_ref().map(|c| c.is_empty()).unwrap_or(true)
    }

    /// 添加观察变量
    pub fn watch_variable(&mut self, var_name: &str) {
        if !self.watch_variables.contains(&var_name.to_string()) {
            self.watch_variables.push(var_name.to_string());
        }
    }

    /// 获取观察变量值
    pub fn get_watched_values(&self, variables: &HashMap<String, Value>) -> HashMap<String, Value> {
        self.watch_variables.iter()
            .filter_map(|name| {
                variables.get(name).map(|v| (name.clone(), v.clone()))
            })
            .collect()
    }
}

impl Default for ChainDebugger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mermaid_export() {
        let graph = SkillChainGraph {
            nodes: vec![
                GraphNode {
                    id: "a".to_string(),
                    label: "Start".to_string(),
                    node_type: "skill".to_string(),
                    shape: "box".to_string(),
                    color: "#4CAF50".to_string(),
                    metadata: HashMap::new(),
                },
                GraphNode {
                    id: "b".to_string(),
                    label: "End".to_string(),
                    node_type: "skill".to_string(),
                    shape: "box".to_string(),
                    color: "#4CAF50".to_string(),
                    metadata: HashMap::new(),
                },
            ],
            edges: vec![
                GraphEdge {
                    id: "e1".to_string(),
                    source: "a".to_string(),
                    target: "b".to_string(),
                    label: Some("next".to_string()),
                    style: "solid".to_string(),
                    metadata: HashMap::new(),
                },
            ],
            metadata: HashMap::new(),
        };

        let mermaid = GraphExporter::to_mermaid(&graph).unwrap();
        assert!(mermaid.contains("flowchart TD"));
        assert!(mermaid.contains("a["));
        assert!(mermaid.contains("b["));
    }

    #[test]
    fn test_tracer() {
        let mut tracer = ExecutionTracer::new();
        let trace_id = tracer.start_trace("test_chain");
        
        tracer.log_event(&trace_id, TraceEvent {
            timestamp: Utc::now(),
            event_type: TraceEventType::StepStart,
            step_id: Some("step1".to_string()),
            details: HashMap::new(),
        }).unwrap();

        tracer.end_trace(&trace_id).unwrap();
        
        let json = tracer.export_trace(&trace_id).unwrap();
        assert!(json.contains("test_chain"));
    }
}
