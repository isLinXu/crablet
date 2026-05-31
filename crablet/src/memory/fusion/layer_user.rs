//! L2: USER Layer - Semantic Long-term Memory
//!
//! The USER layer stores semantic long-term memory about the user, including:
//! - User profile and preferences
//! - Important facts and decisions
//! - Conversation history summaries
//! - Learned patterns and behaviors

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::memory::fusion::layer_session::SessionLayer;
use crate::memory::fusion::{MemoryError, MemoryStats};

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

/// L2 USER Layer - Semantic long-term memory
pub struct UserLayer {
    /// User profile
    profile: RwLock<UserProfileData>,

    /// Semantic memories
    memories: RwLock<Vec<Memory>>,

    /// Memory index by category
    memory_index: RwLock<HashMap<String, Vec<usize>>>,

    /// Storage path
    storage_path: PathBuf,

    /// Configuration
    config: UserConfig,
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
    fn as_str(&self) -> &'static str {
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

#[derive(Debug, Clone)]
struct PreferenceCandidate {
    key: String,
    value: PreferenceValue,
}

#[derive(Debug, Clone, Default)]
struct SessionInsights {
    preferences: Vec<PreferenceCandidate>,
    facts: Vec<(String, String, f64)>,
    goals: Vec<String>,
}

impl UserLayer {
    /// Initialize USER layer from configuration
    pub async fn from_config(config: &UserConfig) -> Result<Self, MemoryError> {
        info!("Initializing USER layer...");

        let storage_path = PathBuf::from(&config.storage_path);

        // Create storage directory if needed
        if !storage_path.exists() {
            tokio::fs::create_dir_all(&storage_path).await?;
        }

        // Load or create profile
        let profile = Self::load_or_create_profile(config).await?;

        // Load memories
        let memories = Self::load_memories(&storage_path).await?;

        // Build index
        let memory_index = Self::build_index(&memories);

        let layer = Self {
            profile: RwLock::new(profile),
            memories: RwLock::new(memories),
            memory_index: RwLock::new(memory_index),
            storage_path,
            config: config.clone(),
        };

        info!("USER layer initialized");
        Ok(layer)
    }

    /// Load or create user profile
    async fn load_or_create_profile(config: &UserConfig) -> Result<UserProfileData, MemoryError> {
        let profile_path = PathBuf::from(&config.storage_path).join("profile.json");

        if profile_path.exists() {
            let content = tokio::fs::read_to_string(&profile_path).await?;
            let profile: UserProfileData = serde_json::from_str(&content)
                .map_err(|e| MemoryError::PersistenceError(e.to_string()))?;
            debug!("Loaded existing user profile");
            Ok(profile)
        } else {
            // Create new profile from config
            let now = Utc::now();
            let profile = UserProfileData {
                user_id: config.user_id.clone(),
                name: config.name.clone(),
                preferences: Self::convert_preferences(&config.preferences),
                facts: Vec::new(),
                goals: Vec::new(),
                communication_style: CommunicationStyle {
                    tone: config.communication.tone.clone(),
                    detail_level: Self::parse_detail_level(&config.communication.detail_level),
                    languages: config.communication.languages.clone(),
                    format_preference: config.communication.format_preference.clone(),
                },
                metadata: ProfileMetadata {
                    created_at: now,
                    updated_at: now,
                    total_interactions: 0,
                    total_memories: 0,
                },
            };
            debug!("Created new user profile");
            Ok(profile)
        }
    }

    /// Convert config preferences to profile format
    fn convert_preferences(
        prefs: &HashMap<String, Preference>,
    ) -> HashMap<String, PreferenceValue> {
        prefs
            .iter()
            .map(|(k, v): (&String, &Preference)| {
                let value = match v.value_type.as_str() {
                    "string" => PreferenceValue::String(v.value.clone()),
                    "number" => PreferenceValue::Number(v.value.parse().unwrap_or(0.0)),
                    "boolean" => PreferenceValue::Boolean(v.value.parse().unwrap_or(false)),
                    "list" => PreferenceValue::List(
                        v.value
                            .split(',')
                            .map(|s: &str| s.trim().to_string())
                            .collect(),
                    ),
                    _ => PreferenceValue::String(v.value.clone()),
                };
                (k.clone(), value)
            })
            .collect()
    }

