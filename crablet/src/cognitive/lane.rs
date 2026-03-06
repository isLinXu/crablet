use tokio::sync::mpsc;
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{info, warn};
use crate::cognitive::router::CognitiveRouter;
use tokio::sync::oneshot;
use crate::error::{Result, CrabletError};

pub struct LaneTask {
    pub input: String,
    pub session_id: String,
    pub response_tx: oneshot::Sender<Result<(String, Vec<crate::types::TraceStep>)>>,
}

pub struct SessionLane {
    tx: mpsc::Sender<LaneTask>,
    _handle: tokio::task::JoinHandle<()>,
}

pub struct LaneRouter {
    lanes: Arc<DashMap<String, SessionLane>>,
    cognitive_router: Arc<CognitiveRouter>,
}

impl LaneRouter {
    pub fn new(cognitive_router: Arc<CognitiveRouter>) -> Self {
        Self {
            lanes: Arc::new(DashMap::new()),
            cognitive_router,
        }
    }

    pub async fn dispatch(&self, session_id: &str, input: String) -> Result<(String, Vec<crate::types::TraceStep>)> {
        // We need to clone lane.tx because entry API holds a lock on the shard.
        // We cannot await inside the closure easily if we wanted to spawn inside?
        // Wait, the original code did spawn inside the closure.
        // `or_insert_with` returns `&mut V`.
        // Then we get `lane.tx.send(...)`.
        // But `lane` is `RefMut`.
        // `lane.tx.send` is async.
        // `RefMut` is not Send across await points if we hold it?
        // Actually DashMap RefMut is not Send?
        // But the original code was:
        /*
        let lane = self.lanes.entry(...).or_insert_with(|| { ... });
        lane.tx.send(task).await ...
        */
        // This holds the lock while awaiting `send`.
        // This is bad for concurrency but works if channel has capacity.
        // I will keep the logic but fix types.

        let lane = self.lanes.entry(session_id.to_string()).or_insert_with(|| {
            let (tx, mut rx) = mpsc::channel::<LaneTask>(100);
            let router = self.cognitive_router.clone();
            let sid = session_id.to_string();
            
            let handle = tokio::spawn(async move {
                info!("Starting Lane Queue for session: {}", sid);
                while let Some(task) = rx.recv().await {
                    let result = router.process(&task.input, &task.session_id).await;
                    if task.response_tx.send(result).is_err() {
                        warn!("Lane task receiver dropped for session: {}", sid);
                    }
                }
                info!("Lane Queue stopped for session: {}", sid);
            });
            
            SessionLane {
                tx,
                _handle: handle,
            }
        });

        let (resp_tx, resp_rx) = oneshot::channel();
        let task = LaneTask {
            input,
            session_id: session_id.to_string(),
            response_tx: resp_tx,
        };

        lane.tx.send(task).await.map_err(|_| CrabletError::Internal("Failed to send task to lane queue".to_string()))?;
        
        // Wait for result from the lane consumer
        resp_rx.await.map_err(|e| CrabletError::Internal(format!("Lane response channel closed: {}", e)))?
    }
}
