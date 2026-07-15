use super::*;
use crate::agent::capability::CapabilityRouter;
use crate::agent::factory::AgentFactory;
use crate::agent::harness::{AgentHarnessContext, HarnessConfig};
use crate::events::EventBus;
use crate::testing::mocks::MockLlmClient;
use sqlx::sqlite::SqlitePoolOptions;
use tokio::time::{sleep, Duration, Instant};

async fn init_test_pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::query(
        "CREATE TABLE swarm_graphs (
                id TEXT PRIMARY KEY,
                goal TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE swarm_tasks (
                id TEXT PRIMARY KEY,
                graph_id TEXT NOT NULL,
                agent_role TEXT NOT NULL,
                prompt TEXT NOT NULL,
                dependencies TEXT NOT NULL,
                status TEXT NOT NULL,
                result TEXT,
                logs TEXT NOT NULL,
                execution_state TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        "CREATE TABLE swarm_templates (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                graph_json TEXT NOT NULL,
                created_at INTEGER NOT NULL
            )",
    )
    .execute(&pool)
    .await
    .unwrap();

    pool
}

fn build_test_coordinator(pool: sqlx::SqlitePool, responses: &[&str]) -> SwarmCoordinator {
    let mut mock_llm = MockLlmClient::new();
    for response in responses {
        mock_llm = mock_llm.with_response(response);
    }

    let llm = Arc::new(mock_llm) as Arc<dyn crate::cognitive::llm::LlmClient>;
    let event_bus = Arc::new(EventBus::new(100));
    let factory = Arc::new(AgentFactory::new(llm.clone(), event_bus));
    let persister = Arc::new(SwarmPersister::new(Some(pool)));
    let executor = Arc::new(SwarmExecutor::new(
        llm.clone(),
        factory,
        Arc::new(CapabilityRouter::new()),
        None,
        persister.clone(),
    ));

    SwarmCoordinator::new(llm, executor, persister)
}

