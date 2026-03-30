use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::{config, db, tmux};

use super::init::{auto_install_hooks_pub, get_launch_command_pub, install_session_start_hook_pub};

// ── Data Structures ──────────────────────────────────────────────────────────

/// A single agent entry derived from squad.yml (session_name + config snapshot).
#[derive(Debug, Clone)]
pub struct YmlAgent {
    pub session_name: String,
    pub provider: String,
    pub role: String,
}

/// A provider change detected between yml and DB.
#[derive(Debug, Clone)]
pub struct ProviderChange {
    pub session_name: String,
    pub old_provider: String,
    pub new_provider: String,
}

/// The result of diffing squad.yml agents against DB agents.
#[derive(Debug)]
pub struct UpdatePlan {
    pub new_agents: Vec<YmlAgent>,
    pub removed_agents: Vec<String>, // session names
    pub provider_changed: Vec<ProviderChange>,
    pub unchanged: Vec<String>, // session names
}

// ── Core diff logic (pure — no I/O) ─────────────────────────────────────────

/// Diff yml agent entries against DB agents.
/// Comparison key: session_name (= sanitized "{project}-{name_or_role}").
/// Provider comparison uses Agent.tool (DB column name for provider).
pub fn classify_changes(
    yml_agents: &[YmlAgent],
    db_agents: &[crate::db::agents::Agent],
) -> UpdatePlan {
    let mut new_agents = Vec::new();
    let mut removed_agents = Vec::new();
    let mut provider_changed = Vec::new();
    let mut unchanged = Vec::new();

    // Index DB agents by name for O(1) lookup
    let db_map: std::collections::HashMap<&str, &crate::db::agents::Agent> =
        db_agents.iter().map(|a| (a.name.as_str(), a)).collect();

    // Index yml agents by session_name for removed detection
    let yml_names: std::collections::HashSet<&str> =
        yml_agents.iter().map(|a| a.session_name.as_str()).collect();

    // Classify each yml agent
    for yml in yml_agents {
        match db_map.get(yml.session_name.as_str()) {
            None => new_agents.push(yml.clone()),
            Some(db_agent) => {
                if db_agent.tool != yml.provider {
                    provider_changed.push(ProviderChange {
                        session_name: yml.session_name.clone(),
                        old_provider: db_agent.tool.clone(),
                        new_provider: yml.provider.clone(),
                    });
                } else {
                    unchanged.push(yml.session_name.clone());
                }
            }
        }
    }

    // Detect removed: in DB but not in yml (skip orchestrator role — handled separately)
    for db_agent in db_agents {
        if db_agent.role != "orchestrator" && !yml_names.contains(db_agent.name.as_str()) {
            removed_agents.push(db_agent.name.clone());
        }
    }

    UpdatePlan {
        new_agents,
        removed_agents,
        provider_changed,
        unchanged,
    }
}

/// Check if an agent has any message currently in 'processing' state.
pub async fn has_processing_message(
    pool: &sqlx::SqlitePool,
    agent_name: &str,
) -> anyhow::Result<bool> {
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM messages WHERE agent_name = ? AND status = 'processing'",
    )
    .bind(agent_name)
    .fetch_one(pool)
    .await?;
    Ok(count.0 > 0)
}

// ── Public run ────────────────────────────────────────────────────────────────

