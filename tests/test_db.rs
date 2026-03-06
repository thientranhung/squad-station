mod helpers;

use squad_station::db::{agents, messages};

// ============================================================
// Agent CRUD tests
// ============================================================

#[tokio::test]
async fn test_insert_and_get_agent() {
    let pool = helpers::setup_test_db().await;

    agents::insert_agent(&pool, "frontend", "claude-code", "worker", "claude --dangerously-skip-permissions")
        .await
        .unwrap();

    let agent = agents::get_agent(&pool, "frontend").await.unwrap();
    assert!(agent.is_some(), "agent should be present after insert");
    let agent = agent.unwrap();
    assert_eq!(agent.name, "frontend");
    assert_eq!(agent.provider, "claude-code");
    assert_eq!(agent.role, "worker");
    assert_eq!(agent.command, "claude --dangerously-skip-permissions");
    assert!(!agent.id.is_empty(), "id should be a non-empty UUID");
}

#[tokio::test]
async fn test_insert_agent_idempotent() {
    // SESS-02: INSERT OR IGNORE — duplicate insert is a no-op, not an error
    let pool = helpers::setup_test_db().await;

    agents::insert_agent(&pool, "backend", "claude-code", "worker", "claude --dangerously-skip-permissions")
        .await
        .expect("first insert should succeed");

    // Second insert must NOT return an error
    agents::insert_agent(&pool, "backend", "gemini", "orchestrator", "gemini --different-command")
        .await
        .expect("duplicate insert should be a no-op, not an error");

    // The first insertion's data must be preserved
    let agent = agents::get_agent(&pool, "backend").await.unwrap().unwrap();
    assert_eq!(agent.provider, "claude-code", "first insert data must be preserved (IGNORE semantics)");
    assert_eq!(agent.role, "worker");
}

#[tokio::test]
async fn test_list_agents() {
    let pool = helpers::setup_test_db().await;

    agents::insert_agent(&pool, "charlie", "claude-code", "worker", "cmd1").await.unwrap();
    agents::insert_agent(&pool, "alpha", "gemini", "worker", "cmd2").await.unwrap();
    agents::insert_agent(&pool, "bravo", "claude-code", "orchestrator", "cmd3").await.unwrap();

    let list = agents::list_agents(&pool).await.unwrap();
    assert_eq!(list.len(), 3, "all 3 agents should be listed");
    // Ordered by name
    assert_eq!(list[0].name, "alpha");
    assert_eq!(list[1].name, "bravo");
    assert_eq!(list[2].name, "charlie");
}

#[tokio::test]
async fn test_get_nonexistent_agent() {
    let pool = helpers::setup_test_db().await;
    let result = agents::get_agent(&pool, "ghost").await.unwrap();
    assert!(result.is_none(), "unknown agent should return None, not error");
}

// ============================================================
// Message CRUD tests — MSG-01
// ============================================================

#[tokio::test]
async fn test_insert_message() {
    // MSG-01: insert_message returns a valid UUID string
    let pool = helpers::setup_test_db().await;

    // Insert agent first (FK constraint)
    agents::insert_agent(&pool, "agent-a", "claude-code", "worker", "cmd").await.unwrap();

    let id = messages::insert_message(&pool, "agent-a", "do the thing", "normal")
        .await
        .unwrap();

    assert!(!id.is_empty(), "returned id should be a non-empty UUID string");
    // UUID v4 is 36 chars: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
    assert_eq!(id.len(), 36, "id should be a standard UUID (36 chars)");
}

#[tokio::test]
async fn test_insert_message_with_priority() {
    let pool = helpers::setup_test_db().await;
    agents::insert_agent(&pool, "agent-b", "claude-code", "worker", "cmd").await.unwrap();

    let id = messages::insert_message(&pool, "agent-b", "urgent task", "high")
        .await
        .unwrap();

    let msgs = messages::list_messages(&pool, Some("agent-b"), None, 10).await.unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].id, id);
    assert_eq!(msgs[0].priority, "high");
    assert_eq!(msgs[0].status, "pending");
}