#[tokio::test]
async fn init_resumes_active_graphs_from_persistence() {
    let pool = init_test_pool().await;
    let seed_persister = Arc::new(SwarmPersister::new(Some(pool.clone())));

    let mut graph = TaskGraph::new().with_goal("resume swarm".to_string());
    graph.add_task(
        "task-1".to_string(),
        "coder".to_string(),
        "debug rust code".to_string(),
        vec![],
    );
    seed_persister
        .persist_graph("graph-1", &graph, "resume swarm")
        .await
        .unwrap();

    let llm = Arc::new(MockLlmClient::new().with_response("resumed result"))
        as Arc<dyn crate::cognitive::llm::LlmClient>;
    let event_bus = Arc::new(EventBus::new(100));
    let factory = Arc::new(AgentFactory::new(llm.clone(), event_bus));
    let persister = Arc::new(SwarmPersister::new(Some(pool)));
    let executor = Arc::new(SwarmExecutor::new(
        llm.clone(),
        factory,
        Arc::new(CapabilityRouter::new()),
        None,
        persister.clone(),
    ));
    let coordinator = SwarmCoordinator::new(llm, executor, persister);

    coordinator.init().await.unwrap();

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let graph = coordinator
            .active_graphs
            .read()
            .await
            .get("graph-1")
            .cloned()
            .unwrap();

        if matches!(graph.status, GraphStatus::Completed) {
            let node = graph.nodes.get("task-1").unwrap();
            assert_eq!(node.result.as_deref(), Some("resumed result"));
            break;
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for graph recovery"
        );
        sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn pause_graph_waits_for_running_tasks_to_quiesce() {
    let pool = init_test_pool().await;
    let coordinator = build_test_coordinator(pool.clone(), &[]);

    let mut graph = TaskGraph::new().with_goal("pause test".to_string());
    graph.add_task(
        "task-1".to_string(),
        "coder".to_string(),
        "finish the in-flight work".to_string(),
        vec![],
    );
    {
        let task = graph.nodes.get_mut("task-1").unwrap();
        task.status = TaskStatus::Running { started_at: 1 };
        task.timeout_ms = 500;
    }

    coordinator
        .persister
        .persist_graph("graph-pause", &graph, "pause test")
        .await
        .unwrap();
    coordinator
        .active_graphs
        .write()
        .await
        .insert("graph-pause".to_string(), graph);

    let active_graphs = coordinator.active_graphs.clone();
    tokio::spawn(async move {
        sleep(Duration::from_millis(120)).await;
        let mut graphs = active_graphs.write().await;
        let graph = graphs.get_mut("graph-pause").unwrap();
        let task = graph.nodes.get_mut("task-1").unwrap();
        task.status = TaskStatus::Completed { duration: 120 };
        task.result = Some("done".to_string());
    });

    let started_at = Instant::now();
    let result = coordinator.pause_graph("graph-pause").await.unwrap();

    assert!(result.quiesced);
    assert_eq!(result.running_tasks, 0);
    assert!(
        started_at.elapsed() >= Duration::from_millis(100),
        "pause_graph returned before the running task drained"
    );

    let graph = coordinator
        .active_graphs
        .read()
        .await
        .get("graph-pause")
        .cloned()
        .unwrap();
    assert!(matches!(graph.status, GraphStatus::Paused));
    assert!(matches!(
        graph.nodes.get("task-1").unwrap().status,
        TaskStatus::Completed { .. }
    ));
}

#[tokio::test]
async fn update_node_rejects_when_pause_is_still_draining() {
    let pool = init_test_pool().await;
    let coordinator = build_test_coordinator(pool.clone(), &[]);

    let mut graph = TaskGraph::new().with_goal("draining update".to_string());
    graph.add_task(
        "task-1".to_string(),
        "coder".to_string(),
        "running work".to_string(),
        vec![],
    );
    graph.add_task(
        "task-2".to_string(),
        "planner".to_string(),
        "pending work".to_string(),
        vec![],
    );
    graph.status = GraphStatus::Paused;
    {
        let task = graph.nodes.get_mut("task-1").unwrap();
        task.status = TaskStatus::Running { started_at: 1 };
    }

    coordinator
        .persister
        .persist_graph("graph-draining", &graph, "draining update")
        .await
        .unwrap();
    coordinator
        .active_graphs
        .write()
        .await
        .insert("graph-draining".to_string(), graph);

    let error = coordinator
        .update_node("graph-draining", "task-2", "new prompt".to_string(), None)
        .await
        .unwrap_err();

    assert!(error.to_string().contains("pause is still draining"));
}

#[tokio::test]
async fn cancel_graph_cancels_running_harness_and_marks_tasks_cancelled() {
    let pool = init_test_pool().await;
    let coordinator = build_test_coordinator(pool.clone(), &[]);

    let mut graph = TaskGraph::new().with_goal("cancel running work".to_string());
    graph.add_task(
        "task-1".to_string(),
        "coder".to_string(),
        "running work".to_string(),
        vec![],
    );
    {
        let task = graph.nodes.get_mut("task-1").unwrap();
        task.status = TaskStatus::Running { started_at: 1 };
        task.execution_state = Some(crate::agent::harness_agent::HarnessExecutionState::new(
            "running work",
            &[],
            None,
        ));
    }

    coordinator
        .persister
        .persist_graph("graph-cancel", &graph, "cancel running work")
        .await
        .unwrap();
    coordinator
        .active_graphs
        .write()
        .await
        .insert("graph-cancel".to_string(), graph);

    let harness = Arc::new(RwLock::new(AgentHarnessContext::new(
        HarnessConfig::default(),
    )));
    coordinator.executor.register_running_harness_for_test(
        "graph-cancel",
        "task-1",
        harness.clone(),
    );

    let cancelled_tasks = coordinator.cancel_graph("graph-cancel").await.unwrap();
    assert_eq!(cancelled_tasks, 1);

    {
        let harness = harness.read().await;
        assert!(harness.should_stop());
        assert!(harness.metadata().cancelled);
    }

    let graph = coordinator
        .active_graphs
        .read()
        .await
        .get("graph-cancel")
        .cloned()
        .unwrap();
    let task = graph.nodes.get("task-1").unwrap();
    assert!(matches!(graph.status, GraphStatus::Paused));
    assert!(matches!(task.status, TaskStatus::Cancelled { .. }));
    assert!(task.result.is_none());
    assert!(task.execution_state.is_none());
    assert!(task
        .logs
        .iter()
        .any(|log| log.contains("Execution cancelled")));
}

#[tokio::test]
async fn resume_graph_requeues_cancelled_tasks_and_respawns() {
    let pool = init_test_pool().await;
    let coordinator = build_test_coordinator(pool.clone(), &["resumed after cancel"]);

    let mut graph = TaskGraph::new().with_goal("resume cancelled task".to_string());
    graph.add_task(
        "task-1".to_string(),
        "coder".to_string(),
        "debug rust code".to_string(),
        vec![],
    );
    {
        let task = graph.nodes.get_mut("task-1").unwrap();
        task.status = TaskStatus::Cancelled {
            cancelled_at: 1,
            reason: "manual cancel".to_string(),
        };
        task.logs
            .push("Execution cancelled previously.".to_string());
    }
    graph.status = GraphStatus::Paused;

    coordinator
        .persister
        .persist_graph("graph-resume-cancelled", &graph, "resume cancelled task")
        .await
        .unwrap();
    coordinator
        .active_graphs
        .write()
        .await
        .insert("graph-resume-cancelled".to_string(), graph);

    let spawned = coordinator
        .resume_graph("graph-resume-cancelled")
        .await
        .unwrap();
    assert!(spawned);

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let graph = coordinator
            .active_graphs
            .read()
            .await
            .get("graph-resume-cancelled")
            .cloned()
            .unwrap();

        if matches!(graph.status, GraphStatus::Completed) {
            let task = graph.nodes.get("task-1").unwrap();
            assert_eq!(task.result.as_deref(), Some("resumed after cancel"));
            assert!(task.logs.iter().any(|log| {
                log.contains("Task reset to Pending when graph resumed after cancellation")
            }));
            break;
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for resumed cancelled task: status={:?}, node={:?}",
            graph.status,
            graph.nodes.get("task-1")
        );
        sleep(Duration::from_millis(20)).await;
    }

    let stored_status: String = sqlx::query_scalar("SELECT status FROM swarm_tasks WHERE id = ?")
        .bind("task-1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(stored_status.contains("\"Completed\""));
}

