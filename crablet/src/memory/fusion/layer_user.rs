//! L2: USER Layer - Semantic Long-term Memory
//!
//! The USER layer stores semantic long-term memory about the user, including:
//! - User profile and preferences
//! - Important facts and decisions
//! - Conversation history summaries
//! - Learned patterns and behaviors

use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use tracing::{info, debug};

use crate::memory::fusion::{MemoryError, MemoryStats};
use crate::memory::fusion::layer_session::SessionLayer;

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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    fn convert_preferences(prefs: &HashMap<String, Preference>) -> HashMap<String, PreferenceValue> {
        prefs
            .iter()
            .map(|(k, v): (&String, &Preference)| {
                let value = match v.value_type.as_str() {
                    "string" => PreferenceValue::String(v.value.clone()),
                    "number" => PreferenceValue::Number(v.value.parse().unwrap_or(0.0)),
                    "boolean" => PreferenceValue::Boolean(v.value.parse().unwrap_or(false)),
                    "list" => PreferenceValue::List(v.value.split(',').map(|s: &str| s.trim().to_string()).collect()),
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
            index
                .entry(memory.category.clone())
                .or_default()
                .push(idx);
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
        index
            .entry(memory.category.clone())
            .or_default()
            .push(idx);
        
        // Update profile metadata
        profile.metadata.total_memories += 1;
        profile.metadata.updated_at = Utc::now();
        
        // Persist
        drop(memories);
        drop(index);
        drop(profile);
        self.persist_memories().await?;
        
        debug!("Recorded memory: {} (category: {})", memory.id, memory.category);
        Ok(())
    }
    
    /// Search for relevant memories
    pub async fn search_relevant_context(&self, limit: usize) -> Result<Vec<Memory>, MemoryError> {
        let memories = self.memories.read().await;
        
        // Simple relevance scoring based on importance and recency
        let mut scored: Vec<(f64, &Memory)> = memories
            .iter()
            .map(|m| {
                let recency_score = Self::calculate_recency_score(&m.last_accessed);
                let importance_score = m.importance;
                let access_score = (m.access_count as f64 / 100.0).min(1.0);
                
                let total_score = importance_score * 0.4 + recency_score * 0.4 + access_score * 0.2;
                (total_score, m)
            })
            .collect();
        
        // Sort by score (descending)
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        
        // Take top results
        let results: Vec<Memory> = scored
            .into_iter()
            .take(limit)
            .map(|(_, m)| m.clone())
            .collect();
        
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
    pub async fn update_from_session(&self, _session: &SessionLayer) -> Result<(), MemoryError> {
        let mut profile = self.profile.write().await;
        
        // Update interaction count
        profile.metadata.total_interactions += 1;
        profile.metadata.updated_at = Utc::now();
        
        // Extract facts from session (simplified)
        // In a real implementation, this would use NLP to extract facts
        
        // Persist
        drop(profile);
        self.persist_profile().await?;
        
        Ok(())
    }
    
    /// Add a user fact
    pub async fn add_fact(&self, content: String, category: String, confidence: f64) -> Result<(), MemoryError> {
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
    pub async fn update_preference(&self, key: String, value: PreferenceValue) -> Result<(), MemoryError> {
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
        
        memories.retain(|m| {
            m.importance > 0.3 || m.last_accessed > cutoff
        });
        
        let removed = original_count - memories.len();
        
        drop(memories);
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
        content.push_str(&format!("created_at: {}\n", profile.metadata.created_at.to_rfc3339()));
        content.push_str(&format!("updated_at: {}\n", profile.metadata.updated_at.to_rfc3339()));
        content.push_str(&format!("total_interactions: {}\n", profile.metadata.total_interactions));
        content.push_str(&format!("total_memories: {}\n", profile.metadata.total_memories));
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
        content.push_str(&format!("- **Tone**: {}\n", profile.communication_style.tone));
        content.push_str(&format!("- **Detail Level**: {:?}\n", profile.communication_style.detail_level));
        content.push_str(&format!("- **Languages**: {}\n", profile.communication_style.languages.join(", ")));
        content.push_str(&format!("- **Format**: {}\n", profile.communication_style.format_preference));
        content.push('\n');
        
        // Facts
        content.push_str("## Facts\n\n");
        for fact in &profile.facts {
            content.push_str(&format!("- **{}** (confidence: {:.0}%): {}\n", 
                fact.category, 
                fact.confidence * 100.0,
                fact.content
            ));
        }
        content.push('\n');
        
        // Goals
        content.push_str("## Goals\n\n");
        for goal in &profile.goals {
            content.push_str(&format!("- **{}** [{:?}] (priority: {}): {}\n",
                goal.description,
                goal.status,
                goal.priority,
                format!("{:.0}% complete", goal.progress * 100.0)
            ));
        }
        
        tokio::fs::write(path, content).await?;
        
        Ok(())
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
                content.push_str(&format!("- **Importance**: {:.0}%\n", memory.importance * 100.0));
                content.push_str(&format!("- **Created**: {}\n", memory.created_at.format("%Y-%m-%d")));
                content.push_str(&format!("- **Source**: {}\n", memory.source));
                content.push_str(&format!("\n{}\n\n", memory.content));
            }
        }
        
        tokio::fs::write(path, content).await?;
        
        Ok(())
    }
    
    /// Get statistics
    pub async fn stats(&self) -> MemoryStats {
        let memories = self.memories.read().await;
        let profile = self.profile.read().await;
        
        let size_bytes = serde_json::to_vec(&*memories)
            .map(|v| v.len())
            .unwrap_or(0);
        
        MemoryStats {
            layer_name: "USER".to_string(),
            item_count: memories.len(),
            size_bytes,
            last_accessed: profile.metadata.updated_at,
        }
    }
}

/// Create a memory from session content
pub fn create_memory_from_session(
    content: String,
    category: String,
    source: String,
) -> Memory {
    let now = Utc::now();
    Memory {
        id: format!("mem_{}", uuid::Uuid::new_v4().to_string().replace("-", "")[..16].to_string()),
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
