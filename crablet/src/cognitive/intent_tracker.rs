//! Cross-Session Intent Tracking System
//!
//! Tracks user intentions and goals across multiple sessions to enable:
//! - Long-term goal inference and tracking
//! - Intent graph construction and analysis
//! - Goal completion prediction
//! - Proactive assistance

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Intent type classification
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum IntentType {
    TaskCompletion,
    Learning,
    InformationRetrieval,
    ProblemSolving,
    DecisionMaking,
    Communication,
    Exploratory,
    Routine,
    Unknown,
}

impl IntentType {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "task" | "task_completion" | "完成" => IntentType::TaskCompletion,
            "learning" | "learn" | "学习" => IntentType::Learning,
            "info" | "information" | "查询" => IntentType::InformationRetrieval,
            "problem" | "problem_solving" | "解决问题" => IntentType::ProblemSolving,
            "decision" | "decision_making" | "决策" => IntentType::DecisionMaking,
            "chat" | "communication" | "交流" => IntentType::Communication,
            "explore" | "exploratory" | "探索" => IntentType::Exploratory,
            "routine" | "日常" => IntentType::Routine,
            _ => IntentType::Unknown,
        }
    }
    
    pub fn priority(&self) -> u8 {
        match self {
            IntentType::TaskCompletion => 10,
            IntentType::ProblemSolving => 9,
            IntentType::DecisionMaking => 8,
            IntentType::Learning => 7,
            IntentType::InformationRetrieval => 6,
            IntentType::Communication => 5,
            IntentType::Exploratory => 4,
            IntentType::Routine => 3,
            IntentType::Unknown => 1,
        }
    }
}

/// Extracted intent from user message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub id: String,
    pub intent_type: IntentType,
    pub description: String,
    pub keywords: Vec<String>,
    pub confidence: f32,
    pub context_hints: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub source_session: String,
}

impl Intent {
    pub fn new(
        intent_type: IntentType,
        description: String,
        confidence: f32,
        source_session: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            intent_type,
            description,
            keywords: Vec::new(),
            confidence,
            context_hints: Vec::new(),
            created_at: Utc::now(),
            source_session,
        }
    }
    
    pub fn with_keywords(mut self, keywords: Vec<String>) -> Self {
        self.keywords = keywords;
        self
    }
}

/// User goal that spans multiple sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub id: String,
    pub title: String,
    pub description: String,
    pub goal_type: IntentType,
    pub status: GoalStatus,
    pub priority: u8,
    pub progress: f32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub target_date: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub related_intents: Vec<String>,
    pub milestones: Vec<Milestone>,
    pub session_count: u32,
    pub completion_probability: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GoalStatus {
    Active,
    Paused,
    Completed,
    Cancelled,
    Blocked,
}

impl Goal {
    pub fn new(title: String, goal_type: IntentType, priority: u8) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            title,
            description: String::new(),
            goal_type,
            status: GoalStatus::Active,
            priority: priority.min(10),
            progress: 0.0,
            created_at: now,
            updated_at: now,
            target_date: None,
            completed_at: None,
            related_intents: Vec::new(),
            milestones: Vec::new(),
            session_count: 1,
            completion_probability: 0.5,
        }
    }
    
    pub fn update_progress(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 1.0);
        self.updated_at = Utc::now();
        
        if self.progress >= 1.0 {
            self.status = GoalStatus::Completed;
            self.completed_at = Some(Utc::now());
            self.completion_probability = 1.0;
        }
    }
    
    pub fn add_milestone(&mut self, title: String, target_progress: f32) {
        self.milestones.push(Milestone {
            id: Uuid::new_v4().to_string(),
            title,
            target_progress,
            completed: false,
            completed_at: None,
        });
    }
}

/// Milestone in a goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub id: String,
    pub title: String,
    pub target_progress: f32,
    pub completed: bool,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Intent node in the graph
#[derive(Debug, Clone)]
pub struct IntentNode {
    pub id: String,
    pub intent_type: IntentType,
    pub keywords: Vec<String>,
    pub frequency: u32,
    pub avg_session_span: f32,
    pub related_goal_ids: Vec<String>,
    pub last_observed: DateTime<Utc>,
}

