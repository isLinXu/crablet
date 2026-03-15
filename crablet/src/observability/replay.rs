//! Execution Replay System
//!
//! Allows replaying, stepping through, and forking Agent executions for debugging.

use super::{AgentSpan, ExecutionContext, TraceSession};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// Records an execution for later replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecording {
    pub session: TraceSession,
    pub spans: Vec<AgentSpan>,
    pub checkpoints: Vec<Checkpoint>,
    pub metadata: RecordingMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    pub total_steps: usize,
    pub total_duration_ms: u64,
    pub tool_calls: usize,
    pub errors: usize,
    pub final_output: Option<String>,
}

/// A checkpoint for quick navigation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub step_index: usize,
    pub description: String,
    pub context_snapshot: ExecutionContext,
}

/// Replay controller for recorded executions
pub struct ExecutionReplay {
    recording: ExecutionRecording,
    current_index: usize,
    fork_point: Option<usize>,
    modified_context: Option<ExecutionContext>,
}

impl ExecutionReplay {
    pub fn new(recording: ExecutionRecording) -> Self {
        Self {
            recording,
            current_index: 0,
            fork_point: None,
            modified_context: None,
        }
    }

    /// Get the total number of steps
    pub fn total_steps(&self) -> usize {
        self.recording.spans.len()
    }

    /// Get current step index
    pub fn current_step(&self) -> usize {
        self.current_index
    }

    /// Check if at the beginning
    pub fn is_at_start(&self) -> bool {
        self.current_index == 0
    }

    /// Check if at the end
    pub fn is_at_end(&self) -> bool {
        self.current_index >= self.recording.spans.len()
    }

    /// Step forward by one
    pub fn step_forward(&mut self) -> Option<&AgentSpan> {
        if self.current_index < self.recording.spans.len() {
            let span = &self.recording.spans[self.current_index];
            self.current_index += 1;
            Some(span)
        } else {
            None
        }
    }

    /// Step backward by one
    pub fn step_backward(&mut self) -> Option<&AgentSpan> {
        if self.current_index > 0 {
            self.current_index -= 1;
            Some(&self.recording.spans[self.current_index])
        } else {
            None
        }
    }

    /// Jump to a specific step
    pub fn jump_to(&mut self, step: usize) -> Option<&AgentSpan> {
        if step < self.recording.spans.len() {
            self.current_index = step;
            Some(&self.recording.spans[self.current_index])
        } else {
            None
        }
    }

    /// Jump to next checkpoint
    pub fn next_checkpoint(&mut self) -> Option<&Checkpoint> {
        let current = self.current_index;
        
        for checkpoint in &self.recording.checkpoints {
            if checkpoint.step_index > current {
                self.current_index = checkpoint.step_index;
                return Some(checkpoint);
            }
        }
        
        None
    }

    /// Jump to previous checkpoint
    pub fn prev_checkpoint(&mut self) -> Option<&Checkpoint> {
        let current = self.current_index;
        let mut prev = None;
        
        for checkpoint in &self.recording.checkpoints {
            if checkpoint.step_index < current {
                prev = Some(checkpoint);
            } else {
                break;
            }
        }
        
        if let Some(checkpoint) = prev {
            self.current_index = checkpoint.step_index;
        }
        
        prev
    }

    /// Get current span
    pub fn current_span(&self) -> Option<&AgentSpan> {
        if self.current_index < self.recording.spans.len() {
            Some(&self.recording.spans[self.current_index])
        } else {
            None
        }
    }

    /// Get all spans up to current
    pub fn spans_up_to_current(&self) -> Vec<&AgentSpan> {
        self.recording.spans[..self.current_index].iter().collect()
    }

    /// Get all spans from current to end
    pub fn spans_from_current(&self) -> Vec<&AgentSpan> {
        self.recording.spans[self.current_index..].iter().collect()
    }

