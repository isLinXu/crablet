use anyhow::Result;
use crate::events::EventBus;
use crate::events::AgentEvent;
use std::sync::Arc;
use colored::Colorize;

pub async fn handle_debug(session_id: &str, event_bus: Arc<EventBus>) -> Result<()> {
    println!("{}", format!("Debugging Session: {}", session_id).bold().blue());
    
    let events = event_bus.replay(session_id).await;
    
    if events.is_empty() {
        println!("{}", "No events found for this session.".yellow());
        return Ok(());
    }
    
    println!("{}", format!("Found {} events:", events.len()).green());
    
    for (i, event) in events.iter().enumerate() {
        print!("{}. ", i + 1);
        match &event.payload {
            AgentEvent::UserInput(s) => println!("USER: {}", s.white()),
            AgentEvent::SystemLog(s) => println!("SYSTEM: {}", s.dimmed()),
            AgentEvent::ThoughtGenerated(s) => println!("THOUGHT: {}", s.cyan()),
            AgentEvent::ToolExecutionStarted { tool, args } => println!("TOOL CALL: {}({})", tool.yellow(), args),
            AgentEvent::ToolExecutionFinished { tool, output } => {
                let out_preview = if output.len() > 100 {
                    format!("{}...", &output[..100])
                } else {
                    output.clone()
                };
                println!("TOOL OUT: {} -> {}", tool.yellow(), out_preview.dimmed());
            },
            AgentEvent::CanvasUpdate { title, kind, .. } => println!("CANVAS: [{}] {}", kind, title.magenta()),
            AgentEvent::SwarmActivity { task_id, from, to, message_type, content, .. } => {
                println!("SWARM [{}]: {} -> {} ({}): {}", task_id, from.blue(), to.blue(), message_type.yellow(), content.dimmed());
            },
            AgentEvent::SwarmGraphUpdate { graph_id, status, .. } => {
                println!("SWARM GRAPH [{}]: Status -> {}", graph_id, status.yellow());
            },
            AgentEvent::SwarmTaskUpdate { graph_id, task_id, status, .. } => {
                println!("SWARM TASK [{}/{}]: Status -> {}", graph_id, task_id, status.yellow());
            },
            AgentEvent::SwarmLog { graph_id, task_id, content, .. } => {
                println!("SWARM LOG [{}/{}]: {}", graph_id, task_id, content.dimmed());
            },
            AgentEvent::GraphRagEntityModeChanged { from_mode, to_mode } => {
                println!("GRAPH_RAG MODE: {} -> {}", from_mode.yellow(), to_mode.yellow());
            },
            AgentEvent::ResponseGenerated(s) => println!("RESPONSE: {}", s.green()),
            AgentEvent::CognitiveLayerChanged { layer } => println!("COGNITIVE: {}", layer.magenta()),
            AgentEvent::Error(s) => println!("ERROR: {}", s.red()),
        }
    }
    
    Ok(())
}
