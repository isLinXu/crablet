//! 思维链可视化图谱 - 支持复杂的推理路径展示
//!
//! 提供树形/图形的思考过程表示，支持分支、回溯、合并等复杂推理模式

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// 思维节点类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ThoughtNodeType {
    /// 推理思考
    Reasoning,
    /// 工具调用
    ToolCall,
    /// 观察结果
    Observation,
    /// 决策点
    Decision,
    /// 反思/自我批评
    Reflection,
    /// 信息检索
    Retrieval,
    /// 规划
    Planning,
    /// 总结
    Summary,
    /// 错误/异常
    Error,
    /// 用户输入
    UserInput,
    /// 系统提示
    SystemPrompt,
}

impl ThoughtNodeType {
    /// 获取节点颜色（用于前端可视化）
    pub fn color(&self) -> &'static str {
        match self {
            ThoughtNodeType::Reasoning => "#3b82f6",      // blue
            ThoughtNodeType::ToolCall => "#f59e0b",       // amber
            ThoughtNodeType::Observation => "#10b981",    // emerald
            ThoughtNodeType::Decision => "#8b5cf6",       // violet
            ThoughtNodeType::Reflection => "#ec4899",     // pink
            ThoughtNodeType::Retrieval => "#06b6d4",      // cyan
            ThoughtNodeType::Planning => "#6366f1",       // indigo
            ThoughtNodeType::Summary => "#14b8a6",        // teal
            ThoughtNodeType::Error => "#ef4444",          // red
            ThoughtNodeType::UserInput => "#84cc16",      // lime
            ThoughtNodeType::SystemPrompt => "#6b7280",   // gray
        }
    }

    /// 获取节点图标
    pub fn icon(&self) -> &'static str {
        match self {
            ThoughtNodeType::Reasoning => "🧠",
            ThoughtNodeType::ToolCall => "🔧",
            ThoughtNodeType::Observation => "👁️",
            ThoughtNodeType::Decision => "🎯",
            ThoughtNodeType::Reflection => "🔄",
            ThoughtNodeType::Retrieval => "🔍",
            ThoughtNodeType::Planning => "📋",
            ThoughtNodeType::Summary => "📝",
            ThoughtNodeType::Error => "❌",
            ThoughtNodeType::UserInput => "💬",
            ThoughtNodeType::SystemPrompt => "⚙️",
        }
    }

    /// 获取节点标签
    pub fn label(&self) -> &'static str {
        match self {
            ThoughtNodeType::Reasoning => "推理",
            ThoughtNodeType::ToolCall => "工具调用",
            ThoughtNodeType::Observation => "观察",
            ThoughtNodeType::Decision => "决策",
            ThoughtNodeType::Reflection => "反思",
            ThoughtNodeType::Retrieval => "检索",
            ThoughtNodeType::Planning => "规划",
            ThoughtNodeType::Summary => "总结",
            ThoughtNodeType::Error => "错误",
            ThoughtNodeType::UserInput => "用户输入",
            ThoughtNodeType::SystemPrompt => "系统提示",
        }
    }
}

/// 思维节点状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ThoughtNodeStatus {
    /// 等待中
    Pending,
    /// 进行中
    Processing,
    /// 已完成
    Completed,
    /// 被跳过/放弃
    Skipped,
    /// 失败
    Failed,
    /// 被修正
    Corrected,
}

/// 思维节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThoughtNode {
    /// 节点唯一ID
    pub id: String,
    /// 节点类型
    pub node_type: ThoughtNodeType,
    /// 节点状态
    pub status: ThoughtNodeStatus,
    /// 节点内容
    pub content: String,
    /// 父节点ID列表（支持多父节点，表示合并）
    pub parent_ids: Vec<String>,
    /// 子节点ID列表
    pub child_ids: Vec<String>,
    /// 同级替代节点ID（决策分支的其他选项）
    pub alternative_ids: Vec<String>,
    /// 创建时间戳
    pub created_at: u64,
    /// 完成时间戳
    pub completed_at: Option<u64>,
    /// 执行耗时（毫秒）
    pub duration_ms: Option<u64>,
    /// 置信度分数 (0.0 - 1.0)
    pub confidence: Option<f32>,
    /// 信息增益分数
    pub information_gain: Option<f32>,
    /// 元数据
    pub metadata: HashMap<String, serde_json::Value>,
    /// 节点层级（深度）
    pub depth: u32,
    /// 分支ID（用于区分不同分支路径）
    pub branch_id: Option<String>,
}