// ============================================================
// Update status tests — MSG-02, MSG-03
// ============================================================

#[tokio::test]
async fn test_update_status_completes_message() {
    // MSG-02: update_status marks most-recent pending message as completed
    let pool = helpers::setup_test_db().await;
    agents::insert_agent(&pool, "agent-c", "claude-code", "worker", "cmd").await.unwrap();

    messages::insert_message(&pool, "agent-c", "task one", "normal").await.unwrap();

    let rows = messages::update_status(&pool, "agent-c").await.unwrap();
    assert_eq!(rows, 1, "exactly one row should be updated");

    let msgs = messages::list_messages(&pool, Some("agent-c"), Some("completed"), 10).await.unwrap();
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].status, "completed");
}

#[tokio::test]
async fn test_update_status_idempotent() {
    // MSG-03: calling update_status twice returns 0 rows on the second call — no error
    let pool = helpers::setup_test_db().await;
    agents::insert_agent(&pool, "agent-d", "claude-code", "worker", "cmd").await.unwrap();

    messages::insert_message(&pool, "agent-d", "task one", "normal").await.unwrap();

    let first = messages::update_status(&pool, "agent-d").await.unwrap();
    assert_eq!(first, 1);

    // Second call: no pending messages → 0 rows affected, NOT an error
    let second = messages::update_status(&pool, "agent-d").await.unwrap();
    assert_eq!(second, 0, "second update_status call must return 0 rows (idempotent, MSG-03)");
}

#[tokio::test]
async fn test_update_status_no_pending() {
    // MSG-03: update_status with no pending messages returns 0, not an error
    let pool = helpers::setup_test_db().await;
    agents::insert_agent(&pool, "agent-e", "claude-code", "worker", "cmd").await.unwrap();

    let rows = messages::update_status(&pool, "agent-e").await.unwrap();
    assert_eq!(rows, 0, "0 rows affected when no pending messages (MSG-03)");
}

// ============================================================
// List filter tests — MSG-04
// ============================================================

#[tokio::test]
async fn test_list_filter_by_agent() {
    // MSG-04: list filtered by agent_name
    let pool = helpers::setup_test_db().await;
    agents::insert_agent(&pool, "alpha", "claude-code", "worker", "cmd").await.unwrap();
    agents::insert_agent(&pool, "beta", "claude-code", "worker", "cmd").await.unwrap();

    messages::insert_message(&pool, "alpha", "task for alpha", "normal").await.unwrap();
    messages::insert_message(&pool, "beta", "task for beta", "normal").await.unwrap();
    messages::insert_message(&pool, "alpha", "task for alpha 2", "high").await.unwrap();

    let result = messages::list_messages(&pool, Some("alpha"), None, 100).await.unwrap();
    assert_eq!(result.len(), 2, "only alpha's messages should be returned");
    for msg in &result {
        assert_eq!(msg.agent_name, "alpha");
    }
}

#[tokio::test]
async fn test_list_filter_by_status() {
    // MSG-04: list filtered by status
    let pool = helpers::setup_test_db().await;
    agents::insert_agent(&pool, "agent-f", "claude-code", "worker", "cmd").await.unwrap();

    messages::insert_message(&pool, "agent-f", "pending task 1", "normal").await.unwrap();
    messages::insert_message(&pool, "agent-f", "pending task 2", "high").await.unwrap();
    // Complete one
    messages::update_status(&pool, "agent-f").await.unwrap();

    let pending = messages::list_messages(&pool, Some("agent-f"), Some("pending"), 100).await.unwrap();
    assert_eq!(pending.len(), 1, "only one pending message should remain");
    assert_eq!(pending[0].status, "pending");

    let completed = messages::list_messages(&pool, Some("agent-f"), Some("completed"), 100).await.unwrap();
    assert_eq!(completed.len(), 1, "one message should be completed");
    assert_eq!(completed[0].status, "completed");
}