/// Edge in intent graph
#[derive(Debug, Clone)]
pub struct IntentEdge {
    pub source_id: String,
    pub target_id: String,
    pub weight: f32,
    pub co_occurrence_count: u32,
    pub avg_transition_time_hours: f32,
}

/// Intent graph for tracking relationships
pub struct IntentGraph {
    nodes: HashMap<String, IntentNode>,
    edges: Vec<IntentEdge>,
    adjacency: HashMap<String, Vec<String>>,
}

impl IntentGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            adjacency: HashMap::new(),
        }
    }
    
    pub fn add_intent(&mut self, intent: &Intent) {
        let node_id = intent.id.clone();
        
        let node = self.nodes.entry(node_id.clone()).or_insert_with(|| IntentNode {
            id: node_id.clone(),
            intent_type: intent.intent_type,
            keywords: intent.keywords.clone(),
            frequency: 1,
            avg_session_span: 1.0,
            related_goal_ids: Vec::new(),
            last_observed: Utc::now(),
        });
        
        node.frequency += 1;
        
        self.adjacency.entry(node_id).or_insert_with(Vec::new);
    }
    
    pub fn add_transition(&mut self, from_id: &str, to_id: &str, hours_elapsed: f32) {
        if !self.nodes.contains_key(from_id) {
            self.nodes.insert(from_id.to_string(), IntentNode {
                id: from_id.to_string(),
                intent_type: IntentType::Unknown,
                keywords: Vec::new(),
                frequency: 0,
                avg_session_span: 0.0,
                related_goal_ids: Vec::new(),
                last_observed: Utc::now(),
            });
        }
        
        if !self.nodes.contains_key(to_id) {
            self.nodes.insert(to_id.to_string(), IntentNode {
                id: to_id.to_string(),
                intent_type: IntentType::Unknown,
                keywords: Vec::new(),
                frequency: 0,
                avg_session_span: 0.0,
                related_goal_ids: Vec::new(),
                last_observed: Utc::now(),
            });
        }
        
        if let Some(edge) = self.edges.iter_mut().find(|e| e.source_id == from_id && e.target_id == to_id) {
            edge.co_occurrence_count += 1;
            let n = edge.co_occurrence_count as f32;
            edge.avg_transition_time_hours = (edge.avg_transition_time_hours * (n - 1.0) + hours_elapsed) / n;
        } else {
            self.edges.push(IntentEdge {
                source_id: from_id.to_string(),
                target_id: to_id.to_string(),
                weight: 1.0,
                co_occurrence_count: 1,
                avg_transition_time_hours: hours_elapsed,
            });
        }
        
        self.adjacency
            .entry(from_id.to_string())
            .or_insert_with(Vec::new)
            .push(to_id.to_string());
    }
    
    pub fn detect_goal_pattern(&self, intent_sequence: &[String]) -> Option<String> {
        if intent_sequence.len() < 2 {
            return None;
        }
        
        for edge in &self.edges {
            if edge.source_id == intent_sequence[0] && edge.target_id == intent_sequence[1] {
                if edge.co_occurrence_count >= 3 {
                    return Some(format!("Goal pattern: {} -> {} (count: {})", 
                        edge.source_id, edge.target_id, edge.co_occurrence_count));
                }
            }
        }
        
        None
    }
}

/// Proactive suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProactiveSuggestion {
    pub id: String,
    pub suggestion_type: SuggestionType,
    pub title: String,
    pub description: String,
    pub action: String,
    pub confidence: f32,
    pub related_goal_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionType {
    GoalReminder,
    IntentFollowUp,
    ContextResumption,
    RelatedTask,
    CompletionCheck,
}

/// Intent tracking statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentStats {
    pub total_sessions: u32,
    pub total_intents: u32,
    pub total_goals: u32,
    pub active_goals: u32,
    pub completed_goals: u32,
    pub avg_intents_per_session: f32,
    pub most_common_intent: Option<IntentType>,
    pub avg_goal_duration_hours: f32,
}