    /// Create a fork at current position with modified context
    pub fn fork(&self, modifications: ContextModifications) -> ForkedExecution {
        let fork_point = self.current_index;
        
        // Get context at fork point
        let base_context = self.get_context_at(fork_point);
        
        // Apply modifications
        let modified_context = modifications.apply(base_context);
        
        ForkedExecution {
            original_recording: self.recording.clone(),
            fork_point,
            modified_context,
            new_spans: Vec::new(),
        }
    }

    /// Get context at a specific step
    fn get_context_at(&self, step: usize) -> ExecutionContext {
        // Reconstruct context from spans up to that step
        let mut context = ExecutionContext {
            execution_id: self.recording.session.execution_id.clone(),
            step_number: step,
            current_thought: None,
            current_action: None,
            variables: HashMap::new(),
        };
        
        for span in &self.recording.spans[..step] {
            match span {
                AgentSpan::Thought { content, .. } => {
                    context.current_thought = Some(content.clone());
                }
                AgentSpan::Action { tool, .. } => {
                    context.current_action = Some(tool.clone());
                }
                // Could extract more context from other span types
                _ => {}
            }
        }
        
        context
    }

    /// Export recording to JSON
    pub fn export_json(&self) -> String {
        serde_json::to_string_pretty(&self.recording).unwrap_or_default()
    }

    /// Get execution statistics
    pub fn get_stats(&self) -> ExecutionStats {
        let spans = &self.recording.spans;
        
        ExecutionStats {
            total_steps: spans.len(),
            thought_steps: spans.iter().filter(|s| matches!(s, AgentSpan::Thought { .. })).count(),
            action_steps: spans.iter().filter(|s| matches!(s, AgentSpan::Action { .. })).count(),
            observation_steps: spans.iter().filter(|s| matches!(s, AgentSpan::Observation { .. })).count(),
            reflection_steps: spans.iter().filter(|s| matches!(s, AgentSpan::Reflection { .. })).count(),
            error_steps: spans.iter().filter(|s| matches!(s, AgentSpan::Error { .. })).count(),
            avg_step_duration_ms: self.calculate_avg_duration(),
        }
    }

    fn calculate_avg_duration(&self) -> u64 {
        let mut total_duration = 0u64;
        let mut count = 0usize;
        
        for window in self.recording.spans.windows(2) {
            if let (Some(t1), Some(t2)) = (get_timestamp(&window[0]), get_timestamp(&window[1])) {
                total_duration += t2 - t1;
                count += 1;
            }
        }
        
        if count > 0 {
            total_duration / count as u64
        } else {
            0
        }
    }
}

/// A forked execution for "what-if" scenarios
pub struct ForkedExecution {
    original_recording: ExecutionRecording,
    fork_point: usize,
    modified_context: ExecutionContext,
    new_spans: Vec<AgentSpan>,
}

impl ForkedExecution {
    /// Get the context at fork point
    pub fn get_fork_context(&self) -> &ExecutionContext {
        &self.modified_context
    }

    /// Add a new span to the fork
    pub fn add_span(&mut self, span: AgentSpan) {
        self.new_spans.push(span);
    }

    /// Compare with original execution
    pub fn compare_with_original(&self) -> ExecutionComparison {
        let original_spans = &self.original_recording.spans[self.fork_point..];
        
        ExecutionComparison {
            fork_point: self.fork_point,
            original_path: original_spans.to_vec(),
            forked_path: self.new_spans.clone(),
            divergence_step: self.find_divergence(original_spans),
        }
    }

    fn find_divergence(&self, original: &[AgentSpan]) -> Option<usize> {
        for (i, (orig, forked)) in original.iter().zip(self.new_spans.iter()).enumerate() {
            if !spans_similar(orig, forked) {
                return Some(i);
            }
        }
        
        if self.new_spans.len() != original.len() {
            Some(std::cmp::min(self.new_spans.len(), original.len()))
        } else {
            None
        }
    }

    /// Convert to a new recording
    pub fn to_recording(self) -> ExecutionRecording {
        let mut spans = self.original_recording.spans[..self.fork_point].to_vec();
        spans.extend(self.new_spans);
        
        ExecutionRecording {
            session: self.original_recording.session,
            spans,
            checkpoints: self.original_recording.checkpoints,
            metadata: self.original_recording.metadata,
        }
    }
}

