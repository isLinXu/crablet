//! Type definitions for the L2 USER Layer.
//!
//! Extracted from `layer_user.rs` to reduce file size and improve maintainability.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// User preference from config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preference {
    pub value: String,
    pub value_type: String,
}

/// User communication config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserCommunication {
    pub tone: String,
    pub detail_level: String,
    pub languages: Vec<String>,
    pub format_preference: String,
}

/// User configuration (local definition)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    pub user_id: String,
    pub name: String,
    pub storage_path: String,
    pub preferences: HashMap<String, Preference>,
    pub communication: UserCommunication,
}

/// User profile data (enhanced from config)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfileData {
    /// User ID
    pub user_id: String,

    /// Display name
    pub name: String,

    /// User preferences
    pub preferences: HashMap<String, PreferenceValue>,

    /// Important facts about the user
    pub facts: Vec<UserFact>,

    /// User goals
    pub goals: Vec<UserGoal>,

    /// Communication style
    pub communication_style: CommunicationStyle,

    /// Metadata
    pub metadata: ProfileMetadata,
}

/// Preference value
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum PreferenceValue {
    String(String),
    Number(f64),
    Boolean(bool),
    List(Vec<String>),
    Object(HashMap<String, String>),
}

/// User fact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserFact {
    /// Fact content
    pub content: String,

    /// Fact category
    pub category: String,

    /// Confidence level (0.0 - 1.0)
    pub confidence: f64,

    /// When the fact was learned
    pub learned_at: DateTime<Utc>,

    /// Source of the fact
    pub source: String,

    /// How many times this fact was reinforced
    pub reinforcement_count: u32,
}

/// User goal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserGoal {
    /// Goal description
    pub description: String,

    /// Goal status
    pub status: GoalStatus,

    /// Priority (1-10)
    pub priority: u8,

    /// Created at
    pub created_at: DateTime<Utc>,

    /// Target completion date (optional)
    pub target_date: Option<DateTime<Utc>>,

    /// Progress (0.0 - 1.0)
    pub progress: f64,
}

/// Goal status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum GoalStatus {
    Active,
    Completed,
    Paused,
    Cancelled,
}

/// Communication style
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationStyle {
    /// Preferred tone
    pub tone: String,

    /// Detail level preference
    pub detail_level: DetailLevel,

    /// Language preferences
    pub languages: Vec<String>,

    /// Response format preference
    pub format_preference: String,
}

/// Detail level
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DetailLevel {
    Brief,
    Moderate,
    Detailed,
    Comprehensive,
}

impl DetailLevel {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            DetailLevel::Brief => "brief",
            DetailLevel::Moderate => "moderate",
            DetailLevel::Detailed => "detailed",
            DetailLevel::Comprehensive => "comprehensive",
        }
    }
}

/// Profile metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMetadata {
    /// Created at
    pub created_at: DateTime<Utc>,

    /// Last updated
    pub updated_at: DateTime<Utc>,

    /// Total interactions
    pub total_interactions: u64,

    /// Total memories stored
    pub total_memories: u64,
}

/// Memory entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Memory ID
    pub id: String,

    /// Memory content
    pub content: String,

    /// Memory type
    pub memory_type: MemoryType,

    /// Category
    pub category: String,

    /// Importance score (0.0 - 1.0)
    pub importance: f64,

    /// Created at
    pub created_at: DateTime<Utc>,

    /// Last accessed
    pub last_accessed: DateTime<Utc>,

    /// Access count
    pub access_count: u32,

    /// Related memory IDs
    pub related_memories: Vec<String>,

    /// Source (session ID, etc.)
    pub source: String,
}

/// Memory type
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryType {
    /// Explicitly stated fact
    ExplicitFact,
    /// Inferred from conversation
    Inferred,
    /// User preference
    Preference,
    /// Decision or choice
    Decision,
    /// Emotional state
    Emotional,
    /// Goal or intention
    Goal,
    /// Contextual information
    Contextual,
}