/// Cross-Session Intent Tracker
pub struct IntentTracker {
    intents: Arc<RwLock<HashMap<String, Intent>>>,
    goals: Arc<RwLock<HashMap<String, Goal>>>,
    intent_graph: Arc<RwLock<IntentGraph>>,
    session_intents: Arc<RwLock<HashMap<String, Vec<String>>>>,
    user_intent_history: Arc<RwLock<VecDeque<Intent>>>,
    stats: Arc<RwLock<IntentStats>>,
}

impl IntentTracker {
    pub fn new() -> Self {
        Self {
            intents: Arc::new(RwLock::new(HashMap::new())),
            goals: Arc::new(RwLock::new(HashMap::new())),
            intent_graph: Arc::new(RwLock::new(IntentGraph::new())),
            session_intents: Arc::new(RwLock::new(HashMap::new())),
            user_intent_history: Arc::new(RwLock::new(VecDeque::new())),
            stats: Arc::new(RwLock::new(IntentStats {
                total_sessions: 0,
                total_intents: 0,
                total_goals: 0,
                active_goals: 0,
                completed_goals: 0,
                avg_intents_per_session: 0.0,
                most_common_intent: None,
                avg_goal_duration_hours: 0.0,
            })),
        }
    }
    
    /// Extract intent from user message
    pub async fn extract_intent(&self, message: &str, session_id: &str) -> Intent {
        let intent_type = self.classify_intent(message);
        let keywords = self.extract_keywords(message);
        let confidence = self.calculate_confidence(&intent_type, &keywords);
        
        let intent = Intent::new(
            intent_type,
            message.chars().take(100).collect(),
            confidence,
            session_id.to_string(),
        )
        .with_keywords(keywords.clone());
        
        // Store intent
        let mut intents = self.intents.write().await;
        intents.insert(intent.id.clone(), intent.clone());
        
        let mut session_intents = self.session_intents.write().await;
        session_intents
            .entry(session_id.to_string())
            .or_insert_with(Vec::new)
            .push(intent.id.clone());
        
        drop(intents);
        let mut graph = self.intent_graph.write().await;
        graph.add_intent(&intent);
        
        let mut history = self.user_intent_history.write().await;
        history.push_back(intent.clone());
        if history.len() > 100 {
            history.pop_front();
        }
        
        intent
    }
    
    fn classify_intent(&self, message: &str) -> IntentType {
        let msg_lower = message.to_lowercase();
        
        if msg_lower.contains("完成") || msg_lower.contains("做好") || 
           msg_lower.contains("帮我") || msg_lower.contains("帮我做") {
            return IntentType::TaskCompletion;
        }
        
        if msg_lower.contains("解决") || msg_lower.contains("修复") || 
           msg_lower.contains("错误") || msg_lower.contains("bug") {
            return IntentType::ProblemSolving;
        }
        
        if msg_lower.contains("学习") || msg_lower.contains("教我") || 
           msg_lower.contains("怎么") || msg_lower.contains("是什么") {
            return IntentType::Learning;
        }
        
        if msg_lower.contains("选择") || msg_lower.contains("哪个好") || 
           msg_lower.contains("建议") || msg_lower.contains("推荐") {
            return IntentType::DecisionMaking;
        }
        
        if msg_lower.contains("查询") || msg_lower.contains("搜索") || 
           msg_lower.contains("找") || msg_lower.contains("看看") {
            return IntentType::InformationRetrieval;
        }
        
        if msg_lower.contains("你好") || msg_lower.contains("聊聊") || 
           msg_lower.contains("没事") || msg_lower.contains("随便") {
            return IntentType::Communication;
        }
        
        if msg_lower.contains("试试") || msg_lower.contains("探索") || 
           msg_lower.contains("玩") || msg_lower.contains("好奇") {
            return IntentType::Exploratory;
        }
        
        IntentType::Unknown
    }
    