impl ThoughtNode {
    /// 创建新节点
    pub fn new(node_type: ThoughtNodeType, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            node_type,
            status: ThoughtNodeStatus::Pending,
            content: content.into(),
            parent_ids: Vec::new(),
            child_ids: Vec::new(),
            alternative_ids: Vec::new(),
            created_at: chrono::Utc::now().timestamp_millis() as u64,
            completed_at: None,
            duration_ms: None,
            confidence: None,
            information_gain: None,
            metadata: HashMap::new(),
            depth: 0,
            branch_id: None,
        }
    }

    /// 设置父节点
    pub fn with_parent(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_ids.push(parent_id.into());
        self
    }

    /// 设置分支ID
    pub fn with_branch(mut self, branch_id: impl Into<String>) -> Self {
        self.branch_id = Some(branch_id.into());
        self
    }

    /// 设置置信度
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = Some(confidence.clamp(0.0, 1.0));
        self
    }

    /// 添加元数据
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// 标记为完成
    pub fn complete(&mut self) {
        self.status = ThoughtNodeStatus::Completed;
        self.completed_at = Some(chrono::Utc::now().timestamp_millis() as u64);
        if let Some(created) = self.created_at.checked_sub(0) {
            self.duration_ms = self.completed_at.map(|c| c - created);
        }
    }

    /// 标记为失败
    pub fn fail(&mut self) {
        self.status = ThoughtNodeStatus::Failed;
        self.completed_at = Some(chrono::Utc::now().timestamp_millis() as u64);
    }
}

/// 思维连接（边）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThoughtEdge {
    /// 边的唯一ID
    pub id: String,
    /// 源节点ID
    pub source: String,
    /// 目标节点ID
    pub target: String,
    /// 边的类型
    pub edge_type: EdgeType,
    /// 边的标签
    pub label: Option<String>,
    /// 边的权重（用于可视化）
    pub weight: f32,
}

/// 边类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    /// 顺序执行
    Sequential,
    /// 分支（决策）
    Branch,
    /// 合并
    Merge,
    /// 回溯
    Backtrack,
    /// 引用
    Reference,
    /// 修正
    Correction,
}

impl EdgeType {
    /// 获取边的样式
    pub fn style(&self) -> &'static str {
        match self {
            EdgeType::Sequential => "solid",
            EdgeType::Branch => "dashed",
            EdgeType::Merge => "solid",
            EdgeType::Backtrack => "dotted",
            EdgeType::Reference => "solid",
            EdgeType::Correction => "dashed",
        }
    }
}

/// 思维图谱
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThoughtGraph {
    /// 图谱ID
    pub id: String,
    /// 根节点ID
    pub root_id: String,
    /// 所有节点
    pub nodes: HashMap<String, ThoughtNode>,
    /// 所有边
    pub edges: Vec<ThoughtEdge>,
    /// 当前活跃节点ID
    pub active_node_id: Option<String>,
    /// 创建时间
    pub created_at: u64,
    /// 最后更新时间
    pub updated_at: u64,
}