#[tokio::test]
async fn recover_node_applies_overrides_when_paused() {
    let pool = init_test_pool().await;
    let coordinator = build_test_coordinator(pool.clone(), &[]);

    let mut graph = TaskGraph::new().with_goal("recover paused node".to_string());
    graph.add_task(
        "task-0".to_string(),
        "planner".to_string(),
        "prepare the recovery plan".to_string(),
        vec![],
    );
    graph.add_task(
        "task-1".to_string(),
        "coder".to_string(),
        "old implementation prompt".to_string(),
        vec!["task-0".to_string()],
    );
    graph.add_task(
        "task-2".to_string(),
        "reviewer".to_string(),
        "verify the old implementation".to_string(),
        vec!["task-1".to_string()],
    );

    graph.nodes.get_mut("task-0").unwrap().status = TaskStatus::Completed { duration: 5 };
    {
        let task = graph.nodes.get_mut("task-1").unwrap();
        task.status = TaskStatus::Cancelled {
            cancelled_at: 1,
            reason: "manual cancel".to_string(),
        };
        task.result = Some("stale result".to_string());
    }
    {
        let task = graph.nodes.get_mut("task-2").unwrap();
        task.status = TaskStatus::Completed { duration: 5 };
        task.result = Some("old downstream result".to_string());
    }
    graph.status = GraphStatus::Paused;

    coordinator
        .persister
        .persist_graph("graph-recover-overrides", &graph, "recover paused node")
        .await
        .unwrap();
    coordinator
        .active_graphs
        .write()
        .await
        .insert("graph-recover-overrides".to_string(), graph);

    let started = coordinator
        .recover_node(
            "graph-recover-overrides",
            "task-1",
            NodeRecoveryOptions {
                agent_role: Some("reviewer".to_string()),
                prompt: Some("review and fix the implementation".to_string()),
                dependencies: Some(vec![]),
                resume_graph: false,
            },
        )
        .await
        .unwrap();
    assert!(!started);

    let graph = coordinator
        .active_graphs
        .read()
        .await
        .get("graph-recover-overrides")
        .cloned()
        .unwrap();
    let task_1 = graph.nodes.get("task-1").unwrap();
    let task_2 = graph.nodes.get("task-2").unwrap();

    assert!(matches!(graph.status, GraphStatus::Paused));
    assert!(matches!(task_1.status, TaskStatus::Pending));
    assert_eq!(task_1.agent_role, "reviewer");
    assert_eq!(task_1.prompt, "review and fix the implementation");
    assert!(task_1.dependencies.is_empty());
    assert!(task_1.result.is_none());
    assert!(task_1
        .logs
        .iter()
        .any(|log| log.contains("Recovery overrides applied")));

    assert!(matches!(task_2.status, TaskStatus::Pending));
    assert!(task_2.result.is_none());
    assert!(task_2
        .logs
        .iter()
        .any(|log| log.contains("upstream task task-1 is being recovered")));

    let stored = coordinator.persister.load_active_graphs().await.unwrap();
    let stored_graph = stored.get("graph-recover-overrides").unwrap();
    let stored_task_1 = stored_graph.nodes.get("task-1").unwrap();
    assert_eq!(stored_task_1.agent_role, "reviewer");
    assert_eq!(stored_task_1.prompt, "review and fix the implementation");
    assert!(stored_task_1.dependencies.is_empty());
    assert!(matches!(stored_task_1.status, TaskStatus::Pending));
}

