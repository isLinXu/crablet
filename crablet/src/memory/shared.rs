use std::sync::Arc;
use dashmap::DashMap;
use std::collections::HashMap;

#[derive(Clone)]
pub struct SharedBlackboard {
    data: Arc<DashMap<String, String>>,
}

impl Default for SharedBlackboard {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedBlackboard {
    pub fn new() -> Self {
        Self {
            data: Arc::new(DashMap::new()),
        }
    }

    pub fn get(&self, key: &str) -> Option<String> {
        self.data.get(key).map(|v| v.clone())
    }

    pub fn set(&self, key: String, value: String) {
        self.data.insert(key, value);
    }
    
    pub fn list(&self) -> HashMap<String, String> {
        self.data.iter().map(|r| (r.key().clone(), r.value().clone())).collect()
    }
}

#[derive(Clone, Debug)]
pub struct CrossAgentMessage {
    pub from: String,
    pub content: String,
    pub timestamp: i64,
}