    /// Parse detail level
    fn parse_detail_level(level: &str) -> DetailLevel {
        match level.to_lowercase().as_str() {
            "brief" => DetailLevel::Brief,
            "detailed" => DetailLevel::Detailed,
            "comprehensive" => DetailLevel::Comprehensive,
            _ => DetailLevel::Moderate,
        }
    }

    /// Load memories from storage
    async fn load_memories(storage_path: &PathBuf) -> Result<Vec<Memory>, MemoryError> {
        let memories_path = storage_path.join("memories.json");

        if memories_path.exists() {
            let content = tokio::fs::read_to_string(&memories_path).await?;
            let memories: Vec<Memory> = serde_json::from_str(&content)
                .map_err(|e| MemoryError::PersistenceError(e.to_string()))?;
            debug!("Loaded {} memories", memories.len());
            Ok(memories)
        } else {
            Ok(Vec::new())
        }
    }

    /// Build memory index
    fn build_index(memories: &[Memory]) -> HashMap<String, Vec<usize>> {
        let mut index: HashMap<String, Vec<usize>> = HashMap::new();

        for (idx, memory) in memories.iter().enumerate() {
            index.entry(memory.category.clone()).or_default().push(idx);
        }

        index
    }

    /// Record a new memory
    pub async fn record_memory(&self, memory: Memory) -> Result<(), MemoryError> {
        let mut memories = self.memories.write().await;
        let mut index = self.memory_index.write().await;
        let mut profile = self.profile.write().await;

        let idx = memories.len();
        memories.push(memory.clone());

        // Update index
        index.entry(memory.category.clone()).or_default().push(idx);

        // Update profile metadata
        profile.metadata.total_memories += 1;
        profile.metadata.updated_at = Utc::now();

        // Persist
        drop(memories);
        drop(index);
        drop(profile);
        self.persist_memories().await?;

        debug!(
            "Recorded memory: {} (category: {})",
            memory.id, memory.category
        );
        Ok(())
    }

    /// Search for relevant memories
    pub async fn search_relevant_context(&self, limit: usize) -> Result<Vec<Memory>, MemoryError> {
        let memories = self.memories.read().await;

        // Simple relevance scoring based on importance and recency
        let mut scored: Vec<(f64, usize)> = memories
            .iter()
            .enumerate()
            .map(|m| {
                let (idx, memory) = m;
                let recency_score = Self::calculate_recency_score(&memory.last_accessed);
                let importance_score = memory.importance;
                let access_score = (memory.access_count as f64 / 100.0).min(1.0);

                let total_score = importance_score * 0.4 + recency_score * 0.4 + access_score * 0.2;
                (total_score, idx)
            })
            .collect();

        // Sort by score (descending)
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));

        // Take top results
        let selected_indices: Vec<usize> =
            scored.into_iter().take(limit).map(|(_, idx)| idx).collect();
        let results: Vec<Memory> = selected_indices
            .iter()
            .filter_map(|&idx| memories.get(idx).cloned())
            .collect();
        drop(memories);

        if !selected_indices.is_empty() {
            let now = Utc::now();
            let mut memories = self.memories.write().await;
            for idx in selected_indices {
                if let Some(memory) = memories.get_mut(idx) {
                    memory.access_count = memory.access_count.saturating_add(1);
                    memory.last_accessed = now;
                }
            }
            drop(memories);
            self.persist_memories().await?;
        }