#[tokio::test]
async fn recover_node_can_resume_paused_graph_when_requested() {
    let pool = init_test_pool().await;
    let coordinator = build_test_coordinator(pool.clone(), &["recovered via recover"]);

    let mut graph = TaskGraph::new().with_goal("recover and resume".to_string());
    graph.add_task(
        "task-1".to_string(),
        "coder".to_string(),
        "debug rust code".to_string(),
        vec![],
    );
    {
        let task = graph.nodes.get_mut("task-1").unwrap();
        task.status = TaskStatus::Cancelled {
            cancelled_at: 1,
            reason: "operator cancellation".to_string(),
        };
    }
    graph.status = GraphStatus::Paused;

    coordinator
        .persister
        .persist_graph("graph-recover-resume", &graph, "recover and resume")
        .await
        .unwrap();
    coordinator
        .active_graphs
        .write()
        .await
        .insert("graph-recover-resume".to_string(), graph);

    let started = coordinator
        .recover_node(
            "graph-recover-resume",
            "task-1",
            NodeRecoveryOptions {
                resume_graph: true,
                ..NodeRecoveryOptions::default()
            },
        )
        .await
        .unwrap();
    assert!(started);

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let graph = coordinator
            .active_graphs
            .read()
            .await
            .get("graph-recover-resume")
            .cloned()
            .unwrap();

        if matches!(graph.status, GraphStatus::Completed) {
            let task = graph.nodes.get("task-1").unwrap();
            assert_eq!(task.result.as_deref(), Some("recovered via recover"));
            break;
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for recover_node execution: status={:?}, node={:?}",
            graph.status,
            graph.nodes.get("task-1")
        );
        sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn retry_node_resets_descendants_and_persists_when_paused() {
    let pool = init_test_pool().await;
    let coordinator = build_test_coordinator(pool.clone(), &[]);

    let mut graph = TaskGraph::new().with_goal("ship feature".to_string());
    graph.add_task(
        "task-1".to_string(),
        "coder".to_string(),
        "rebuild the module".to_string(),
        vec![],
    );
    graph.add_task(
        "task-2".to_string(),
        "coder".to_string(),
        "stabilize the rebuilt module".to_string(),
        vec!["task-1".to_string()],
    );

    {
        let task_1 = graph.nodes.get_mut("task-1").unwrap();
        task_1.status = TaskStatus::Completed { duration: 10 };
        task_1.result = Some("old task 1".to_string());
    }
    {
        let task_2 = graph.nodes.get_mut("task-2").unwrap();
        task_2.status = TaskStatus::Completed { duration: 10 };
        task_2.result = Some("old task 2".to_string());
    }
    graph.status = GraphStatus::Paused;

    coordinator
        .persister
        .persist_graph("graph-retry", &graph, "ship feature")
        .await
        .unwrap();
    coordinator
        .active_graphs
        .write()
        .await
        .insert("graph-retry".to_string(), graph);

    let spawned = coordinator
        .retry_node("graph-retry", "task-1")
        .await
        .unwrap();
    assert!(!spawned);

    let graph = coordinator
        .active_graphs
        .read()
        .await
        .get("graph-retry")
        .cloned()
        .unwrap();
    let task_1 = graph.nodes.get("task-1").unwrap();
    let task_2 = graph.nodes.get("task-2").unwrap();

    assert!(matches!(task_1.status, TaskStatus::Pending));
    assert!(matches!(task_2.status, TaskStatus::Pending));
    assert!(task_1.result.is_none());
    assert!(task_2.result.is_none());
    assert!(task_2
        .logs
        .iter()
        .any(|log| log.contains("upstream task task-1")));

    let stored = coordinator.persister.load_active_graphs().await.unwrap();
    let stored_graph = stored.get("graph-retry").unwrap();
    let stored_task_1 = stored_graph.nodes.get("task-1").unwrap();
    let stored_task_2 = stored_graph.nodes.get("task-2").unwrap();

    assert!(matches!(stored_task_1.status, TaskStatus::Pending));
    assert!(matches!(stored_task_2.status, TaskStatus::Pending));
    assert!(stored_task_1.result.is_none());
    assert!(stored_task_2.result.is_none());
}

#[tokio::test]
async fn retry_node_respawns_completed_graph() {
    let pool = init_test_pool().await;
    let coordinator = build_test_coordinator(pool.clone(), &["resumed result"]);

    let mut graph = TaskGraph::new().with_goal("resume swarm".to_string());
    graph.add_task(
        "task-1".to_string(),
        "coder".to_string(),
        "debug rust code".to_string(),
        vec![],
    );
    {
        let task_1 = graph.nodes.get_mut("task-1").unwrap();
        task_1.status = TaskStatus::Completed { duration: 10 };
        task_1.result = Some("old task 1".to_string());
    }
    graph.status = GraphStatus::Completed;

    coordinator
        .persister
        .persist_graph("graph-rerun", &graph, "resume swarm")
        .await
        .unwrap();
    coordinator
        .active_graphs
        .write()
        .await
        .insert("graph-rerun".to_string(), graph);

    let spawned = coordinator
        .retry_node("graph-rerun", "task-1")
        .await
        .unwrap();
    assert!(spawned);

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let graph = coordinator
            .active_graphs
            .read()
            .await
            .get("graph-rerun")
            .cloned()
            .unwrap();

        if matches!(graph.status, GraphStatus::Completed) {
            let task_1 = graph.nodes.get("task-1").unwrap();
            assert_eq!(task_1.result.as_deref(), Some("resumed result"));
            break;
        }

        assert!(
            Instant::now() < deadline,
            "timed out waiting for retry execution: status={:?}, node={:?}",
            graph.status,
            graph.nodes.get("task-1")
        );
        sleep(Duration::from_millis(20)).await;
    }

    let stored_result: Option<String> =
        sqlx::query_scalar("SELECT result FROM swarm_tasks WHERE id = ?")
            .bind("task-1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored_result.as_deref(), Some("resumed result"));
}

#[tokio::test]
async fn update_node_persists_when_graph_is_paused() {
    let pool = init_test_pool().await;
    let coordinator = build_test_coordinator(pool.clone(), &[]);

    let mut graph = TaskGraph::new().with_goal("refine workflow".to_string());
    graph.add_task(
        "task-1".to_string(),
        "planner".to_string(),
        "plan the work".to_string(),
        vec![],
    );
    graph.add_task(
        "task-2".to_string(),
        "coder".to_string(),
        "old prompt".to_string(),
        vec!["task-1".to_string()],
    );
    graph.status = GraphStatus::Paused;

    coordinator
        .persister
        .persist_graph("graph-update", &graph, "refine workflow")
        .await
        .unwrap();
    coordinator
        .active_graphs
        .write()
        .await
        .insert("graph-update".to_string(), graph);

    coordinator
        .update_node(
            "graph-update",
            "task-2",
            "new prompt".to_string(),
            Some(vec!["task-1".to_string()]),
        )
        .await
        .unwrap();

    let graph = coordinator
        .active_graphs
        .read()
        .await
        .get("graph-update")
        .cloned()
        .unwrap();
    let task = graph.nodes.get("task-2").unwrap();
    assert_eq!(task.prompt, "new prompt");
    assert_eq!(task.dependencies, vec!["task-1".to_string()]);

    let stored_prompt: String = sqlx::query_scalar("SELECT prompt FROM swarm_tasks WHERE id = ?")
        .bind("task-2")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored_prompt, "new prompt");
}

#[tokio::test]
async fn delete_graph_removes_memory_and_persistence() {
    let pool = init_test_pool().await;
    let coordinator = build_test_coordinator(pool.clone(), &[]);

    let mut graph = TaskGraph::new().with_goal("delete me".to_string());
    graph.add_task(
        "task-1".to_string(),
        "coder".to_string(),
        "remove this graph".to_string(),
        vec![],
    );

    coordinator
        .persister
        .persist_graph("graph-delete", &graph, "delete me")
        .await
        .unwrap();
    coordinator
        .active_graphs
        .write()
        .await
        .insert("graph-delete".to_string(), graph);
    coordinator
        .running_graphs
        .insert("graph-delete".to_string(), ());

    coordinator.delete_graph("graph-delete").await.unwrap();

    assert!(!coordinator
        .active_graphs
        .read()
        .await
        .contains_key("graph-delete"));
    assert!(!coordinator.running_graphs.contains_key("graph-delete"));

    let graph_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_graphs WHERE id = ?")
        .bind("graph-delete")
        .fetch_one(&pool)
        .await
        .unwrap();
    let task_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM swarm_tasks WHERE graph_id = ?")
        .bind("graph-delete")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(graph_count, 0);
    assert_eq!(task_count, 0);
}