    fn extract_keywords(&self, message: &str) -> Vec<String> {
        let stop_words = ["的", "了", "在", "是", "我", "你", "他", "她", "它", "这", "那", "一个", "什么", "怎么", "如何", "the", "a", "an", "is", "are", "was", "were", "do", "does", "did"];
        
        message
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '#' && !c.is_whitespace())
            .filter(|w| w.len() > 1)
            .filter(|w| !stop_words.contains(&w.to_lowercase().as_str()))
            .take(10)
            .map(|w| w.to_string())
            .collect()
    }
    
    fn calculate_confidence(&self, intent_type: &IntentType, keywords: &[String]) -> f32 {
        let base = 0.5;
        let keyword_bonus = (keywords.len() as f32 * 0.05).min(0.3);
        let type_bonus = if *intent_type == IntentType::Unknown { 0.0 } else { 0.2 };
        
        (base + keyword_bonus + type_bonus).min(1.0)
    }
    
    /// Create a new goal from intent
    pub async fn create_goal_from_intent(&self, intent_id: &str, title: String, priority: u8) -> Option<Goal> {
        let intents = self.intents.read().await;
        let intent = intents.get(intent_id)?;
        
        let mut goal = Goal::new(title, intent.intent_type, priority);
        goal.related_intents.push(intent_id.to_string());
        
        let mut goals = self.goals.write().await;
        goals.insert(goal.id.clone(), goal.clone());
        
        drop(goals);
        self.update_stats().await;
        
        Some(goal)
    }
    
    /// Update goal progress
    pub async fn update_goal_progress(&self, goal_id: &str, progress: f32) -> Result<(), String> {
        let mut goals = self.goals.write().await;
        
        if let Some(goal) = goals.get_mut(goal_id) {
            goal.update_progress(progress);
            Ok(())
        } else {
            Err(format!("Goal not found: {}", goal_id))
        }
    }
    
    /// Get active goals
    pub async fn get_active_goals(&self) -> Vec<Goal> {
        let goals = self.goals.read().await;
        goals
            .values()
            .filter(|g| g.status == GoalStatus::Active)
            .cloned()
            .collect()
    }
    
    /// Get goal by ID
    pub async fn get_goal(&self, goal_id: &str) -> Option<Goal> {
        let goals = self.goals.read().await;
        goals.get(goal_id).cloned()
    }
    
    /// Get session intents
    pub async fn get_session_intents(&self, session_id: &str) -> Vec<Intent> {
        let session_intents = self.session_intents.read().await;
        let intents = self.intents.read().await;
        
        if let Some(intent_ids) = session_intents.get(session_id) {
            intent_ids
                .iter()
                .filter_map(|id| intents.get(id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }
    
    /// Generate proactive suggestions
    pub async fn generate_suggestions(&self, session_id: &str) -> Vec<ProactiveSuggestion> {
        let mut suggestions = Vec::new();
        
        let active_goals = self.get_active_goals().await;
        
        for goal in active_goals {
            let intents = self.get_session_intents(session_id).await;
            let mentioned = intents.iter().any(|i| {
                goal.title.to_lowercase().contains(&i.description.to_lowercase()) ||
                i.description.to_lowercase().contains(&goal.title.to_lowercase())
            });
            
            if !mentioned && goal.progress < 1.0 {
                let confidence = goal.completion_probability * goal.priority as f32 / 10.0;
                
                suggestions.push(ProactiveSuggestion {
                    id: Uuid::new_v4().to_string(),
                    suggestion_type: SuggestionType::GoalReminder,
                    title: format!("继续: {}", goal.title),
                    description: format!("你的目标「{}」完成了 {:.0}%", goal.title, goal.progress * 100.0),
                    action: format!("/resume goal {}", goal.id),
                    confidence,
                    related_goal_id: Some(goal.id.clone()),
                    created_at: Utc::now(),
                });
            }
        }
        
        suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        suggestions.truncate(5);
        
        suggestions
    }
    
    /// Predict goal completion time
    pub async fn predict_completion(&self, goal_id: &str) -> Option<f32> {
        let goals = self.goals.read().await;
        
        if let Some(goal) = goals.get(goal_id) {
            if goal.progress <= 0.0 {
                return None;
            }
            
            let hours_elapsed = (Utc::now() - goal.created_at).num_minutes() as f32 / 60.0;
            let remaining_progress = 1.0 - goal.progress;
            
            if remaining_progress <= 0.0 {
                return Some(0.0);
            }
            
            let hours_per_progress = hours_elapsed / goal.progress;
            let predicted_hours = hours_per_progress * remaining_progress;
            
            Some(predicted_hours)
        } else {
            None
        }
    }
    
    /// Get intent statistics
    pub async fn get_stats(&self) -> IntentStats {
        self.stats.read().await.clone()
    }
    
    async fn update_stats(&self) {
        let intents = self.intents.read().await;
        let goals = self.goals.read().await;
        let sessions = self.session_intents.read().await;
        
        let mut stats = self.stats.write().await;
        
        stats.total_intents = intents.len() as u32;
        stats.total_goals = goals.len() as u32;
        stats.active_goals = goals.values().filter(|g| g.status == GoalStatus::Active).count() as u32;
        stats.completed_goals = goals.values().filter(|g| g.status == GoalStatus::Completed).count() as u32;
        stats.total_sessions = sessions.len() as u32;
        
        if stats.total_sessions > 0 {
            stats.avg_intents_per_session = stats.total_intents as f32 / stats.total_sessions as f32;
        }
        
        let mut intent_counts: HashMap<IntentType, u32> = HashMap::new();
        for intent in intents.values() {
            *intent_counts.entry(intent.intent_type).or_insert(0) += 1;
        }
        
        stats.most_common_intent = intent_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(intent_type, _)| intent_type);
    }
    
    /// End session and analyze patterns
    pub async fn end_session(&self, session_id: &str) {
        let session_intents = self.session_intents.read().await;
        
        if let Some(intent_ids) = session_intents.get(session_id) {
            if intent_ids.len() >= 2 {
                let intents = self.intents.read().await;
                
                let mut graph = self.intent_graph.write().await;
                
                for i in 0..intent_ids.len() - 1 {
                    let from = &intent_ids[i];
                    let to = &intent_ids[i + 1];
                    
                    let time_hours = if let (Some(from_intent), Some(to_intent)) = 
                        (intents.get(from), intents.get(to)) {
                        (to_intent.created_at - from_intent.created_at).num_minutes() as f32 / 60.0
                    } else {
                        1.0
                    };
                    
                    graph.add_transition(from, to, time_hours);
                }
            }
        }
        
        self.update_stats().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_intent_extraction() {
        let tracker = IntentTracker::new();
        
        let intent = tracker.extract_intent("帮我完成这个任务", "session_1").await;
        assert_eq!(intent.intent_type, IntentType::TaskCompletion);
        assert!(intent.confidence > 0.5);
    }

    #[tokio::test]
    async fn test_goal_creation() {
        let tracker = IntentTracker::new();
        
        let intent = tracker.extract_intent("我想学习Rust编程", "session_1").await;
        let goal = tracker.create_goal_from_intent(&intent.id, "学习Rust".to_string(), 8).await;
        
        assert!(goal.is_some());
        let goal = goal.unwrap();
        assert_eq!(goal.goal_type, IntentType::Learning);
    }

    #[tokio::test]
    async fn test_goal_progress_update() {
        let tracker = IntentTracker::new();
        
        let intent = tracker.extract_intent("完成项目", "session_1").await;
        let goal = tracker.create_goal_from_intent(&intent.id, "完成项目".to_string(), 7).await.unwrap();
        
        tracker.update_goal_progress(&goal.id, 0.5).await.unwrap();
        
        let updated = tracker.get_goal(&goal.id).await.unwrap();
        assert_eq!(updated.progress, 0.5);
    }

    #[tokio::test]
    async fn test_proactive_suggestions() {
        let tracker = IntentTracker::new();
        
        let intent = tracker.extract_intent("完成报告", "session_1").await;
        tracker.create_goal_from_intent(&intent.id, "完成季度报告".to_string(), 8).await;
        
        let suggestions = tracker.generate_suggestions("session_2").await;
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn test_intent_classification() {
        let tracker = IntentTracker::new();
        
        assert_eq!(tracker.classify_intent("帮我完成这个任务"), IntentType::TaskCompletion);
        assert_eq!(tracker.classify_intent("怎么学习编程"), IntentType::Learning);
    }

    #[test]
    fn test_keyword_extraction() {
        let tracker = IntentTracker::new();
        
        let keywords = tracker.extract_keywords("帮我完成Python项目的开发工作");
        
        assert!(keywords.contains(&"Python".to_string()));
        assert!(!keywords.contains(&"的".to_string()));
    }
}