        Ok(results)
    }

    /// Calculate recency score
    fn calculate_recency_score(last_accessed: &DateTime<Utc>) -> f64 {
        let now = Utc::now();
        let duration = now.signed_duration_since(*last_accessed);
        let days = duration.num_days() as f64;

        // Exponential decay over 30 days
        (-days / 30.0).exp()
    }

    /// Search memories by category
    pub async fn search_by_category(&self, category: &str, limit: usize) -> Vec<Memory> {
        let memories = self.memories.read().await;
        let index = self.memory_index.read().await;

        if let Some(indices) = index.get(category) {
            indices
                .iter()
                .take(limit)
                .filter_map(|&idx| memories.get(idx).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Update user profile from session
    pub async fn update_from_session(&self, session: &SessionLayer) -> Result<(), MemoryError> {
        let messages = session.get_messages().await;
        let summary = session.generate_summary().await;
        let insights = Self::extract_session_insights(&messages);

        {
            let mut profile = self.profile.write().await;
            profile.metadata.total_interactions += 1;
            profile.metadata.updated_at = Utc::now();

            for candidate in insights.preferences {
                Self::apply_preference(&mut profile, candidate);
            }

            for (content, category, confidence) in insights.facts {
                Self::merge_fact(
                    &mut profile,
                    content,
                    category,
                    confidence,
                    session.session_id(),
                );
            }

            for goal in insights.goals {
                Self::merge_goal(&mut profile, goal);
            }
        }

        let inserted_memory = {
            let mut memories = self.memories.write().await;
            let mut index = self.memory_index.write().await;
            Self::upsert_session_summary_memory(
                &mut memories,
                &mut index,
                session.session_id(),
                &summary,
            )
        };

        let memory_count = if inserted_memory {
            None
        } else {
            Some(self.memories.read().await.len() as u64)
        };

        {
            let mut profile = self.profile.write().await;
            if inserted_memory {
                profile.metadata.total_memories += 1;
            } else if let Some(memory_count) = memory_count {
                profile.metadata.total_memories = memory_count;
            }
            profile.metadata.updated_at = Utc::now();
        }

        self.persist_profile().await?;
        self.persist_memories().await?;

        Ok(())
    }

    /// Add a user fact
    pub async fn add_fact(
        &self,
        content: String,
        category: String,
        confidence: f64,
    ) -> Result<(), MemoryError> {
        let mut profile = self.profile.write().await;

        // Check if fact already exists
        if let Some(existing) = profile.facts.iter_mut().find(|f| f.content == content) {
            // Reinforce existing fact
            existing.confidence = (existing.confidence + confidence) / 2.0;
            existing.reinforcement_count += 1;
        } else {
            // Add new fact
            profile.facts.push(UserFact {
                content,
                category,
                confidence,
                learned_at: Utc::now(),
                source: "conversation".to_string(),
                reinforcement_count: 1,
            });
        }

        drop(profile);
        self.persist_profile().await?;

        Ok(())
    }

    /// Add a user goal
    pub async fn add_goal(&self, description: String, priority: u8) -> Result<(), MemoryError> {
        let mut profile = self.profile.write().await;

        profile.goals.push(UserGoal {
            description,
            status: GoalStatus::Active,
            priority: priority.min(10),
            created_at: Utc::now(),
            target_date: None,
            progress: 0.0,
        });

        drop(profile);
        self.persist_profile().await?;

        Ok(())
    }

    /// Update preference
    pub async fn update_preference(
        &self,
        key: String,
        value: PreferenceValue,
    ) -> Result<(), MemoryError> {
        let mut profile = self.profile.write().await;

        profile.preferences.insert(key, value);
        profile.metadata.updated_at = Utc::now();

        drop(profile);
        self.persist_profile().await?;

        Ok(())
    }

    /// Get user profile
    pub async fn get_profile(&self) -> UserProfileData {
        self.profile.read().await.clone()
    }

    /// Get all memories
    pub async fn get_all_memories(&self) -> Vec<Memory> {
        self.memories.read().await.clone()
    }

    /// Consolidate memories (merge similar, archive old)
    pub async fn consolidate(&self) -> Result<usize, MemoryError> {
        let mut memories = self.memories.write().await;
        let original_count = memories.len();

        // Simple consolidation: remove very old, low-importance memories
        let cutoff = Utc::now() - chrono::Duration::days(90);

        memories.retain(|m| m.importance > 0.3 || m.last_accessed > cutoff);

        let removed = original_count - memories.len();
        let rebuilt_index = Self::build_index(&memories);
        let memory_count = memories.len() as u64;

        drop(memories);
        {
            let mut index = self.memory_index.write().await;
            *index = rebuilt_index;
        }
        {
            let mut profile = self.profile.write().await;
            profile.metadata.total_memories = memory_count;
            profile.metadata.updated_at = Utc::now();
        }
        self.persist_profile().await?;
        self.persist_memories().await?;

        info!("Consolidated memories: removed {}", removed);
        Ok(removed)
    }

    /// Persist memories to storage
    async fn persist_memories(&self) -> Result<(), MemoryError> {
        let memories = self.memories.read().await;
        let path = self.storage_path.join("memories.json");

        let content = serde_json::to_string_pretty(&*memories)
            .map_err(|e| MemoryError::PersistenceError(e.to_string()))?;

        tokio::fs::write(&path, content).await?;

        Ok(())
    }

    /// Persist profile to storage
    async fn persist_profile(&self) -> Result<(), MemoryError> {
        let profile = self.profile.read().await;
        let path = self.storage_path.join("profile.json");

        let content = serde_json::to_string_pretty(&*profile)
            .map_err(|e| MemoryError::PersistenceError(e.to_string()))?;

        tokio::fs::write(&path, content).await?;

        Ok(())
    }

    /// Export profile to Markdown
    pub async fn export_to_markdown(&self, path: &PathBuf) -> Result<(), MemoryError> {
        let profile = self.profile.read().await;

        let mut content = String::new();
        content.push_str("---\n");
        content.push_str(&format!("user_id: {}\n", profile.user_id));
        content.push_str(&format!("name: {}\n", profile.name));
        content.push_str(&format!(
            "created_at: {}\n",
            profile.metadata.created_at.to_rfc3339()
        ));
        content.push_str(&format!(
            "updated_at: {}\n",
            profile.metadata.updated_at.to_rfc3339()
        ));
        content.push_str(&format!(
            "total_interactions: {}\n",
            profile.metadata.total_interactions
        ));
        content.push_str(&format!(
            "total_memories: {}\n",
            profile.metadata.total_memories
        ));
        content.push_str("---\n\n");

        content.push_str("# User Profile\n\n");

        // Preferences
        content.push_str("## Preferences\n\n");
        for (key, value) in &profile.preferences {
            let value_str = match value {
                PreferenceValue::String(s) => s.clone(),
                PreferenceValue::Number(n) => n.to_string(),
                PreferenceValue::Boolean(b) => b.to_string(),
                PreferenceValue::List(l) => l.join(", "),
                PreferenceValue::Object(o) => format!("{:?}", o),
            };
            content.push_str(&format!("- **{}**: {}\n", key, value_str));
        }
        content.push('\n');

        // Communication style
        content.push_str("## Communication Style\n\n");
        content.push_str(&format!(
            "- **Tone**: {}\n",
            profile.communication_style.tone
        ));
        content.push_str(&format!(
            "- **Detail Level**: {:?}\n",
            profile.communication_style.detail_level
        ));
        content.push_str(&format!(
            "- **Languages**: {}\n",
            profile.communication_style.languages.join(", ")
        ));
        content.push_str(&format!(
            "- **Format**: {}\n",
            profile.communication_style.format_preference
        ));
        content.push('\n');

        // Facts
        content.push_str("## Facts\n\n");
        for fact in &profile.facts {
            content.push_str(&format!(
                "- **{}** (confidence: {:.0}%): {}\n",
                fact.category,
                fact.confidence * 100.0,
                fact.content
            ));
        }
        content.push('\n');

        // Goals
        content.push_str("## Goals\n\n");
        for goal in &profile.goals {
            content.push_str(&format!(
                "- **{}** [{:?}] (priority: {}): {}\n",
                goal.description,
                goal.status,
                goal.priority,
                format!("{:.0}% complete", goal.progress * 100.0)
            ));
        }

        tokio::fs::write(path, content).await?;

        Ok(())
    }

    /// Build a compact profile summary suitable for prompt injection
    pub async fn prompt_summary(&self) -> String {
        let profile = self.profile.read().await.clone();
        Self::format_prompt_summary(&profile)
    }

    fn format_prompt_summary(profile: &UserProfileData) -> String {
        let mut lines = vec![
            format!("User: {} ({})", profile.name, profile.user_id),
            format!(
                "Communication: tone={}, detail={}, format={}, languages={}",
                profile.communication_style.tone,
                profile.communication_style.detail_level.as_str(),
                profile.communication_style.format_preference,
                if profile.communication_style.languages.is_empty() {
                    "unspecified".to_string()
                } else {
                    profile.communication_style.languages.join(", ")
                }
            ),
        ];

        if !profile.preferences.is_empty() {
            lines.push("Preferences:".to_string());
            let mut preferences: Vec<_> = profile.preferences.iter().collect();
            preferences.sort_by(|a, b| a.0.cmp(b.0));
            for (key, value) in preferences.into_iter().take(6) {
                lines.push(format!(
                    "- {}: {}",
                    key,
                    Self::format_preference_value(value)
                ));
            }
        }

        if !profile.facts.is_empty() {
            lines.push("Known facts:".to_string());
            let mut facts: Vec<_> = profile.facts.iter().collect();
            facts.sort_by(|a, b| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(Ordering::Equal)
            });
            for fact in facts.into_iter().take(4) {
                lines.push(format!("- [{}] {}", fact.category, fact.content));
            }
        }

        if !profile.goals.is_empty() {
            lines.push("Current goals:".to_string());
            let mut goals: Vec<_> = profile.goals.iter().collect();
            goals.sort_by(|a, b| b.priority.cmp(&a.priority));
            for goal in goals.into_iter().take(3) {
                lines.push(format!("- {} ({:?})", goal.description, goal.status));
            }
        }

        lines.join("\n")
    }

    fn format_preference_value(value: &PreferenceValue) -> String {
        match value {
            PreferenceValue::String(s) => s.clone(),
            PreferenceValue::Number(n) => n.to_string(),
            PreferenceValue::Boolean(b) => b.to_string(),
            PreferenceValue::List(items) => items.join(", "),
            PreferenceValue::Object(map) => {
                let mut items: Vec<_> = map.iter().collect();
                items.sort_by(|a, b| a.0.cmp(b.0));
                items
                    .into_iter()
                    .map(|(key, value)| format!("{}={}", key, value))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
    }

    /// Export memories to Markdown
    pub async fn export_memories_to_markdown(&self, path: &PathBuf) -> Result<(), MemoryError> {
        let memories = self.memories.read().await;

        let mut content = String::new();
        content.push_str("---\n");
        content.push_str("type: memory-store\n");
        content.push_str(&format!("count: {}\n", memories.len()));
        content.push_str(&format!("exported_at: {}\n", Utc::now().to_rfc3339()));
        content.push_str("---\n\n");

        content.push_str("# Memory Store\n\n");

        // Group by category
        let mut by_category: HashMap<String, Vec<&Memory>> = HashMap::new();
        for memory in memories.iter() {
            by_category
                .entry(memory.category.clone())
                .or_default()
                .push(memory);
        }

        for (category, category_memories) in by_category {
            content.push_str(&format!("## {}\n\n", category));

            for memory in category_memories {
                content.push_str(&format!("### {}\n", memory.id));
                content.push_str(&format!("- **Type**: {:?}\n", memory.memory_type));
                content.push_str(&format!(
                    "- **Importance**: {:.0}%\n",
                    memory.importance * 100.0
                ));
                content.push_str(&format!(
                    "- **Created**: {}\n",
                    memory.created_at.format("%Y-%m-%d")
                ));
                content.push_str(&format!("- **Source**: {}\n", memory.source));
                content.push_str(&format!("\n{}\n\n", memory.content));
            }
        }

        tokio::fs::write(path, content).await?;

        Ok(())
    }

    fn extract_session_insights(messages: &[crate::types::Message]) -> SessionInsights {
        let mut insights = SessionInsights::default();

        for text in messages
            .iter()
            .filter(|message| {
                message.role == "user"
                    || (message.role == "system"
                        && message
                            .text()
                            .map(|text| text.contains("Conversation summary"))
                            .unwrap_or(false))
            })
            .filter_map(|message| message.text())
        {
            let normalized = Self::normalize_text(&text);
            if normalized.is_empty() {
                continue;
            }

            if let Some(detail_level) = Self::detect_detail_preference(&normalized) {
                insights.preferences.push(PreferenceCandidate {
                    key: "communication.detail_level".to_string(),
                    value: PreferenceValue::String(detail_level.to_string()),
                });
            }

            if let Some(format_preference) = Self::detect_format_preference(&normalized) {
                insights.preferences.push(PreferenceCandidate {
                    key: "format.preference".to_string(),
                    value: PreferenceValue::String(format_preference.to_string()),
                });
            }

            if let Some(language_preference) = Self::detect_language_preference(&normalized) {
                insights.preferences.push(PreferenceCandidate {
                    key: "communication.languages".to_string(),
                    value: PreferenceValue::List(vec![language_preference.to_string()]),
                });
            }

            if let Some(tone_preference) = Self::detect_tone_preference(&normalized) {
                insights.preferences.push(PreferenceCandidate {
                    key: "communication.tone".to_string(),
                    value: PreferenceValue::String(tone_preference.to_string()),
                });
            }

            if let Some(name) = Self::extract_after_marker(
                &normalized,
                &["my name is ", "i am ", "i'm ", "我叫", "我是"],
            ) {
                insights.facts.push((
                    format!("User identity: {}", name),
                    "identity".to_string(),
                    0.85,
                ));
            }

            if let Some(tooling) = Self::extract_after_marker(
                &normalized,
                &[
                    "i use ",
                    "we use ",
                    "i am using ",
                    "我使用",
                    "我们使用",
                    "我正在使用",
                ],
            ) {
                insights.facts.push((
                    format!("User works with {}", tooling),
                    "tooling".to_string(),
                    0.7,
                ));
            }

            if let Some(project) = Self::extract_after_marker(
                &normalized,
                &["i am working on ", "i'm working on ", "我正在做", "我在做"],
            ) {
                insights.facts.push((
                    format!("Current project: {}", project),
                    "project".to_string(),
                    0.75,
                ));
            }

            if let Some(goal) = Self::extract_after_marker(
                &normalized,
                &[
                    "i want to ",
                    "i need to ",
                    "i plan to ",
                    "my goal is to ",
                    "希望",
                    "想要",
                    "打算",
                    "目标是",
                    "需要",
                ],
            ) {
                insights.goals.push(goal);
            }
        }

        insights
    }

    fn apply_preference(profile: &mut UserProfileData, candidate: PreferenceCandidate) {
        profile
            .preferences
            .insert(candidate.key.clone(), candidate.value.clone());

        match candidate.key.as_str() {
            "communication.detail_level" => {
                if let PreferenceValue::String(value) = candidate.value {
                    profile.communication_style.detail_level = Self::parse_detail_level(&value);
                }
            }
            "format.preference" => {
                if let PreferenceValue::String(value) = candidate.value {
                    profile.communication_style.format_preference = value;
                }
            }
            "communication.languages" => {
                if let PreferenceValue::List(value) = candidate.value {
                    profile.communication_style.languages = value;
                }
            }
            "communication.tone" => {
                if let PreferenceValue::String(value) = candidate.value {
                    profile.communication_style.tone = value;
                }
            }
            _ => {}
        }
    }

    fn merge_fact(
        profile: &mut UserProfileData,
        content: String,
        category: String,
        confidence: f64,
        source: &str,
    ) {
        if let Some(existing) = profile
            .facts
            .iter_mut()
            .find(|fact| fact.content.eq_ignore_ascii_case(&content))
        {
            existing.confidence = existing.confidence.max(confidence);
            existing.reinforcement_count += 1;
            existing.learned_at = Utc::now();
            existing.source = source.to_string();
            return;
        }

        profile.facts.push(UserFact {
            content,
            category,
            confidence,
            learned_at: Utc::now(),
            source: source.to_string(),
            reinforcement_count: 1,
        });
    }

    fn merge_goal(profile: &mut UserProfileData, description: String) {
        if profile
            .goals
            .iter()
            .any(|goal| goal.description.eq_ignore_ascii_case(&description))
        {
            return;
        }

        profile.goals.push(UserGoal {
            description,
            status: GoalStatus::Active,
            priority: 6,
            created_at: Utc::now(),
            target_date: None,
            progress: 0.0,
        });
    }

    fn upsert_session_summary_memory(
        memories: &mut Vec<Memory>,
        index: &mut HashMap<String, Vec<usize>>,
        session_id: &str,
        summary: &crate::memory::fusion::layer_session::SessionSummary,
    ) -> bool {
        let mut summary_memory = create_memory_from_session(
            summary.summary.clone(),
            "session_summary".to_string(),
            session_id.to_string(),
        );
        summary_memory.memory_type = MemoryType::Contextual;
        summary_memory.importance = 0.65;
        summary_memory.related_memories = summary.key_topics.clone();

        if let Some(position) = memories
            .iter()
            .position(|memory| memory.category == "session_summary" && memory.source == session_id)
        {
            memories[position] = summary_memory;
            return false;
        }

        let index_position = memories.len();
        memories.push(summary_memory);
        index
            .entry("session_summary".to_string())
            .or_default()
            .push(index_position);
        true
    }

    fn normalize_text(text: &str) -> String {
        text.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string()
    }

    fn detect_detail_preference(text: &str) -> Option<&'static str> {
        let lower = text.to_lowercase();
        if lower.contains("brief")
            || lower.contains("concise")
            || lower.contains("short")
            || text.contains("简洁")
            || text.contains("简短")
        {
            Some("brief")
        } else if lower.contains("comprehensive")
            || lower.contains("thorough")
            || text.contains("全面")
        {
            Some("comprehensive")
        } else if lower.contains("detailed")
            || lower.contains("deep")
            || text.contains("详细")
            || text.contains("深入")
        {
            Some("detailed")
        } else {
            None
        }
    }

    fn detect_format_preference(text: &str) -> Option<&'static str> {
        let lower = text.to_lowercase();
        if lower.contains("markdown") {
            Some("markdown")
        } else if lower.contains("json") {
            Some("json")
        } else if lower.contains("table") || text.contains("表格") {
            Some("table")
        } else if lower.contains("list") || text.contains("列表") {
            Some("list")
        } else {
            None
        }
    }

    fn detect_language_preference(text: &str) -> Option<&'static str> {
        let lower = text.to_lowercase();
        if lower.contains("english") || text.contains("英文") {
            Some("en")
        } else if lower.contains("chinese") || text.contains("中文") {
            Some("zh")
        } else {
            None
        }
    }

    fn detect_tone_preference(text: &str) -> Option<&'static str> {
        let lower = text.to_lowercase();
        if lower.contains("friendly") || text.contains("友好") {
            Some("friendly")
        } else if lower.contains("formal") || text.contains("正式") {
            Some("formal")
        } else if lower.contains("professional") || text.contains("专业") {
            Some("professional")
        } else if lower.contains("casual") || text.contains("随意") {
            Some("casual")
        } else {
            None
        }
    }

    fn extract_after_marker(text: &str, markers: &[&str]) -> Option<String> {
        let lower = text.to_lowercase();

        for marker in markers {
            let marker_lower = marker.to_lowercase();
            if let Some(start) = lower.find(&marker_lower) {
                let value = text[start + marker.len()..]
                    .trim()
                    .trim_matches(|c: char| matches!(c, '.' | ',' | ';' | ':' | '!' | '?'))
                    .to_string();

                if value.is_empty() {
                    continue;
                }

                return Some(Self::truncate_value(&value, 120));
            }
        }

        None
    }

    fn truncate_value(value: &str, max_chars: usize) -> String {
        if value.chars().count() <= max_chars {
            return value.to_string();
        }

        let mut result = String::new();
        for ch in value.chars().take(max_chars.saturating_sub(3)) {
            result.push(ch);
        }
        result.push_str("...");
        result
    }

    /// Get statistics
    pub async fn stats(&self) -> MemoryStats {
        let memories = self.memories.read().await;
        let profile = self.profile.read().await;

        let size_bytes = serde_json::to_vec(&*memories).map(|v| v.len()).unwrap_or(0);

        MemoryStats {
            layer_name: "USER".to_string(),
            item_count: memories.len(),
            size_bytes,
            last_accessed: profile.metadata.updated_at,
        }
    }
}