/// Modifications to apply when forking
pub struct ContextModifications {
    pub variable_updates: HashMap<String, serde_json::Value>,
    pub inject_thought: Option<String>,
    pub skip_next_n_steps: usize,
}

impl ContextModifications {
    pub fn apply(&self, mut context: ExecutionContext) -> ExecutionContext {
        // Update variables
        for (key, value) in &self.variable_updates {
            context.variables.insert(key.clone(), value.clone());
        }
        
        // Inject thought if provided
        if let Some(ref thought) = self.inject_thought {
            context.current_thought = Some(thought.clone());
        }
        
        context
    }
}

/// Comparison between original and forked execution
pub struct ExecutionComparison {
    pub fork_point: usize,
    pub original_path: Vec<AgentSpan>,
    pub forked_path: Vec<AgentSpan>,
    pub divergence_step: Option<usize>,
}

/// Execution statistics
pub struct ExecutionStats {
    pub total_steps: usize,
    pub thought_steps: usize,
    pub action_steps: usize,
    pub observation_steps: usize,
    pub reflection_steps: usize,
    pub error_steps: usize,
    pub avg_step_duration_ms: u64,
}

/// A point in the replay timeline
pub struct ReplayPoint {
    pub step_index: usize,
    pub span: AgentSpan,
    pub context: ExecutionContext,
    pub is_checkpoint: bool,
}

/// Builder for creating recordings
pub struct RecordingBuilder {
    session: Option<TraceSession>,
    spans: Vec<AgentSpan>,
    checkpoints: Vec<Checkpoint>,
}

impl RecordingBuilder {
    pub fn new() -> Self {
        Self {
            session: None,
            spans: Vec::new(),
            checkpoints: Vec::new(),
        }
    }

    pub fn with_session(mut self, session: TraceSession) -> Self {
        self.session = Some(session);
        self
    }

    pub fn add_span(mut self, span: AgentSpan) -> Self {
        self.spans.push(span);
        self
    }

    pub fn add_checkpoint(mut self, description: impl Into<String>) -> Self {
        let checkpoint = Checkpoint {
            step_index: self.spans.len(),
            description: description.into(),
            context_snapshot: ExecutionContext {
                execution_id: self.session.as_ref().map(|s| s.execution_id.clone()).unwrap_or_default(),
                step_number: self.spans.len(),
                current_thought: None,
                current_action: None,
                variables: HashMap::new(),
            },
        };
        self.checkpoints.push(checkpoint);
        self
    }

    pub fn build(self) -> Option<ExecutionRecording> {
        self.session.map(|session| {
            let metadata = RecordingMetadata {
                total_steps: self.spans.len(),
                total_duration_ms: 0, // Calculate from spans
                tool_calls: self.spans.iter().filter(|s| matches!(s, AgentSpan::Action { .. })).count(),
                errors: self.spans.iter().filter(|s| matches!(s, AgentSpan::Error { .. })).count(),
                final_output: None,
            };

            ExecutionRecording {
                session,
                spans: self.spans,
                checkpoints: self.checkpoints,
                metadata,
            }
        })
    }
}

fn get_timestamp(span: &AgentSpan) -> Option<u64> {
    match span {
        AgentSpan::Thought { timestamp, .. } => Some(*timestamp),
        AgentSpan::Action { timestamp, .. } => Some(*timestamp),
        AgentSpan::Observation { timestamp, .. } => Some(*timestamp),
        AgentSpan::Reflection { timestamp, .. } => Some(*timestamp),
        AgentSpan::Decision { timestamp, .. } => Some(*timestamp),
        AgentSpan::LoopDetected { timestamp, .. } => Some(*timestamp),
        AgentSpan::Error { timestamp, .. } => Some(*timestamp),
    }
}

fn spans_similar(a: &AgentSpan, b: &AgentSpan) -> bool {
    // Simple comparison - could be more sophisticated
    std::mem::discriminant(a) == std::mem::discriminant(b)
}
