use std::sync::Arc;
use anyhow::{Result, anyhow};
use crate::cognitive::llm::LlmClient;
use crate::types::Message;

#[derive(Clone, Debug)]
pub struct MCTSConfig {
    pub simulations: usize,
    pub max_depth: usize,
    pub branching_factor: usize,
    pub exploration_weight: f64,
}

impl Default for MCTSConfig {
    fn default() -> Self {
        Self {
            simulations: 24,
            max_depth: 4,
            branching_factor: 3,
            exploration_weight: 1.2,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ThoughtState {
    pub content: String,
    pub depth: usize,
}

#[derive(Clone, Debug)]
struct MCTSNode {
    state: ThoughtState,
    visits: u32,
    total_value: f64,
    children: Vec<usize>,
    parent: Option<usize>,
    pending_expansions: Vec<String>,
}

impl MCTSNode {
    fn new(content: String, depth: usize, parent: Option<usize>) -> Self {
        Self {
            state: ThoughtState { content, depth },
            visits: 0,
            total_value: 0.0,
            children: Vec::new(),
            parent,
            pending_expansions: Vec::new(),
        }
    }

    fn ucb1_score(&self, parent_visits: u32, exploration_weight: f64) -> f64 {
        if self.visits == 0 {
            return f64::INFINITY;
        }
        let exploitation = self.total_value / self.visits as f64;
        let exploration = (2.0 * (parent_visits as f64 + 1.0).ln() / self.visits as f64).sqrt();
        exploitation + exploration_weight * exploration
    }
}

pub struct MCTSTreeOfThoughts {
    llm: Arc<Box<dyn LlmClient>>,
    config: MCTSConfig,
}

impl MCTSTreeOfThoughts {
    pub fn new(llm: Arc<Box<dyn LlmClient>>, config: MCTSConfig) -> Self {
        Self { llm, config }
    }

    pub async fn solve(&self, problem: &str) -> Result<String> {
        let mut nodes = vec![MCTSNode::new(problem.to_string(), 0, None)];
        for _ in 0..self.config.simulations {
            let selected = self.select(&nodes);
            let expanded = self.expand(selected, &mut nodes).await?;
            let value = self.simulate(&nodes[expanded]).await?;
            self.backpropagate(expanded, value, &mut nodes);
        }
        let best_idx = self.best_terminal(&nodes)?;
        Ok(nodes[best_idx].state.content.clone())
    }

    fn select(&self, nodes: &[MCTSNode]) -> usize {
        let mut current = 0usize;
        loop {
            if nodes[current].children.is_empty() {
                return current;
            }
            if !nodes[current].pending_expansions.is_empty() {
                return current;
            }
            let parent_visits = nodes[current].visits.max(1);
            let mut best_child = nodes[current].children[0];
            let mut best_score = f64::MIN;
            for &child_idx in &nodes[current].children {
                let score = nodes[child_idx].ucb1_score(parent_visits, self.config.exploration_weight);
                if score > best_score {
                    best_score = score;
                    best_child = child_idx;
                }
            }
            current = best_child;
            if nodes[current].state.depth >= self.config.max_depth {
                return current;
            }
        }
    }

    async fn expand(&self, node_idx: usize, nodes: &mut Vec<MCTSNode>) -> Result<usize> {
        if nodes[node_idx].state.depth >= self.config.max_depth {
            return Ok(node_idx);
        }
        if nodes[node_idx].pending_expansions.is_empty() {
            let candidates = self.generate_candidates(&nodes[node_idx].state.content).await?;
            nodes[node_idx].pending_expansions = candidates;
        }
        if let Some(next_content) = nodes[node_idx].pending_expansions.pop() {
            let child_depth = nodes[node_idx].state.depth + 1;
            let child_idx = nodes.len();
            nodes.push(MCTSNode::new(next_content, child_depth, Some(node_idx)));
            nodes[node_idx].children.push(child_idx);
            return Ok(child_idx);
        }
        if nodes[node_idx].children.is_empty() {
            return Ok(node_idx);
        }
        let parent_visits = nodes[node_idx].visits.max(1);
        let mut best_child = nodes[node_idx].children[0];
        let mut best_score = f64::MIN;
        for &child_idx in &nodes[node_idx].children {
            let score = nodes[child_idx].ucb1_score(parent_visits, self.config.exploration_weight);
            if score > best_score {
                best_score = score;
                best_child = child_idx;
            }
        }
        Ok(best_child)
    }

    async fn simulate(&self, node: &MCTSNode) -> Result<f64> {
        let prompt = format!(
            "评估以下思路对解决问题的价值。\n思路：{}\n只输出0.0到1.0之间的小数。",
            node.state.content
        );
        let response = self.llm.chat_complete(&[Message::user(prompt)]).await?;
        let value = response
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.')
            .collect::<String>()
            .parse::<f64>()
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);
        Ok(value)
    }

    fn backpropagate(&self, mut node_idx: usize, value: f64, nodes: &mut [MCTSNode]) {
        loop {
            nodes[node_idx].visits += 1;
            nodes[node_idx].total_value += value;
            if let Some(parent) = nodes[node_idx].parent {
                node_idx = parent;
            } else {
                break;
            }
        }
    }

    fn best_terminal(&self, nodes: &[MCTSNode]) -> Result<usize> {
        let mut best_idx = None;
        let mut best_visits = 0u32;
        for (idx, node) in nodes.iter().enumerate() {
            if node.children.is_empty() && node.state.depth > 0 && node.visits >= best_visits {
                best_visits = node.visits;
                best_idx = Some(idx);
            }
        }
        best_idx.ok_or_else(|| anyhow!("MCTS failed to produce terminal thought"))
    }

    async fn generate_candidates(&self, content: &str) -> Result<Vec<String>> {
        let prompt = format!(
            "问题上下文：{}\n请生成{}个下一步推理分支。输出JSON字符串数组。",
            content, self.config.branching_factor
        );
        let response = self.llm.chat_complete(&[Message::user(prompt)]).await?;
        let list = extract_json_array(&response)
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default();
        if list.is_empty() {
            return Ok(vec![content.to_string()]);
        }
        Ok(list)
    }
}

fn extract_json_array(text: &str) -> Option<&str> {
    let start = text.find('[')?;
    let end = text.rfind(']')?;
    if start <= end {
        Some(&text[start..=end])
    } else {
        None
    }
}