pub async fn run(config_path: PathBuf) -> Result<()> {
    let config = config::load_config(&config_path)?;
    let db_path = config::resolve_db_path(&config)?;

    // DB must already exist — update is not init
    if !db_path.exists() {
        anyhow::bail!(
            "No database found at {}. Run `squad-station init` first.",
            db_path.display()
        );
    }

    let pool = db::connect(&db_path).await?;
    let project_root = db_path
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(Path::new("."))
        .to_path_buf();

    // Build yml agent list (workers only — orchestrator handled separately)
    let yml_agents: Vec<YmlAgent> = config
        .agents
        .iter()
        .map(|a| {
            let role_suffix = a.name.as_deref().unwrap_or(&a.role);
            YmlAgent {
                session_name: config::build_session_name(&config.project, role_suffix),
                provider: a.provider.clone(),
                role: a.role.clone(),
            }
        })
        .collect();

    // Load DB agents (workers only)
    let db_agents: Vec<_> = db::agents::list_agents(&pool)
        .await?
        .into_iter()
        .filter(|a| a.role != "orchestrator")
        .collect();

    let plan = classify_changes(&yml_agents, &db_agents);

    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         Squad Station  •  Update                             ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("  Project  : {}", config.project);
    println!();

    // Nothing changed?
    if plan.new_agents.is_empty()
        && plan.removed_agents.is_empty()
        && plan.provider_changed.is_empty()
    {
        println!("  ✓ No agent changes detected.");
        run_housekeeping(&config, &project_root)?;
        if let Err(e) = super::context::run(false).await {
            eprintln!("  [WARN] Could not regenerate orchestrator context: {e}");
        }
        ensure_monitor(&config, false)?; // no changes — only recreate if dead
        println!();
        return Ok(());
    }

    // ── Check for processing tasks on affected agents ────────────────────────
    let affected: Vec<&str> = plan
        .removed_agents
        .iter()
        .map(|s| s.as_str())
        .chain(
            plan.provider_changed
                .iter()
                .map(|c| c.session_name.as_str()),
        )
        .collect();

    let mut blocked: Vec<String> = Vec::new();
    for name in &affected {
        if has_processing_message(&pool, name).await? {
            blocked.push(name.to_string());
        }
    }

    if !blocked.is_empty() {
        println!("  ⚠️  Cannot update — agents have tasks in progress:");
        for name in &blocked {
            println!("       • {} (status: processing)", name);
        }
        println!();
        println!(
            "  Wait for tasks to complete, or run `squad-station clean` + `init` to force restart."
        );
        println!();
        return Ok(());
    }

    // ── Print plan ───────────────────────────────────────────────────────────
    for a in &plan.new_agents {
        println!("  [NEW]     {} ({})", a.session_name, a.provider);
    }
    for name in &plan.removed_agents {
        println!("  [REMOVED] {} — session will be killed", name);
    }
    for c in &plan.provider_changed {
        println!(
            "  [WARN]    {} provider changed: {} → {} (task history will be preserved in DB but session restarted)",
            c.session_name, c.old_provider, c.new_provider
        );
    }
    for name in &plan.unchanged {
        println!("  [OK]      {} — no change", name);
    }
    println!();

    // ── Execute plan ─────────────────────────────────────────────────────────

    // 1. Kill removed agents
    for name in &plan.removed_agents {
        if tmux::session_exists(name) {
            tmux::kill_session(name)?;
            println!("  [KILLED]  {}", name);
        }
    }

    // 2. Kill + relaunch provider-changed agents
    for change in &plan.provider_changed {
        if tmux::session_exists(&change.session_name) {
            tmux::kill_session(&change.session_name)?;
        }
        // Find matching agent config to get launch command
        if let Some(agent_cfg) = config.agents.iter().find(|a| {
            let role_suffix = a.name.as_deref().unwrap_or(&a.role);
            let sname = config::build_session_name(&config.project, role_suffix);
            sname == change.session_name
        }) {
            let cmd = get_launch_command_pub(agent_cfg);
            tmux::launch_agent_in_dir(&change.session_name, &cmd, &project_root.to_string_lossy())?;
            // Update DB provider
            db::agents::insert_agent(
                &pool,
                &change.session_name,
                &change.new_provider,
                &agent_cfg.role,
                agent_cfg.model.as_deref(),
                agent_cfg.description.as_deref(),
            )
            .await?;
            println!(
                "  [RESTART] {} (provider: {} → {})",
                change.session_name, change.old_provider, change.new_provider
            );
        }
    }

    // 3. Launch new agents
    for yml_agent in &plan.new_agents {
        if let Some(agent_cfg) = config.agents.iter().find(|a| {
            let role_suffix = a.name.as_deref().unwrap_or(&a.role);
            let sname = config::build_session_name(&config.project, role_suffix);
            sname == yml_agent.session_name
        }) {
            db::agents::insert_agent(
                &pool,
                &yml_agent.session_name,
                &yml_agent.provider,
                &yml_agent.role,
                agent_cfg.model.as_deref(),
                agent_cfg.description.as_deref(),
            )
            .await?;
            let cmd = get_launch_command_pub(agent_cfg);
            tmux::launch_agent_in_dir(
                &yml_agent.session_name,
                &cmd,
                &project_root.to_string_lossy(),
            )?;
            println!("  [LAUNCHED] {}", yml_agent.session_name);
        }
    }

    // 4. Housekeeping: hooks
    let killed = run_housekeeping(&config, &project_root)?;
    debug_assert!(
        killed.is_empty(),
        "run_housekeeping must never kill sessions — got: {:?}",
        killed
    );

    // 5. Regenerate squad-orchestrator.md so orchestrator sees the updated agent list
    if let Err(e) = super::context::run(false).await {
        eprintln!("  [WARN] Could not regenerate orchestrator context: {e}");
    }

    // 6. Rebuild monitor to reflect the new agent set
    ensure_monitor(&config, true)?;

    println!();
    println!("  ✓ Update complete.");
    println!();

    Ok(())
}

