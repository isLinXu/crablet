use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::gateway::canvas::{CanvasState, CanvasComponent};
use crate::events::EventBus;

#[derive(Clone)]
pub struct CanvasManager {
    states: Arc<RwLock<HashMap<String, CanvasState>>>, // session_id -> CanvasState
    event_bus: Arc<EventBus>,
}

impl CanvasManager {
    pub fn new(event_bus: EventBus) -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
            event_bus: Arc::new(event_bus),
        }
    }

    pub async fn get_state(&self, session_id: &str) -> Option<CanvasState> {
        let states = self.states.read().await;
        states.get(session_id).cloned()
    }

    pub async fn create_or_get_state(&self, session_id: &str) -> CanvasState {
        let mut states = self.states.write().await;
        if let Some(state) = states.get(session_id) {
            return state.clone();
        }
        
        let state = CanvasState::new(session_id.to_string());
        states.insert(session_id.to_string(), state.clone());
        state
    }

    pub async fn add_section(&self, session_id: &str, section_id: &str, title: &str) {
        let mut states = self.states.write().await;
        let state = states.entry(session_id.to_string())
            .or_insert_with(|| CanvasState::new(session_id.to_string()));
            
        state.add_section(section_id.to_string(), title.to_string());
        
        // Broadcast update
        /* 
        let _ = self.event_bus.publish(crate::events::AgentEvent::CanvasUpdate {
             title: "Canvas Update".to_string(),
             content: format!("Section Added: {}", title),
             kind: "markdown".to_string(),
        });
        */
    }

    pub async fn add_component(&self, session_id: &str, section_id: &str, component: CanvasComponent) {
        let mut states = self.states.write().await;
        let state = states.entry(session_id.to_string())
            .or_insert_with(|| CanvasState::new(session_id.to_string()));
            
        state.add_component(section_id, component.clone());
        
        // Broadcast update
        /*
        let _ = self.event_bus.publish(crate::events::AgentEvent::CanvasUpdate {
             title: "Canvas Update".to_string(),
             content: format!("Component Added to {}", section_id),
             kind: "markdown".to_string(),
        });
        */
    }

    pub async fn update_component_state(&self, session_id: &str, section_id: &str, index: usize, component: CanvasComponent) {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(session_id) {
            state.update_component(section_id, index, component.clone());
            
            // Broadcast update
            /*
            let _ = self.event_bus.publish(crate::events::AgentEvent::CanvasUpdate {
                 title: "Canvas Update".to_string(),
                 content: format!("Component Updated in {}", section_id),
                 kind: "markdown".to_string(),
            });
            */
        }
    }

    pub async fn remove_component(&self, session_id: &str, section_id: &str, index: usize) {
        let mut states = self.states.write().await;
        if let Some(state) = states.get_mut(session_id) {
            state.remove_component(section_id, index);
            
            // Broadcast update
            /*
            let _ = self.event_bus.publish(crate::events::AgentEvent::CanvasUpdate {
                 title: "Canvas Update".to_string(),
                 content: format!("Component Removed from {}", section_id),
                 kind: "markdown".to_string(),
            });
            */
        }
    }
}