#[tokio::test]
async fn test_list_with_limit() {
    let pool = helpers::setup_test_db().await;
    agents::insert_agent(&pool, "agent-g", "claude-code", "worker", "cmd").await.unwrap();

    for i in 0..10 {
        messages::insert_message(&pool, "agent-g", &format!("task {}", i), "normal")
            .await
            .unwrap();
    }

    let result = messages::list_messages(&pool, None, None, 3).await.unwrap();
    assert_eq!(result.len(), 3, "limit=3 must return exactly 3 messages");
}

#[tokio::test]
async fn test_list_no_filters() {
    let pool = helpers::setup_test_db().await;
    agents::insert_agent(&pool, "alpha2", "claude-code", "worker", "cmd").await.unwrap();
    agents::insert_agent(&pool, "beta2", "claude-code", "worker", "cmd").await.unwrap();

    messages::insert_message(&pool, "alpha2", "task 1", "normal").await.unwrap();
    messages::insert_message(&pool, "beta2", "task 2", "high").await.unwrap();
    messages::insert_message(&pool, "alpha2", "task 3", "urgent").await.unwrap();

    let result = messages::list_messages(&pool, None, None, 100).await.unwrap();
    assert_eq!(result.len(), 3, "all 3 messages returned when no filters applied");
}

// ============================================================
// Peek tests — MSG-05, MSG-06
// ============================================================

#[tokio::test]
async fn test_peek_returns_pending() {
    // MSG-06: peek returns only the pending message
    let pool = helpers::setup_test_db().await;
    agents::insert_agent(&pool, "agent-h", "claude-code", "worker", "cmd").await.unwrap();

    messages::insert_message(&pool, "agent-h", "pending task", "normal").await.unwrap();
    messages::insert_message(&pool, "agent-h", "completed task", "normal").await.unwrap();

    // Complete the most-recent one (completed task)
    messages::update_status(&pool, "agent-h").await.unwrap();

    let peeked = messages::peek_message(&pool, "agent-h").await.unwrap();
    assert!(peeked.is_some(), "should return the pending message");
    assert_eq!(peeked.unwrap().status, "pending");
}

#[tokio::test]
async fn test_peek_priority_ordering() {
    // MSG-05: urgent > high > normal; oldest-first tie-breaking
    let pool = helpers::setup_test_db().await;
    agents::insert_agent(&pool, "agent-i", "claude-code", "worker", "cmd").await.unwrap();

    // Insert in order: normal, high, urgent
    messages::insert_message(&pool, "agent-i", "normal task", "normal").await.unwrap();
    messages::insert_message(&pool, "agent-i", "high task", "high").await.unwrap();
    messages::insert_message(&pool, "agent-i", "urgent task", "urgent").await.unwrap();

    let peeked = messages::peek_message(&pool, "agent-i").await.unwrap();
    assert!(peeked.is_some(), "should return a message");
    let msg = peeked.unwrap();
    assert_eq!(msg.priority, "urgent", "MSG-05: urgent priority must be returned first");
    assert_eq!(msg.task, "urgent task");
}

#[tokio::test]
async fn test_peek_no_pending() {
    // MSG-06: peek returns None when no pending messages exist — not an error
    let pool = helpers::setup_test_db().await;
    agents::insert_agent(&pool, "agent-j", "claude-code", "worker", "cmd").await.unwrap();

    messages::insert_message(&pool, "agent-j", "a task", "normal").await.unwrap();
    messages::update_status(&pool, "agent-j").await.unwrap(); // complete it

    let result = messages::peek_message(&pool, "agent-j").await.unwrap();
    assert!(result.is_none(), "no pending messages → peek returns None (not error)");
}

#[tokio::test]
async fn test_peek_nonexistent_agent() {
    // MSG-06: peek for unknown agent returns None — not an error
    let pool = helpers::setup_test_db().await;

    let result = messages::peek_message(&pool, "ghost-agent").await.unwrap();
    assert!(result.is_none(), "unknown agent → peek returns None (not error)");
}
