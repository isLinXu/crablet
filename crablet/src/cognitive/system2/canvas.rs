use crate::events::EventBus;
use crate::events::AgentEvent;

pub fn detect_and_publish_canvas(response: &str, event_bus: &EventBus) {
    // Simple heuristics for artifacts
    // 1. Mermaid Diagrams
    if response.contains("```mermaid") {
        if let Some(start) = response.find("```mermaid") {
            let rest = &response[start..];
            if let Some(end_code) = rest[10..].find("```") {
                let content = &rest[10..10+end_code];
                event_bus.publish(AgentEvent::CanvasUpdate {
                    title: "Diagram Generated".to_string(),
                    content: content.trim().to_string(),
                    kind: "mermaid".to_string(),
                });
            }
        }
    }
    
    // 2. HTML Previews (e.g. for UI mockups)
    if response.contains("```html") {
         if let Some(start) = response.find("```html") {
            let rest = &response[start..];
            if let Some(end_code) = rest[7..].find("```") {
                let content = &rest[7..7+end_code];
                // Heuristic: Only publish if it looks like a full page or component
                if content.contains("<div") || content.contains("<html") || content.contains("<body") {
                    event_bus.publish(AgentEvent::CanvasUpdate {
                        title: "HTML Preview".to_string(),
                        content: content.trim().to_string(),
                        kind: "html".to_string(),
                    });
                }
            }
        }
    }
    
    // 3. Significant Code Blocks (Rust/Python)
    // Iterate over types to find the longest block
    for lang in ["rust", "python", "javascript", "typescript", "json", "toml"] {
        let tag = format!("```{}", lang);
        if response.contains(&tag) {
             if let Some(start) = response.find(&tag) {
                let offset = tag.len();
                let rest = &response[start..];
                if let Some(end_code) = rest[offset..].find("```") {
                    let content = &rest[offset..offset+end_code];
                    // Only publish if significant length (> 5 lines or > 100 chars)
                    if content.lines().count() > 5 || content.len() > 100 {
                        event_bus.publish(AgentEvent::CanvasUpdate {
                            title: format!("{} Snippet", lang.to_uppercase()),
                            content: content.trim().to_string(),
                            kind: "code".to_string(),
                        });
                    }
                }
            }
        }
    }
}
