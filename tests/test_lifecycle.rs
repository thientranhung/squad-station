mod helpers;

use squad_station::db;

// ============================================================
// Signal guard tests — HOOK-03
// ============================================================

#[test]
fn test_signal_no_tmux_pane_exits_zero() {
    // HOOK-03: Guard 1 — when TMUX_PANE is unset, signal exits 0 silently
    let bin = env!("CARGO_BIN_EXE_squad-station");
    let output = std::process::Command::new(bin)
        .arg("signal")
        .arg("some-agent")
        .env_remove("TMUX_PANE") // Ensure not set
        .output()
        .expect("failed to run binary");
    assert!(
        output.status.success(),
        "signal without TMUX_PANE should exit 0, got: {:?}",
        output.status
    );
    // Guard 1 should produce no stdout output (silent exit)
    assert!(
        output.stdout.is_empty(),
        "signal guard should produce no stdout output"
    );
}

// ============================================================
// Agent status lifecycle tests — SESS-03, SESS-04
// ============================================================

#[tokio::test]
async fn test_update_agent_status_dead_to_idle() {
    // SESS-04: agent can be revived from dead to idle (simulates tmux session reappearing)
    let pool = helpers::setup_test_db().await;
    db::agents::insert_agent(&pool, "agent-1", "claude", "worker", "echo").await.unwrap();
    // Set to dead
    db::agents::update_agent_status(&pool, "agent-1", "dead").await.unwrap();
    let agent = db::agents::get_agent(&pool, "agent-1").await.unwrap().unwrap();
    assert_eq!(agent.status, "dead");
    // Revive to idle (simulates tmux session reappearing)
    db::agents::update_agent_status(&pool, "agent-1", "idle").await.unwrap();
    let agent = db::agents::get_agent(&pool, "agent-1").await.unwrap().unwrap();
    assert_eq!(agent.status, "idle");
}

// ============================================================
// Orchestrator detection tests — HOOK-01
// ============================================================

#[tokio::test]
async fn test_orchestrator_has_orchestrator_role() {
    // HOOK-01: get_orchestrator returns the agent with role = "orchestrator"
    let pool = helpers::setup_test_db().await;
    db::agents::insert_agent(&pool, "orch", "claude", "orchestrator", "echo").await.unwrap();
    let orch = db::agents::get_orchestrator(&pool).await.unwrap().unwrap();
    assert_eq!(orch.role, "orchestrator");
    assert_eq!(orch.name, "orch");
}

#[tokio::test]
async fn test_get_orchestrator_returns_none_when_no_orchestrator() {
    // HOOK-01: get_orchestrator returns None if no orchestrator is registered
    let pool = helpers::setup_test_db().await;
    db::agents::insert_agent(&pool, "worker-1", "claude", "worker", "echo").await.unwrap();
    let result = db::agents::get_orchestrator(&pool).await.unwrap();
    assert!(result.is_none(), "no orchestrator registered → get_orchestrator returns None");
}

// ============================================================
// List agents status tests — SESS-04
// ============================================================

#[tokio::test]
async fn test_list_agents_includes_status() {
    // SESS-04: list_agents returns status for each agent
    let pool = helpers::setup_test_db().await;
    db::agents::insert_agent(&pool, "a1", "claude", "worker", "echo").await.unwrap();
    db::agents::insert_agent(&pool, "a2", "gemini", "worker", "echo").await.unwrap();
    db::agents::update_agent_status(&pool, "a2", "busy").await.unwrap();
    let agents = db::agents::list_agents(&pool).await.unwrap();
    assert_eq!(agents.len(), 2);
    let a1 = agents.iter().find(|a| a.name == "a1").unwrap();
    let a2 = agents.iter().find(|a| a.name == "a2").unwrap();
    assert_eq!(a1.status, "idle");
    assert_eq!(a2.status, "busy");
}