impl ThoughtGraph {
    /// 创建新的思维图谱
    pub fn new(root_content: impl Into<String>) -> Self {
        let root = ThoughtNode::new(ThoughtNodeType::SystemPrompt, root_content);
        let root_id = root.id.clone();
        let now = chrono::Utc::now().timestamp_millis() as u64;
        
        let mut nodes = HashMap::new();
        nodes.insert(root_id.clone(), root);
        
        Self {
            id: Uuid::new_v4().to_string(),
            root_id,
            nodes,
            edges: Vec::new(),
            active_node_id: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// 添加节点
    pub fn add_node(&mut self, mut node: ThoughtNode) -> String {
        // 计算深度
        if !node.parent_ids.is_empty() {
            let parent_depth = node.parent_ids.iter()
                .filter_map(|pid| self.nodes.get(pid))
                .map(|p| p.depth)
                .max()
                .unwrap_or(0);
            node.depth = parent_depth + 1;
        }
        
        let id = node.id.clone();
        
        // 更新父节点的子节点列表
        for parent_id in &node.parent_ids {
            if let Some(parent) = self.nodes.get_mut(parent_id) {
                if !parent.child_ids.contains(&id) {
                    parent.child_ids.push(id.clone());
                }
            }
        }
        
        self.nodes.insert(id.clone(), node);
        self.updated_at = chrono::Utc::now().timestamp_millis() as u64;
        id
    }

    /// 添加边
    pub fn add_edge(&mut self, edge: ThoughtEdge) {
        self.edges.push(edge);
        self.updated_at = chrono::Utc::now().timestamp_millis() as u64;
    }

    /// 创建顺序边
    pub fn connect_sequential(&mut self, from: &str, to: &str) {
        let edge = ThoughtEdge {
            id: Uuid::new_v4().to_string(),
            source: from.to_string(),
            target: to.to_string(),
            edge_type: EdgeType::Sequential,
            label: None,
            weight: 1.0,
        };
        self.add_edge(edge);
    }

    /// 创建分支边
    pub fn connect_branch(&mut self, from: &str, to: &str, label: impl Into<String>) {
        let edge = ThoughtEdge {
            id: Uuid::new_v4().to_string(),
            source: from.to_string(),
            target: to.to_string(),
            edge_type: EdgeType::Branch,
            label: Some(label.into()),
            weight: 1.0,
        };
        self.add_edge(edge);
    }

    /// 设置活跃节点
    pub fn set_active(&mut self, node_id: &str) {
        self.active_node_id = Some(node_id.to_string());
        self.updated_at = chrono::Utc::now().timestamp_millis() as u64;
    }

    /// 获取从根到当前活跃节点的路径
    pub fn get_active_path(&self) -> Vec<&ThoughtNode> {
        let mut path = Vec::new();
        
        if let Some(active_id) = &self.active_node_id {
            let mut current_id = Some(active_id.as_str());
            
            while let Some(id) = current_id {
                if let Some(node) = self.nodes.get(id) {
                    path.push(node);
                    current_id = node.parent_ids.first().map(|s| s.as_str());
                } else {
                    break;
                }
            }
        }
        
        path.reverse();
        path
    }

    /// 获取指定深度的所有节点
    pub fn get_nodes_at_depth(&self, depth: u32) -> Vec<&ThoughtNode> {
        self.nodes.values()
            .filter(|n| n.depth == depth)
            .collect()
    }

    /// 获取图谱统计信息
    pub fn get_stats(&self) -> ThoughtGraphStats {
        let total_nodes = self.nodes.len();
        let completed_nodes = self.nodes.values()
            .filter(|n| n.status == ThoughtNodeStatus::Completed)
            .count();
        let failed_nodes = self.nodes.values()
            .filter(|n| n.status == ThoughtNodeStatus::Failed)
            .count();
        let max_depth = self.nodes.values()
            .map(|n| n.depth)
            .max()
            .unwrap_or(0);
        
        let total_duration: u64 = self.nodes.values()
            .filter_map(|n| n.duration_ms)
            .sum();
        
        let avg_confidence = self.nodes.values()
            .filter_map(|n| n.confidence)
            .fold(0.0, |acc, c| acc + c) 
            / self.nodes.values().filter(|n| n.confidence.is_some()).count().max(1) as f32;

        ThoughtGraphStats {
            total_nodes,
            completed_nodes,
            failed_nodes,
            max_depth,
            total_duration_ms: total_duration,
            average_confidence: if avg_confidence > 0.0 { Some(avg_confidence) } else { None },
            branch_count: self.edges.iter().filter(|e| e.edge_type == EdgeType::Branch).count(),
        }
    }

    /// 导出为 Mermaid 图表语法
    pub fn to_mermaid(&self) -> String {
        let mut output = String::from("graph TD\n");
        
        // 添加节点定义
        for node in self.nodes.values() {
            let label = format!("{} {}", node.node_type.icon(), 
                node.content.chars().take(30).collect::<String>());
            let style = format!("    {}[\"{}\"]\n", node.id, label);
            output.push_str(&style);
            
            // 添加节点样式
            let color = node.node_type.color();
            output.push_str(&format!("    style {} fill:{}\n", node.id, color));
        }
        
        // 添加边
        for edge in &self.edges {
            let line = match &edge.label {
                Some(label) => format!("    {} -->|{}| {}\n", edge.source, label, edge.target),
                None => format!("    {} --> {}\n", edge.source, edge.target),
            };
            output.push_str(&line);
        }
        
        output
    }
}

/// 思维图谱统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThoughtGraphStats {
    pub total_nodes: usize,
    pub completed_nodes: usize,
    pub failed_nodes: usize,
    pub max_depth: u32,
    pub total_duration_ms: u64,
    pub average_confidence: Option<f32>,
    pub branch_count: usize,
}

/// 思维图谱构建器
pub struct ThoughtGraphBuilder {
    graph: ThoughtGraph,
    current_node_id: String,
}

impl ThoughtGraphBuilder {
    /// 创建新的构建器
    pub fn new(task_description: impl Into<String>) -> Self {
        let graph = ThoughtGraph::new(task_description);
        let root_id = graph.root_id.clone();
        
        Self {
            graph,
            current_node_id: root_id,
        }
    }