/// Ensure the monitor session reflects the current agent set.
///
/// - `force = false`: recreate only if dead (no-op when alive)
/// - `force = true`: kill and recreate unconditionally — use after agent changes
///   so new/removed agents are reflected in monitor panes.
fn ensure_monitor(config: &config::SquadConfig, force: bool) -> Result<()> {
    let monitor_name = config::build_session_name(&config.project, "monitor");

    if tmux::session_exists(&monitor_name) {
        if !force {
            return Ok(()); // alive and no changes — nothing to do
        }
        // force=true: kill first so we recreate with updated pane list
        let _ = tmux::kill_session(&monitor_name);
    }

    // Build the ordered list of agent sessions (orchestrator first, then workers)
    let mut sessions: Vec<String> = vec![];
    let orch_suffix = config
        .orchestrator
        .name
        .as_deref()
        .unwrap_or("orchestrator");
    sessions.push(config::build_session_name(&config.project, orch_suffix));
    for agent in &config.agents {
        let role_suffix = agent.name.as_deref().unwrap_or(&agent.role);
        sessions.push(config::build_session_name(&config.project, role_suffix));
    }

    if sessions.is_empty() {
        return Ok(());
    }

    match tmux::create_view_session(&monitor_name, &sessions) {
        Ok(()) => println!("  [MONITOR] {} recreated", monitor_name),
        Err(e) => eprintln!("  [WARN] Could not recreate monitor session: {e}"),
    }

    Ok(())
}

