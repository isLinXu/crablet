use std::collections::VecDeque;
use crate::types::Message;

pub struct WorkingMemory {
    capacity: usize,
    history: VecDeque<Message>,
}

impl WorkingMemory {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            history: VecDeque::with_capacity(capacity),
        }
    }

    pub fn add_message(&mut self, role: &str, content: &str) {
        // Simple sliding window compression
        if self.history.len() >= self.capacity {
            // Instead of just popping, let's try to preserve the system message if it exists at index 0
            if !self.history.is_empty() && self.history[0].role == "system" {
                if self.history.len() > 1 {
                    self.history.remove(1); // Remove oldest non-system message
                } else {
                    self.history.pop_front();
                }
            } else {
                self.history.pop_front();
            }
        }
        self.history.push_back(Message::new(role, content));
    }
    
    pub fn compress_context(&mut self) {
        // A simple compression strategy: 
        // If history is full, summarize the first half (excluding system prompt) into one message?
        // For now, we implemented a smarter sliding window in add_message.
        // This function can be expanded later for LLM-based summarization.
        if self.history.len() > self.capacity {
             while self.history.len() > self.capacity {
                 if !self.history.is_empty() && self.history[0].role == "system" {
                     if self.history.len() > 1 {
                         self.history.remove(1);
                     } else {
                         self.history.pop_front();
                     }
                 } else {
                     self.history.pop_front();
                 }
             }
        }
    }
    
    pub fn clear(&mut self) {
        self.history.clear();
    }

    pub fn get_context(&self) -> Vec<Message> {
        self.history.iter().cloned().collect()
    }
}