/// Create a memory from session content
pub fn create_memory_from_session(content: String, category: String, source: String) -> Memory {
    let now = Utc::now();
    Memory {
        id: format!(
            "mem_{}",
            &uuid::Uuid::new_v4().to_string().replace("-", "")[..16]
        ),
        content,
        memory_type: MemoryType::ExplicitFact,
        category,
        importance: 0.5,
        created_at: now,
        last_accessed: now,
        access_count: 0,
        related_memories: Vec::new(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_config(storage_path: &str) -> UserConfig {
        UserConfig {
            user_id: "test-user".to_string(),
            name: "Test User".to_string(),
            storage_path: storage_path.to_string(),
            preferences: HashMap::new(),
            communication: UserCommunication {
                tone: "friendly".to_string(),
                detail_level: "moderate".to_string(),
                languages: vec!["en".to_string()],
                format_preference: "markdown".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn update_from_session_extracts_preferences_facts_and_goals() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(temp_dir.path().to_str().unwrap());
        let layer = UserLayer::from_config(&config).await.unwrap();

        let session = SessionLayer::new("user-session".to_string(), 1024);
        session
            .add_user_message("My name is Alex and I use Rust every day.".to_string())
            .await
            .unwrap();
        session
            .add_user_message(
                "Please answer briefly in markdown. I want to build a multi-agent framework."
                    .to_string(),
            )
            .await
            .unwrap();

        layer.update_from_session(&session).await.unwrap();

        let profile = layer.get_profile().await;
        assert_eq!(
            profile.preferences.get("communication.detail_level"),
            Some(&PreferenceValue::String("brief".to_string()))
        );
        assert_eq!(
            profile.preferences.get("format.preference"),
            Some(&PreferenceValue::String("markdown".to_string()))
        );
        assert!(profile
            .facts
            .iter()
            .any(|fact| fact.content.contains("Alex")));
        assert!(profile
            .goals
            .iter()
            .any(|goal| goal.description.contains("build a multi-agent framework")));

        let memories = layer.get_all_memories().await;
        assert!(memories
            .iter()
            .any(|memory| memory.category == "session_summary"));
    }

    #[tokio::test]
    async fn consolidate_rebuilds_category_index() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(temp_dir.path().to_str().unwrap());
        let layer = UserLayer::from_config(&config).await.unwrap();

        let now = Utc::now();
        layer
            .record_memory(Memory {
                id: "old-memory".to_string(),
                content: "Legacy project context".to_string(),
                memory_type: MemoryType::Contextual,
                category: "project".to_string(),
                importance: 0.1,
                created_at: now - chrono::Duration::days(120),
                last_accessed: now - chrono::Duration::days(120),
                access_count: 0,
                related_memories: Vec::new(),
                source: "test".to_string(),
            })
            .await
            .unwrap();
        layer
            .record_memory(Memory {
                id: "recent-memory".to_string(),
                content: "Active framework direction".to_string(),
                memory_type: MemoryType::Contextual,
                category: "project".to_string(),
                importance: 0.8,
                created_at: now,
                last_accessed: now,
                access_count: 2,
                related_memories: Vec::new(),
                source: "test".to_string(),
            })
            .await
            .unwrap();

        let removed = layer.consolidate().await.unwrap();
        assert_eq!(removed, 1);

        let project_memories = layer.search_by_category("project", 10).await;
        assert_eq!(project_memories.len(), 1);
        assert_eq!(project_memories[0].id, "recent-memory");

        let profile = layer.get_profile().await;
        assert_eq!(profile.metadata.total_memories, 1);
    }

    #[tokio::test]
    async fn update_from_session_uses_compressed_summary_context() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(temp_dir.path().to_str().unwrap());
        let layer = UserLayer::from_config(&config).await.unwrap();

        let session = SessionLayer::new("compressed-summary-session".to_string(), 24);
        session
            .add_user_message("I want to build a multi-agent framework.".to_string())
            .await
            .unwrap();

        for idx in 0..7 {
            session
                .add_user_message(format!(
                    "This is filler context number {} for the conversation",
                    idx
                ))
                .await
                .unwrap();
        }

        layer.update_from_session(&session).await.unwrap();

        let profile = layer.get_profile().await;
        assert!(profile
            .goals
            .iter()
            .any(|goal| goal.description.contains("build a multi-agent framework")));
    }

    #[tokio::test]
    async fn search_relevant_context_reinforces_hits() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(temp_dir.path().to_str().unwrap());
        let layer = UserLayer::from_config(&config).await.unwrap();

        layer
            .record_memory(Memory {
                id: "hit-memory".to_string(),
                content: "Rust agent routing preference".to_string(),
                memory_type: MemoryType::ExplicitFact,
                category: "routing".to_string(),
                importance: 0.9,
                created_at: Utc::now(),
                last_accessed: Utc::now() - chrono::Duration::days(7),
                access_count: 0,
                related_memories: Vec::new(),
                source: "test".to_string(),
            })
            .await
            .unwrap();

        let before = layer.get_all_memories().await;
        let before_memory = before
            .iter()
            .find(|memory| memory.id == "hit-memory")
            .unwrap()
            .clone();

        let hits = layer.search_relevant_context(1).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "hit-memory");

        let after = layer.get_all_memories().await;
        let after_memory = after
            .iter()
            .find(|memory| memory.id == "hit-memory")
            .unwrap();

        assert_eq!(after_memory.access_count, before_memory.access_count + 1);
        assert!(after_memory.last_accessed >= before_memory.last_accessed);
    }

    #[tokio::test]
    async fn prompt_summary_includes_profile_state() {
        let temp_dir = TempDir::new().unwrap();
        let config = test_config(temp_dir.path().to_str().unwrap());
        let layer = UserLayer::from_config(&config).await.unwrap();

        let summary = layer.prompt_summary().await;
        assert!(summary.contains("User: Test User"));
        assert!(summary.contains("Communication:"));
        assert!(summary.contains("friendly"));
        assert!(summary.contains("markdown"));
    }
}