/// Re-run hooks and regenerate context. Always safe to call.
///
/// INVARIANT: must never kill any tmux sessions — agents and monitor
/// sessions are managed exclusively by the explicit plan execution above.
/// Returns the list of sessions killed (MUST always be empty).
pub fn run_housekeeping(config: &config::SquadConfig, project_root: &Path) -> Result<Vec<String>> {
    // Collect all unique providers (orchestrator + workers)
    let mut providers: Vec<&str> = vec![config.orchestrator.provider.as_str()];
    for agent in &config.agents {
        if !providers.contains(&agent.provider.as_str()) {
            providers.push(agent.provider.as_str());
        }
    }

    // Re-install hooks for every provider (idempotent)
    for provider in &providers {
        let _ = auto_install_hooks_pub(provider);
        let _ = install_session_start_hook_pub(provider, project_root);
    }

    // Return empty — housekeeping NEVER kills sessions.
    Ok(vec![])
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::agents::Agent;

    fn make_db_agent(name: &str, tool: &str, role: &str) -> Agent {
        Agent {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            tool: tool.to_string(),
            role: role.to_string(),
            command: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            status: "idle".to_string(),
            status_updated_at: "2026-01-01T00:00:00Z".to_string(),
            model: None,
            description: None,
            current_task: None,
        }
    }

    fn make_yml_agent(session_name: &str, provider: &str, role: &str) -> YmlAgent {
        YmlAgent {
            session_name: session_name.to_string(),
            provider: provider.to_string(),
            role: role.to_string(),
        }
    }

    // ── RED: write tests first ────────────────────────────────────────────

    #[test]
    fn test_classify_no_changes_returns_all_unchanged() {
        let yml = vec![
            make_yml_agent("proj-coder", "claude-code", "worker"),
            make_yml_agent("proj-tester", "claude-code", "worker"),
        ];
        let db = vec![
            make_db_agent("proj-coder", "claude-code", "worker"),
            make_db_agent("proj-tester", "claude-code", "worker"),
        ];

        let plan = classify_changes(&yml, &db);

        assert!(plan.new_agents.is_empty());
        assert!(plan.removed_agents.is_empty());
        assert!(plan.provider_changed.is_empty());
        assert_eq!(plan.unchanged.len(), 2);
        assert!(plan.unchanged.contains(&"proj-coder".to_string()));
        assert!(plan.unchanged.contains(&"proj-tester".to_string()));
    }

    #[test]
    fn test_classify_new_agent_not_in_db() {
        let yml = vec![
            make_yml_agent("proj-coder", "claude-code", "worker"),
            make_yml_agent("proj-new", "gemini-cli", "worker"), // new
        ];
        let db = vec![make_db_agent("proj-coder", "claude-code", "worker")];

        let plan = classify_changes(&yml, &db);

        assert_eq!(plan.new_agents.len(), 1);
        assert_eq!(plan.new_agents[0].session_name, "proj-new");
        assert_eq!(plan.new_agents[0].provider, "gemini-cli");
        assert!(plan.removed_agents.is_empty());
        assert_eq!(plan.unchanged.len(), 1);
    }

    #[test]
    fn test_classify_removed_agent_not_in_yml() {
        let yml = vec![make_yml_agent("proj-coder", "claude-code", "worker")];
        let db = vec![
            make_db_agent("proj-coder", "claude-code", "worker"),
            make_db_agent("proj-old", "claude-code", "worker"), // removed
        ];

        let plan = classify_changes(&yml, &db);

        assert!(plan.new_agents.is_empty());
        assert_eq!(plan.removed_agents.len(), 1);
        assert_eq!(plan.removed_agents[0], "proj-old");
        assert_eq!(plan.unchanged.len(), 1);
    }

    #[test]
    fn test_classify_provider_changed() {
        let yml = vec![make_yml_agent("proj-coder", "gemini-cli", "worker")];
        let db = vec![make_db_agent("proj-coder", "claude-code", "worker")];

        let plan = classify_changes(&yml, &db);

        assert!(plan.new_agents.is_empty());
        assert!(plan.removed_agents.is_empty());
        assert_eq!(plan.provider_changed.len(), 1);
        assert_eq!(plan.provider_changed[0].session_name, "proj-coder");
        assert_eq!(plan.provider_changed[0].old_provider, "claude-code");
        assert_eq!(plan.provider_changed[0].new_provider, "gemini-cli");
        assert!(plan.unchanged.is_empty());
    }

    #[test]
    fn test_classify_mixed_changes() {
        let yml = vec![
            make_yml_agent("proj-coder", "claude-code", "worker"), // unchanged
            make_yml_agent("proj-new", "codex", "worker"),         // new
            make_yml_agent("proj-changed", "gemini-cli", "worker"), // provider changed
        ];
        let db = vec![
            make_db_agent("proj-coder", "claude-code", "worker"),
            make_db_agent("proj-changed", "claude-code", "worker"),
            make_db_agent("proj-removed", "claude-code", "worker"), // removed
        ];

        let plan = classify_changes(&yml, &db);

        assert_eq!(plan.new_agents.len(), 1);
        assert_eq!(plan.new_agents[0].session_name, "proj-new");

        assert_eq!(plan.removed_agents.len(), 1);
        assert_eq!(plan.removed_agents[0], "proj-removed");

        assert_eq!(plan.provider_changed.len(), 1);
        assert_eq!(plan.provider_changed[0].session_name, "proj-changed");

        assert_eq!(plan.unchanged.len(), 1);
        assert_eq!(plan.unchanged[0], "proj-coder");
    }

    #[test]
    fn test_classify_empty_db_all_agents_are_new() {
        let yml = vec![
            make_yml_agent("proj-coder", "claude-code", "worker"),
            make_yml_agent("proj-tester", "codex", "worker"),
        ];
        let db: Vec<Agent> = vec![];

        let plan = classify_changes(&yml, &db);

        assert_eq!(plan.new_agents.len(), 2);
        assert!(plan.removed_agents.is_empty());
        assert!(plan.provider_changed.is_empty());
        assert!(plan.unchanged.is_empty());
    }

    #[test]
    fn test_classify_empty_yml_all_db_agents_removed() {
        let yml: Vec<YmlAgent> = vec![];
        let db = vec![
            make_db_agent("proj-coder", "claude-code", "worker"),
            make_db_agent("proj-tester", "claude-code", "worker"),
        ];

        let plan = classify_changes(&yml, &db);

        assert!(plan.new_agents.is_empty());
        assert_eq!(plan.removed_agents.len(), 2);
        assert!(plan.provider_changed.is_empty());
        assert!(plan.unchanged.is_empty());
    }

    #[test]
    fn test_classify_orchestrator_in_db_not_counted_as_removed() {
        // Orchestrator lives in DB but is managed separately — should not appear in removed
        let yml: Vec<YmlAgent> = vec![]; // no workers in yml
        let db = vec![
            make_db_agent("proj-orchestrator", "claude-code", "orchestrator"), // should be skipped
            make_db_agent("proj-coder", "claude-code", "worker"),
        ];

        let plan = classify_changes(&yml, &db);

        // proj-coder is removed, but proj-orchestrator should NOT be in removed list
        assert_eq!(plan.removed_agents.len(), 1);
        assert_eq!(plan.removed_agents[0], "proj-coder");
        assert!(!plan
            .removed_agents
            .contains(&"proj-orchestrator".to_string()));
    }

    async fn setup_test_db() -> sqlx::SqlitePool {
        use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();
        std::mem::forget(tmp);
        let opts = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
            .unwrap();
        sqlx::migrate!("./src/db/migrations")
            .run(&pool)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn test_has_processing_message_returns_true_when_processing() {
        let pool = setup_test_db().await;
        db::agents::insert_agent(&pool, "test-agent", "claude-code", "worker", None, None)
            .await
            .unwrap();
        db::messages::insert_message(
            &pool,
            "orchestrator",
            "test-agent",
            "task_request",
            "do something",
            "normal",
            None,
        )
        .await
        .unwrap();

        let result = has_processing_message(&pool, "test-agent").await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_has_processing_message_returns_false_when_no_messages() {
        let pool = setup_test_db().await;
        db::agents::insert_agent(&pool, "test-agent", "claude-code", "worker", None, None)
            .await
            .unwrap();

        let result = has_processing_message(&pool, "test-agent").await.unwrap();
        assert!(!result);
    }

    // ── Regression: run_housekeeping must NEVER kill sessions ─────────────
    // Bug v0.7.12: housekeeping killed the monitor session without relaunching it.
    // This test enforces the invariant: run_housekeeping returns an empty killed list.
    // If someone adds kill_session() to run_housekeeping, they MUST add it to the
    // returned Vec — and this test will catch the regression immediately.
    #[test]
    fn test_housekeeping_never_kills_any_sessions() {
        // Build a minimal SquadConfig with a real temp dir as project_root
        let tmp = tempfile::tempdir().unwrap();
        let config = config::SquadConfig {
            project: "test-proj".to_string(),
            sdd: None,
            telegram: None,
            orchestrator: config::AgentConfig {
                name: None,
                provider: "claude-code".to_string(),
                role: "orchestrator".to_string(),
                model: None,
                description: None,
            },
            agents: vec![],
        };

        let killed =
            run_housekeeping(&config, tmp.path()).expect("run_housekeeping should not error");

        assert!(
            killed.is_empty(),
            "REGRESSION: run_housekeeping killed sessions {:?} — it must never kill any sessions",
            killed
        );
    }
}
