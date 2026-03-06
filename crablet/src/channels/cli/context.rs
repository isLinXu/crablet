use anyhow::Result;
use std::sync::Arc;
use tracing::{info, warn, error};
use crate::config::Config;
use crate::cognitive::router::CognitiveRouter;
use crate::memory::episodic::EpisodicMemory;
use crate::memory::semantic::SharedKnowledgeGraph;
#[cfg(feature = "knowledge")]
use crate::knowledge::vector_store::VectorStore;
use crate::events::EventBus;

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
    pub async fn new(config: Config) -> Result<Self> {
        let database_url = config.database_url.clone();
        
        // Initialize Episodic Memory
        let memory = match EpisodicMemory::new(&database_url).await {
            Ok(mem) => {
                info!("Episodic memory initialized successfully");
                Some(Arc::new(mem))
            },
            Err(e) => {
                warn!("Episodic memory unavailable, running in stateless mode: {}", e);
                None
            },
        };

        // Initialize Knowledge Graph
        #[cfg(feature = "knowledge")]
        let kg: Option<SharedKnowledgeGraph> = if let Ok(neo4j_uri) = std::env::var("NEO4J_URI") {
             info!("Connecting to Neo4j at {}", neo4j_uri);
             None 
        } else {
            match sqlx::sqlite::SqlitePool::connect(&database_url).await {
                Ok(pool) => {
                    match crate::memory::semantic::SqliteKnowledgeGraph::new(pool).await {
                        Ok(g) => {
                            info!("SQLite Knowledge Graph initialized");
                            Some(Arc::new(g) as SharedKnowledgeGraph)
                        },
                        Err(e) => {
                            warn!("Failed to initialize Knowledge Graph: {}", e);
                            None
                        }
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
            Ok(pool) => {
                match crate::knowledge::vector_store::VectorStore::new(pool).await {
                    Ok(vs) => {
                        info!("Vector Store initialized");
                        Some(Arc::new(vs))
                    },
                    Err(e) => {
                        warn!("Failed to initialize Vector Store: {}", e);
                        None
                    }
                }
            },
            Err(e) => {
                warn!("Failed to connect to SQLite for Vector Store: {}", e);
                None
            }
        };

        let event_bus = match sqlx::sqlite::SqlitePool::connect(&database_url).await {
            Ok(pool) => {
                Arc::new(EventBus::new(100).with_pool(pool))
            },
            Err(e) => {
                warn!("Failed to connect to SQLite for EventBus: {}", e);
                Arc::new(EventBus::new(100))
            }
        };

        let mut router = CognitiveRouter::new(&config, memory.clone(), event_bus.clone()).await;
        
        #[cfg(feature = "knowledge")]
        {
            router = router.with_knowledge(kg.clone(), vector_store.clone());
        }
        #[cfg(not(feature = "knowledge"))]
        {
            router = router.with_knowledge(kg.clone());
        }
        
        let router = router
            .with_config(&config)
            .watch_skills(&config.skills_dir);
            
        // Load skills
        if let Err(e) = router.load_skills(&config.skills_dir).await {
            error!("Failed to load skills from {}: {}", config.skills_dir.display(), e);
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
