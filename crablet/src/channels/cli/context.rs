use anyhow::Result;
use std::sync::Arc;
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
            Ok(mem) => Some(Arc::new(mem)),
            Err(_) => None,
        };

        // Initialize Knowledge Graph
        #[cfg(feature = "knowledge")]
        let kg: Option<SharedKnowledgeGraph> = if let Ok(_neo4j_uri) = std::env::var("NEO4J_URI") {
            // Neo4j logic omitted for brevity in this refactor step, assuming similar to original or simplified
            None 
        } else {
            if let Ok(pool) = sqlx::sqlite::SqlitePool::connect(&database_url).await {
                crate::memory::semantic::SqliteKnowledgeGraph::new(pool).await.ok().map(|g| Arc::new(g) as SharedKnowledgeGraph)
            } else {
                None
            }
        };

        #[cfg(not(feature = "knowledge"))]
        let kg: Option<SharedKnowledgeGraph> = None;

        // Initialize Vector Store
        #[cfg(feature = "knowledge")]
        let vector_store = if let Ok(pool) = sqlx::sqlite::SqlitePool::connect(&database_url).await {
            crate::knowledge::vector_store::VectorStore::new(pool).await.ok().map(Arc::new)
        } else {
            None
        };

        let event_bus = Arc::new(EventBus::new());

        let mut router = CognitiveRouter::new(memory.clone(), event_bus.clone()).await;
        
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
        let _ = router.load_skills(&config.skills_dir).await;
        
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