    /// 添加推理步骤
    pub fn add_reasoning(&mut self, content: impl Into<String>) -> &mut Self {
        let node = ThoughtNode::new(ThoughtNodeType::Reasoning, content)
            .with_parent(self.current_node_id.clone());
        let id = self.graph.add_node(node);
        self.graph.connect_sequential(&self.current_node_id, &id);
        self.current_node_id = id;
        self
    }

    /// 添加工具调用
    pub fn add_tool_call(&mut self, tool_name: impl Into<String>, params: serde_json::Value) -> &mut Self {
        let content = format!("调用工具: {}", tool_name.into());
        let node = ThoughtNode::new(ThoughtNodeType::ToolCall, content)
            .with_parent(self.current_node_id.clone())
            .with_metadata("tool_params", params);
        let id = self.graph.add_node(node);
        self.graph.connect_sequential(&self.current_node_id, &id);
        self.current_node_id = id;
        self
    }

    /// 添加观察结果
    pub fn add_observation(&mut self, content: impl Into<String>) -> &mut Self {
        let node = ThoughtNode::new(ThoughtNodeType::Observation, content)
            .with_parent(self.current_node_id.clone());
        let id = self.graph.add_node(node);
        self.graph.connect_sequential(&self.current_node_id, &id);
        self.current_node_id = id;
        self
    }

    /// 添加决策点（创建分支）
    pub fn add_decision(&mut self, content: impl Into<String>, choices: Vec<String>) -> DecisionBranch<'_> {
        let decision_node = ThoughtNode::new(ThoughtNodeType::Decision, content)
            .with_parent(self.current_node_id.clone());
        let decision_id = self.graph.add_node(decision_node);
        self.graph.connect_sequential(&self.current_node_id, &decision_id);
        
        DecisionBranch {
            builder: self,
            decision_id,
            choices,
            current_choice_index: 0,
        }
    }

    /// 构建图谱
    pub fn build(self) -> ThoughtGraph {
        self.graph
    }

    /// 获取当前节点ID
    pub fn current_id(&self) -> &str {
        &self.current_node_id
    }
}

/// 决策分支处理器
pub struct DecisionBranch<'a> {
    builder: &'a mut ThoughtGraphBuilder,
    decision_id: String,
    choices: Vec<String>,
    current_choice_index: usize,
}

impl<'a> DecisionBranch<'a> {
    /// 选择分支并继续构建
    pub fn choose(&mut self, choice_index: usize) -> &mut ThoughtGraphBuilder {
        if choice_index < self.choices.len() {
            let choice = &self.choices[choice_index];
            let branch_node = ThoughtNode::new(
                ThoughtNodeType::Reasoning, 
                format!("分支: {}", choice)
            )
            .with_parent(self.decision_id.clone());
            
            let branch_id = self.builder.graph.add_node(branch_node);
            self.builder.graph.connect_branch(&self.decision_id, &branch_id, choice);
            self.builder.current_node_id = branch_id;
        }
        self.builder
    }

    /// 获取所有选项
    pub fn choices(&self) -> &[String] {
        &self.choices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thought_graph_creation() {
        let graph = ThoughtGraph::new("开始任务");
        assert_eq!(graph.nodes.len(), 1);
        assert!(!graph.root_id.is_empty());
    }

    #[test]
    fn test_node_creation() {
        let node = ThoughtNode::new(ThoughtNodeType::Reasoning, "测试推理")
            .with_confidence(0.85);
        
        assert_eq!(node.node_type, ThoughtNodeType::Reasoning);
        assert_eq!(node.confidence, Some(0.85));
        assert_eq!(node.status, ThoughtNodeStatus::Pending);
    }

    #[test]
    fn test_graph_builder() {
        let mut builder = ThoughtGraphBuilder::new("分析代码");
        builder
            .add_reasoning("需要理解代码结构")
            .add_tool_call("file_read", serde_json::json!({"path": "main.rs"}))
            .add_observation("代码包含 3 个模块");
        
        let graph = builder.build();
        assert_eq!(graph.nodes.len(), 4);
        assert_eq!(graph.edges.len(), 3);
    }

    #[test]
    fn test_mermaid_export() {
        let mut graph = ThoughtGraph::new("任务");
        let node1 = ThoughtNode::new(ThoughtNodeType::Reasoning, "步骤1");
        let id1 = graph.add_node(node1);
        let root_id = graph.root_id.clone();
        graph.connect_sequential(&root_id, &id1);
        
        let mermaid = graph.to_mermaid();
        assert!(mermaid.contains("graph TD"));
        assert!(mermaid.contains(&id1));
    }
}
