use crate::agent::{Agent, AgentRole};
use crate::cognitive::llm::LlmClient;
use crate::tools::search::WebSearchTool;
use crate::types::Message;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::info;

use crate::agent::swarm::{AgentId, SwarmAgent, SwarmMessage};

use crate::events::{AgentEvent, EventBus};

#[derive(Clone)]
pub struct ResearchAgent {
    id: AgentId,
    llm: Arc<Box<dyn LlmClient>>,
    search: Arc<WebSearchTool>,
    event_bus: Arc<EventBus>,
}

impl ResearchAgent {
    pub fn new(llm: Arc<Box<dyn LlmClient>>, event_bus: Arc<EventBus>) -> Self {
        Self {
            id: AgentId::from_name("researcher"),
            llm,
            search: Arc::new(WebSearchTool::new()),
            event_bus,
        }
    }
}

#[async_trait]
impl SwarmAgent for ResearchAgent {
    fn id(&self) -> &AgentId {
        &self.id
    }

    fn name(&self) -> &str {
        "researcher"
    }

    async fn receive(&mut self, message: SwarmMessage, sender: AgentId) -> Option<SwarmMessage> {
        match message {
            SwarmMessage::Task {
                task_id,
                description,
                context,
                ..
            } => {
                info!("ResearchAgent received task {} from {}", task_id, sender.0);
                match self.execute(&description, &context).await {
                    Ok(result) => Some(SwarmMessage::Result {
                        task_id,
                        content: result,
                        payload: None,
                    }),
                    Err(e) => Some(SwarmMessage::Error {
                        task_id,
                        error: e.to_string(),
                    }),
                }
            }
            _ => None,
        }
    }
}

#[async_trait]
impl Agent for ResearchAgent {
    fn name(&self) -> &str {
        "researcher"
    }

    fn role(&self) -> AgentRole {
        AgentRole::Researcher
    }

    fn description(&self) -> &str {
        "A specialist agent for deep web research and summarization"
    }

    async fn execute(&self, task: &str, _context: &[Message]) -> Result<String> {
        let msg = format!("ResearchAgent starting task: {}", task);
        info!("{}", msg);
        self.event_bus.publish(AgentEvent::SystemLog(msg.clone()));
        self.event_bus.publish(AgentEvent::ThoughtGenerated(format!(
            "I need to research '{}'. I will generate search queries first.",
            task
        )));

        // 1. Generate search queries (Limit to 3 to prevent infinite loops/cost explosion)
        const MAX_QUERIES: usize = 3;

        let prompt = format!(
            "Generate {} distinct search queries to research the following topic:\nTopic: {}\nReturn ONLY the queries, one per line.",
            MAX_QUERIES, task
        );
        let query_resp = self
            .llm
            .chat_complete(&[Message::new("user", &prompt)])
            .await?;

        // Clean and de-duplicate queries before searching.
        const RESULTS_PER_QUERY: usize = 3;
        let mut queries: Vec<String> = Vec::new();
        for line in query_resp.lines() {
            let cleaned = line.trim().trim_start_matches("- ").trim().to_string();
            if cleaned.is_empty() || queries.iter().any(|q| q.eq_ignore_ascii_case(&cleaned)) {
                continue;
            }
            queries.push(cleaned);
            if queries.len() >= MAX_QUERIES {
                break;
            }
        }

        // 2. Perform searches CONCURRENTLY (fan-out), each query independent.
        for q in &queries {
            self.event_bus.publish(AgentEvent::ToolExecutionStarted {
                tool: "web_search".to_string(),
                args: q.clone(),
            });
        }

        let search_futures = queries.iter().map(|q| {
            let search = self.search.clone();
            let query = q.clone();
            async move {
                let result = search.search(&query).await;
                (query, result)
            }
        });
        let search_outcomes = futures::future::join_all(search_futures).await;

        // 3. Aggregate findings, de-duplicating sources across queries.
        let mut findings = String::new();
        let mut seen_sources: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut success_count = 0;

        for (q, outcome) in search_outcomes {
            match outcome {
                Ok(results) if !results.is_empty() => {
                    success_count += 1;
                    let top_snippet = results
                        .first()
                        .map(|r| r.snippet.clone())
                        .unwrap_or_default();

                    self.event_bus.publish(AgentEvent::ToolExecutionFinished {
                        tool: "web_search".to_string(),
                        output: format!(
                            "[{}] Found {} results. Top snippet: {}...",
                            q,
                            results.len(),
                            top_snippet.chars().take(100).collect::<String>()
                        ),
                    });

                    // Stream intermediate finding to UI
                    let partial_finding = format!(
                        "Found relevant info for query '{}':\n{}\n\n",
                        q,
                        top_snippet.chars().take(200).collect::<String>()
                    );
                    self.event_bus
                        .publish(AgentEvent::SystemLog(partial_finding));

                    for res in results.iter().take(RESULTS_PER_QUERY) {
                        // Skip duplicate sources to keep the evidence set diverse.
                        if !res.link.is_empty() && !seen_sources.insert(res.link.clone()) {
                            continue;
                        }
                        findings.push_str(&format!(
                            "Source: {}\nTitle: {}\nSnippet: {}\n\n",
                            res.link, res.title, res.snippet
                        ));
                    }
                }
                _ => {
                    self.event_bus.publish(AgentEvent::ToolExecutionFinished {
                        tool: "web_search".to_string(),
                        output: format!("[{}] No results or search failed", q),
                    });
                }
            }
        }

        if success_count == 0 {
            return Ok("I attempted to research this topic but could not find any relevant information via web search. Please try refining your request.".to_string());
        }

        // 3. Summarize findings
        self.event_bus.publish(AgentEvent::ThoughtGenerated(
            "Summarizing findings...".to_string(),
        ));
        let summary_prompt = format!(
            "Based on the following search results, write a comprehensive summary about '{}'.\n\nSearch Results:\n{}",
            task, findings
        );

        info!(
            "Sending summary request to LLM. Prompt length: {} chars",
            summary_prompt.len()
        );

        let summary_msg = vec![Message::new("user", &summary_prompt)];
        let summary = self.llm.chat_complete(&summary_msg).await?;
        let report = format!("**Research Report**\n\n{}", summary);

        // Auto-save the report to a file
        let filename = format!(
            "research_report_{}.md",
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        );
        if tokio::fs::write(&filename, &report).await.is_ok() {
            self.event_bus.publish(AgentEvent::SystemLog(format!(
                "Report saved to {}",
                filename
            )));
        }

        Ok(report)
    }
}
