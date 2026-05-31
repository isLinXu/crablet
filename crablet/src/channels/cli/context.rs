use crate::cognitive::router::CognitiveRouter;
use crate::config::Config;
use crate::events::EventBus;
#[cfg(feature = "knowledge")]
use crate::knowledge::vector_store::VectorStore;
use crate::memory::episodic::EpisodicMemory;
use crate::memory::fusion::{
    self,
    daily_logs::DailyLogsConfig,
    layer_soul::{AgentIdentityConfig, CoreValue, ImmutableRule, SoulConfig, SoulMetadataConfig},
    layer_tools::ToolsConfig,
    layer_user::{Preference, UserCommunication, UserConfig},
    weaver::SemanticMemoryConfig,
};
use crate::memory::semantic::SharedKnowledgeGraph;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::cognitive::lane::LaneRouter;

pub struct AppContext {
    pub config: Config,
    pub memory: Option<Arc<EpisodicMemory>>,
    pub kg: Option<SharedKnowledgeGraph>,
    #[cfg(feature = "knowledge")]
    pub vector_store: Option<Arc<VectorStore>>,
    pub event_bus: Arc<EventBus>,
    pub router: Arc<CognitiveRouter>,
    pub lane_router: Arc<LaneRouter>,
}

impl AppContext {
    fn fusion_runtime_config(config: &Config) -> Arc<fusion::FusionConfig> {
        let workspace_root = config
            .skills_dir
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let fusion_root = workspace_root.join(".crablet").join("fusion-memory");

        Arc::new(fusion::FusionConfig {
            soul: SoulConfig {
                identity: AgentIdentityConfig {
                    name: "Crablet".to_string(),
                    description: "OpenClaw-style fusion memory runtime".to_string(),
                    role: "agent_framework".to_string(),
                    version: "0.1.0".to_string(),
                },
                core_values: vec![CoreValue {
                    name: "user_first".to_string(),
                    description: "Preserve user context and preferences across sessions."
                        .to_string(),
                    priority: 10,
                    category: "alignment".to_string(),
                }],
                immutable_rules: vec![ImmutableRule {
                    rule: "Never discard important user context without explicit policy."
                        .to_string(),
                    reason: Some("Fusion memory should be reliable and auditable.".to_string()),
                }],
                metadata: SoulMetadataConfig {
                    created_at: chrono::Utc::now().to_rfc3339(),
                    updated_at: chrono::Utc::now().to_rfc3339(),
                    author: "crablet-runtime".to_string(),
                },
            },
            tools: ToolsConfig {
                available_tools: Vec::new(),
                permissions: Vec::new(),
                tool_chains: Vec::new(),
            },
            user: UserConfig {
                user_id: "default-user".to_string(),
                name: "Crablet User".to_string(),
                storage_path: fusion_root.join("user").to_string_lossy().to_string(),
                preferences: HashMap::from([(
                    "language".to_string(),
                    Preference {
                        value: "zh-CN".to_string(),
                        value_type: "string".to_string(),
                    },
                )]),
                communication: UserCommunication {
                    tone: "balanced".to_string(),
                    detail_level: "moderate".to_string(),
                    languages: vec!["zh-CN".to_string(), "en".to_string()],
                    format_preference: "markdown".to_string(),
                },
            },
            memory: fusion::MemoryConfig {
                working: fusion::WorkingMemoryConfig {
                    max_tokens: 8_000,
                    capacity_messages: 32,
                },
                daily_logs: DailyLogsConfig {
                    enabled: true,
                    storage_path: fusion_root.join("daily_logs").to_string_lossy().to_string(),
                    context_window_days: 7,
                    auto_extract_memories: true,
                },
                semantic: SemanticMemoryConfig {
                    backend: "local".to_string(),
                    enabled: true,
                },
            },
        })
    }

    pub async fn new(config: Config) -> Result<Self> {
        let database_url = config.database_url.clone();

        // Initialize Episodic Memory
        let memory = match EpisodicMemory::new(&database_url).await {
            Ok(mem) => {
                info!("Episodic memory initialized successfully");
                Some(Arc::new(mem))
            }
            Err(e) => {
                warn!(
                    "Episodic memory unavailable, running in stateless mode: {}",
                    e
                );
                None
            }
        };

        // Initialize Knowledge Graph
        #[cfg(feature = "knowledge")]
        let kg: Option<SharedKnowledgeGraph> = if let Ok(neo4j_uri) = std::env::var("NEO4J_URI") {
            info!("Connecting to Neo4j at {}", neo4j_uri);
            None
        } else {
            match sqlx::sqlite::SqlitePool::connect(&database_url).await {
                Ok(pool) => match crate::memory::semantic::SqliteKnowledgeGraph::new(pool).await {
                    Ok(g) => {
                        info!("SQLite Knowledge Graph initialized");
                        Some(Arc::new(g) as SharedKnowledgeGraph)
                    }
                    Err(e) => {
                        warn!("Failed to initialize Knowledge Graph: {}", e);
                        None
                    }
                },
                Err(e) => {
                    warn!("Failed to connect to SQLite for KG: {}", e);
                    None
                }
            }
        };

        #[cfg(not(feature = "knowledge"))]
        let kg: Option<SharedKnowledgeGraph> = None;

        // Initialize Vector Store
        #[cfg(feature = "knowledge")]
        let vector_store = match sqlx::sqlite::SqlitePool::connect(&database_url).await {
            Ok(pool) => match crate::knowledge::vector_store::VectorStore::new(pool).await {
                Ok(vs) => {
                    info!("Vector Store initialized");
                    Some(Arc::new(vs))
                }
                Err(e) => {
                    warn!("Failed to initialize Vector Store: {}", e);
                    None
                }
            },
            Err(e) => {
                warn!("Failed to connect to SQLite for Vector Store: {}", e);
                None
            }
        };

        let event_bus = match sqlx::sqlite::SqlitePool::connect(&database_url).await {
            Ok(pool) => Arc::new(EventBus::new(100).with_pool(pool)),
            Err(e) => {
                warn!("Failed to connect to SQLite for EventBus: {}", e);
                Arc::new(EventBus::new(100))
            }
        };

        let router = CognitiveRouter::new(&config, memory.clone(), event_bus.clone()).await;
        let mut router = match router
            .with_fusion_memory(Self::fusion_runtime_config(&config))
            .await
        {
            Ok(router) => {
                info!("Fusion memory initialized successfully");
                router
            }
            Err(err) => {
                warn!(
                    "Fusion memory unavailable, continuing with legacy memory manager: {}",
                    err
                );
                CognitiveRouter::new(&config, memory.clone(), event_bus.clone()).await
            }
        };

        #[cfg(feature = "knowledge")]
        {
            router = router.with_knowledge(kg.clone(), vector_store.clone());
        }
        #[cfg(not(feature = "knowledge"))]
        {
            router = router.with_knowledge(kg.clone());
        }

        let router = router.with_config(&config).watch_skills(&config.skills_dir);

        // Load skills
        if let Err(e) = router.load_skills(&config.skills_dir).await {
            error!(
                "Failed to load skills from {}: {}",
                config.skills_dir.display(),
                e
            );
        } else {
            info!("Skills loaded from {}", config.skills_dir.display());
        }

        let router = Arc::new(router);
        let lane_router = Arc::new(LaneRouter::new(router.clone()));

        Ok(Self {
            config,
            memory,
            kg,
            #[cfg(feature = "knowledge")]
            vector_store,
            event_bus,
            router,
            lane_router,
        })
    }
}
