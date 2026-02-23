use tokio::sync::mpsc;
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{info, warn};
use crate::cognitive::router::CognitiveRouter;
use tokio::sync::oneshot;

pub struct LaneTask {
    pub input: String,
    pub session_id: String,
    pub response_tx: oneshot::Sender<anyhow::Result<(String, Vec<crate::types::TraceStep>)>>,
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

    pub async fn dispatch(&self, session_id: &str, input: String) -> anyhow::Result<(String, Vec<crate::types::TraceStep>)> {
        let lane = self.lanes.entry(session_id.to_string()).or_insert_with(|| {
            let (tx, mut rx) = mpsc::channel::<LaneTask>(100);
            let router = self.cognitive_router.clone();
            let sid = session_id.to_string();
            
            let handle = tokio::spawn(async move {
                info!("Starting Lane Queue for session: {}", sid);
                while let Some(task) = rx.recv().await {
                    let result = router.process(&task.input, &task.session_id).await;
                    if let Err(_) = task.response_tx.send(result) {
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

        lane.tx.send(task).await.map_err(|_| anyhow::anyhow!("Failed to send task to lane queue"))?;
        
        // Wait for result from the lane consumer
        resp_rx.await?
    }
}
