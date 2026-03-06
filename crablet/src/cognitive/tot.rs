use std::sync::Arc;
use std::collections::VecDeque;
use anyhow::{Result, anyhow};
use crate::cognitive::llm::LlmClient;
use crate::types::Message;
use serde::Serialize;
use tracing::{info, debug};

#[derive(Clone, Debug)]
pub struct TotConfig {
    pub max_depth: usize,
    pub branching_factor: usize,
    pub beam_width: usize, // For Beam Search strategy
    pub strategy: SearchStrategy,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SearchStrategy {
    BFS,
    DFS,
    BeamSearch,
}

impl Default for TotConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            branching_factor: 3,
            beam_width: 2,
            strategy: SearchStrategy::BFS,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ThoughtNode {
    pub id: String,
    pub content: String,
    pub parent_id: Option<String>,
    pub score: f32, // 0.0 - 1.0
    pub depth: usize,
    pub children_ids: Vec<String>,
}

pub struct TreeOfThoughts {
    llm: Arc<Box<dyn LlmClient>>,
    config: TotConfig,
}

impl TreeOfThoughts {
    pub fn new(llm: Arc<Box<dyn LlmClient>>, config: TotConfig) -> Self {
        Self { llm, config }
    }

    pub async fn solve(&self, problem: &str) -> Result<String> {
        match self.config.strategy {
            SearchStrategy::BFS => self.solve_bfs(problem).await,
            SearchStrategy::DFS => self.solve_dfs(problem).await,
            SearchStrategy::BeamSearch => self.solve_beam(problem).await,
        }
    }

    async fn solve_bfs(&self, problem: &str) -> Result<String> {
        let root = Self::root(problem);
        let mut queue = VecDeque::new();
        queue.push_back(root);
        let mut best_solution: Option<ThoughtNode> = None;
        let mut best_score = 0.0f32;
        while let Some(node) = queue.pop_front() {
            if node.depth >= self.config.max_depth {
                continue;
            }
            let candidates = self.generate_thoughts(&node, self.config.branching_factor).await?;
            for mut candidate in candidates {
                let score = self.evaluate_thought(&candidate).await?;
                candidate.score = score;
                debug!("BFS Thought: {} | Score: {}", candidate.content, score);
                if score > 0.5 {
                    if score > best_score {
                        best_score = score;
                        best_solution = Some(candidate.clone());
                    }
                    queue.push_back(candidate);
                }
            }
        }
        best_solution.map(|n| n.content).ok_or_else(|| anyhow!("Failed to find a solution with ToT BFS"))
    }

    async fn solve_dfs(&self, problem: &str) -> Result<String> {
        let root = Self::root(problem);
        let mut stack = vec![root];
        let mut best_solution: Option<ThoughtNode> = None;
        let mut best_score = 0.0f32;
        while let Some(node) = stack.pop() {
            if node.depth >= self.config.max_depth {
                continue;
            }
            let candidates = self.generate_thoughts(&node, self.config.branching_factor).await?;
            let mut scored = Vec::new();
            for mut candidate in candidates {
                let score = self.evaluate_thought(&candidate).await?;
                candidate.score = score;
                debug!("DFS Thought: {} | Score: {}", candidate.content, score);
                if score > 0.5 {
                    if score > best_score {
                        best_score = score;
                        best_solution = Some(candidate.clone());
                    }
                    scored.push(candidate);
                }
            }
            scored.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal));
            for candidate in scored {
                stack.push(candidate);
            }
        }
        best_solution.map(|n| n.content).ok_or_else(|| anyhow!("Failed to find a solution with ToT DFS"))
    }

    async fn solve_beam(&self, problem: &str) -> Result<String> {
        let root = Self::root(problem);
        let mut frontier = vec![root];
        let mut best_solution: Option<ThoughtNode> = None;
        let mut best_score = 0.0;
        for depth in 0..self.config.max_depth {
            info!("ToT Beam Depth {}: Frontier size {}", depth, frontier.len());
            let mut next_frontier = Vec::new();
            for node in frontier {
                if node.depth >= self.config.max_depth {
                    continue;
                }
                let candidates = self.generate_thoughts(&node, self.config.branching_factor).await?;
                for mut candidate in candidates {
                    let score = self.evaluate_thought(&candidate).await?;
                    candidate.score = score;
                    debug!("Beam Thought: {} | Score: {}", candidate.content, score);
                    if score > 0.5 {
                        next_frontier.push(candidate.clone());
                        if score > best_score {
                            best_score = score;
                            best_solution = Some(candidate);
                        }
                    }
                }
            }
            next_frontier.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            frontier = next_frontier.into_iter().take(self.config.beam_width).collect();
            if frontier.is_empty() {
                break;
            }
        }
        best_solution.map(|n| n.content).ok_or_else(|| anyhow!("Failed to find a solution with ToT Beam"))
    }

    fn root(problem: &str) -> ThoughtNode {
        ThoughtNode {
            id: uuid::Uuid::new_v4().to_string(),
            content: problem.to_string(),
            parent_id: None,
            score: 1.0,
            depth: 0,
            children_ids: Vec::new(),
        }
    }

    async fn generate_thoughts(&self, parent: &ThoughtNode, n: usize) -> Result<Vec<ThoughtNode>> {
        let prompt = format!(
            "Problem/Context: {}\n\
             Current Thought: {}\n\
             \n\
             Generate {} distinct next steps or thoughts to advance towards the solution. \
             Each thought should be concise. \
             Output format: JSON list of strings.",
             if parent.parent_id.is_none() { "Start" } else { "..." }, 
             parent.content, 
             n
        );

        let response = self.llm.chat_complete(&[Message::user(&prompt)]).await?;
        
        // Naive JSON extraction
        let json_str = extract_json(&response).unwrap_or("[]");
        let thoughts_text: Vec<String> = serde_json::from_str(json_str).unwrap_or_default();
        
        let nodes = thoughts_text.into_iter().map(|content| ThoughtNode {
            id: uuid::Uuid::new_v4().to_string(),
            content,
            parent_id: Some(parent.id.clone()),
            score: 0.0, // To be evaluated
            depth: parent.depth + 1,
            children_ids: Vec::new(),
        }).collect();

        Ok(nodes)
    }

    async fn evaluate_thought(&self, node: &ThoughtNode) -> Result<f32> {
        let prompt = format!(
            "Evaluate the following thought step towards solving the problem.\n\
             Thought: {}\n\
             \n\
             Rate it from 0.0 to 1.0 based on correctness, feasibility, and progress.\n\
             Output ONLY the float number (e.g., 0.85).",
             node.content
        );

        let response = self.llm.chat_complete(&[Message::user(&prompt)]).await?;
        
        // Extract float robustly
        // Try to find a floating point number in the string
        let score = response.chars()
            .filter(|c| c.is_numeric() || *c == '.')
            .collect::<String>()
            .parse::<f32>()
            .unwrap_or(0.5);
            
        Ok(score.clamp(0.0, 1.0))
    }
}

fn extract_json(text: &str) -> Option<&str> {
    let start = text.find('[')?;
    let end = text.rfind(']')?;
    if start <= end {
        Some(&text[start..=end])
    } else {
        None
    }
}
