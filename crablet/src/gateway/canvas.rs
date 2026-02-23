use serde::{Serialize, Deserialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum CanvasComponent {
    #[serde(rename = "markdown")]
    Markdown {
        content: String,
    },
    #[serde(rename = "code")]
    Code {
        language: String,
        content: String,
        filename: Option<String>,
    },
    #[serde(rename = "mermaid")]
    Mermaid {
        chart: String,
    },
    #[serde(rename = "datatable")]
    DataTable {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
        title: Option<String>,
    },
    #[serde(rename = "html")]
    Html {
        content: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasSection {
    pub id: String,
    pub title: String,
    pub components: Vec<CanvasComponent>,
    pub layout: String, // e.g., "column", "row", "grid"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasState {
    pub session_id: String,
    pub sections: HashMap<String, CanvasSection>,
    pub section_order: Vec<String>, // List of section IDs
}

impl CanvasState {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            sections: HashMap::new(),
            section_order: Vec::new(),
        }
    }

    pub fn add_section(&mut self, id: String, title: String) {
        if !self.sections.contains_key(&id) {
            let section = CanvasSection {
                id: id.clone(),
                title,
                components: Vec::new(),
                layout: "column".to_string(),
            };
            self.sections.insert(id.clone(), section);
            self.section_order.push(id);
        }
    }

    pub fn add_component(&mut self, section_id: &str, component: CanvasComponent) {
        if let Some(section) = self.sections.get_mut(section_id) {
            section.components.push(component);
        }
    }
    
    pub fn update_component(&mut self, section_id: &str, index: usize, component: CanvasComponent) {
        if let Some(section) = self.sections.get_mut(section_id) {
            if index < section.components.len() {
                section.components[index] = component;
            }
        }
    }
}
