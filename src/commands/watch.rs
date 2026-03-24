use anyhow::{bail, Result};
use crate::{commands::helpers, commands::reconcile, config, db, tmux};

/// Alert state for prolonged busy detection (Pass 2).
/// Tracks per-agent notification cooldown to avoid spamming the orchestrator.
struct BusyAlertState {
    last_alert_at: std::collections::HashMap<String, chrono::DateTime<chrono::Utc>>,
    cooldown_secs: u64,
}

impl BusyAlertState {
    fn new(cooldown_secs: u64) -> Self {
        Self {
            last_alert_at: std::collections::HashMap::new(),
            cooldown_secs,
        }
    }

    fn should_alert(&self, agent: &str, now: chrono::DateTime<chrono::Utc>) -> bool {
        match self.last_alert_at.get(agent) {
            None => true,
            Some(last) => (now - *last).num_seconds() > self.cooldown_secs as i64,
        }
    }

    fn record_alert(&mut self, agent: &str, now: chrono::DateTime<chrono::Utc>) {
        self.last_alert_at.insert(agent.to_string(), now);
    }

    /// Remove agent from tracking (e.g. after reconcile heals it)
    fn clear(&mut self, agent: &str) {
        self.last_alert_at.remove(agent);
    }
}

pub async fn run(
    interval_secs: u64,
    _stall_threshold_mins: u64,
    daemon: bool,
    stop: bool,
) -> Result<()> {
    let config_path = std::path::Path::new("squad.yml");
    let config = config::load_config(config_path)?;
    let db_path = config::resolve_db_path(&config)?;
    let squad_dir = db_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();

    // --stop: kill running daemon
    if stop {
        let pid_file = squad_dir.join("watch.pid");
        if pid_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&pid_file) {
                if let Ok(pid) = content.trim().parse::<i32>() {
                    #[cfg(unix)]
                    // SAFETY: signal 0 probes liveness without side effects; SIGTERM is
                    // the standard graceful-shutdown signal. pid comes from our own PID file.
                    unsafe {
                        if libc::kill(pid, 0) == 0 {
                            libc::kill(pid, libc::SIGTERM);
                            println!("Stopped watchdog daemon (PID {})", pid);
                        } else {
                            println!("Watchdog daemon not running (stale PID file)");
                        }
                    }
                }
            }
            let _ = std::fs::remove_file(&pid_file);
        } else {
            println!("No watchdog daemon running (no PID file)");
        }
        return Ok(());
    }

    // Check for existing daemon
    let pid_file = squad_dir.join("watch.pid");
    if pid_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&pid_file) {
            if let Ok(pid) = content.trim().parse::<i32>() {
                #[cfg(unix)]
                {
                    let my_pid = std::process::id() as i32;
                    // If the PID file contains our own PID, we are the daemon child
                    // that was just spawned — the parent wrote our PID before we started.
                    // Skip the duplicate check so we can proceed to run.
                    if pid != my_pid {
                        // SAFETY: signal 0 is a null signal (no-op liveness probe); pid from our PID file.
                        let alive = unsafe { libc::kill(pid, 0) == 0 };
                        if alive {
                            bail!(
                                "Watchdog daemon already running (PID {}). Use --stop to kill it first.",
                                pid
                            );
                        }
                    }
                }
            }
        }
        // Stale PID file — remove it (unless it's our own)
        if let Ok(content) = std::fs::read_to_string(&pid_file) {
            if let Ok(pid) = content.trim().parse::<i32>() {
                if pid != std::process::id() as i32 {
                    let _ = std::fs::remove_file(&pid_file);
                }
            }
        }
    }

    // --daemon: fork to background
    if daemon {
        #[cfg(unix)]
        {
            let cwd = std::env::current_dir()?;
            let child =
                helpers::spawn_watchdog_daemon(&cwd, interval_secs, _stall_threshold_mins)?;
            let pid = child.id();
            std::fs::write(&pid_file, pid.to_string())?;
            println!("Watchdog daemon started (PID {})", pid);
            return Ok(());
        }
        #[cfg(not(unix))]
        {
            bail!("--daemon mode is only supported on Unix");
        }
    }

    // Write PID file for foreground mode too (so --stop works)
    std::fs::write(&pid_file, std::process::id().to_string())?;

    // Setup graceful shutdown via SIGTERM/SIGINT
    setup_signal_handlers();

    let mut busy_alert_state = BusyAlertState::new(600); // 10min cooldown per agent

    log_watch(
        &squad_dir,
        "INFO",
        &format!("watchdog started interval={}s", interval_secs),
    );

    let is_running = || {
        !SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed)
    };

    while is_running() {
        if let Err(e) = tick(
            &db_path,
            &squad_dir,
            &mut busy_alert_state,
        )
        .await
        {
            log_watch(&squad_dir, "ERROR", &format!("tick failed: {}", e));
        }

        // Sleep in small increments so we can check the shutdown flag
        for _ in 0..interval_secs {
            if !is_running() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    log_watch(&squad_dir, "INFO", "watchdog stopped");
    let _ = std::fs::remove_file(&pid_file);
    Ok(())
}

/// Global shutdown flag for signal handler.
static SHUTDOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn setup_signal_handlers() {
    #[cfg(unix)]
    // SAFETY: signal_trampoline is an extern "C" fn that only sets an AtomicBool —
    // async-signal-safe. We register it for SIGTERM/SIGINT for graceful shutdown.
    unsafe {
        libc::signal(libc::SIGTERM, signal_trampoline as *const () as usize);
        libc::signal(libc::SIGINT, signal_trampoline as *const () as usize);
    }
}

#[cfg(unix)]
extern "C" fn signal_trampoline(_sig: libc::c_int) {
    SHUTDOWN.store(true, std::sync::atomic::Ordering::Relaxed);
}

async fn tick(
    db_path: &std::path::Path,
    squad_dir: &std::path::Path,
    busy_alert_state: &mut BusyAlertState,
) -> Result<()> {
    let pool = db::connect(db_path).await?;

    // Pass 1: Individual agent reconciliation
    let results = reconcile::reconcile_agents(&pool, false).await?;
    for r in &results {
        if r.action != "skip" {
            log_watch(
                squad_dir,
                "RECONCILE",
                &format!("agent={} action={} reason={}", r.agent, r.action, r.reason),
            );
        }
    }

    // Pass 2: Prolonged busy detection with tiered escalation
    // Pre-check: if DB says "busy" but zero processing messages, agent is orphaned — fix immediately
    // Tier 1 (10-30min): Log only — long tasks are normal
    // Tier 2 (30-60min): Log with pane snapshot for diagnostics
    // Tier 3 (60min+): Notify orchestrator — agent may be stuck, human review needed
    let agents = db::agents::list_agents(&pool).await?;
    for agent in &agents {
        if agent.status != "busy" {
            continue;
        }

        // Pre-check: agent is "busy" in DB but has zero processing messages.
        // This means the signal completed the task but failed to reset agent status
        // (e.g. duplicate signal, race condition, or signal exited before status update).
        // This is a definitive check — no heuristics, no pane inspection needed.
        let processing_count = db::messages::count_processing(&pool, &agent.name).await?;
        if processing_count == 0 {
            db::agents::clear_current_task(&pool, &agent.name).await?;
            db::agents::update_agent_status(&pool, &agent.name, "idle").await?;
            log_watch(
                squad_dir,
                "HEAL",
                &format!(
                    "agent={} action=orphan_reset reason=busy_with_zero_processing_messages",
                    agent.name
                ),
            );
            busy_alert_state.clear(&agent.name);
            continue;
        }

        let ts = match chrono::DateTime::parse_from_rfc3339(&agent.status_updated_at) {
            Ok(ts) => ts,
            Err(_) => continue,
        };
        let busy_mins = chrono::Utc::now()
            .signed_duration_since(ts)
            .num_minutes();

        if busy_mins < 10 {
            continue; // Normal operation
        }

        if busy_mins < 30 {
            // Tier 1: Log only
            log_watch(
                squad_dir,
                "WARN",
                &format!(
                    "agent={} busy_minutes={} tier=log_only",
                    agent.name, busy_mins
                ),
            );
            continue;
        }

        // Tier 2 (30-60min): Log with pane snapshot for diagnostics
        // (Task completion is handled exclusively by the signal hook, not by the watchdog.)
        if busy_mins < 60 {
            // Capture pane snapshot for diagnostics
            if tmux::session_exists(&agent.name) {
                let pane_snapshot = reconcile::capture_pane(&agent.name);
                let snapshot_tail: String = pane_snapshot
                    .lines()
                    .rev()
                    .take(5)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join(" | ");
                log_watch(
                    squad_dir,
                    "WARN",
                    &format!(
                        "agent={} busy_minutes={} tier=prolonged_busy pane_snapshot=\"{}\"",
                        agent.name, busy_mins, snapshot_tail
                    ),
                );
            } else {
                log_watch(
                    squad_dir,
                    "WARN",
                    &format!(
                        "agent={} busy_minutes={} tier=prolonged_busy pane=no_session",
                        agent.name, busy_mins
                    ),
                );
            }
            continue;
        }

        // Tier 3 (60min+): Notify orchestrator that agent may be stuck
        let now = chrono::Utc::now();
        if busy_alert_state.should_alert(&agent.name, now) {
            if let Ok(Some(orch)) = db::agents::get_orchestrator(&pool).await {
                if orch.tool != "antigravity" && tmux::session_exists(&orch.name) {
                    let urgency = if busy_mins >= 120 {
                        "URGENT"
                    } else {
                        "WARNING"
                    };
                    let msg = format!(
                        "[SQUAD WATCHDOG] {} — Agent '{}' busy for {}m, may be stuck. Check: tmux capture-pane -t {} -p | tail -20",
                        urgency, agent.name, busy_mins, agent.name
                    );
                    let _ = tmux::send_keys_literal(&orch.name, &msg).await;
                    log_watch(
                        squad_dir,
                        "ALERT",
                        &format!(
                            "agent={} busy_minutes={} tier=notify_orch",
                            agent.name, busy_mins
                        ),
                    );
                }
            }
            busy_alert_state.record_alert(&agent.name, now);
        }
    }

    pool.close().await;
    Ok(())
}

fn log_watch(squad_dir: &std::path::Path, level: &str, msg: &str) {
    let log_dir = squad_dir.join("log");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_file = log_dir.join("watch.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
    {
        use std::io::Write;
        let _ = writeln!(
            f,
            "{} {:<9} {}",
            chrono::Utc::now().to_rfc3339(),
            level,
            msg
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_watch_creates_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        log_watch(tmp.path(), "INFO", "test message");

        let log_file = tmp.path().join("log").join("watch.log");
        assert!(log_file.exists());
        let content = std::fs::read_to_string(&log_file).unwrap();
        assert!(content.contains("INFO"));
        assert!(content.contains("test message"));
    }

    #[test]
    fn test_busy_alert_state_first_alert() {
        let state = BusyAlertState::new(600);
        assert!(state.should_alert("agent-a", chrono::Utc::now()));
    }

    #[test]
    fn test_busy_alert_state_respects_cooldown() {
        let mut state = BusyAlertState::new(600);
        let now = chrono::Utc::now();
        state.record_alert("agent-a", now);

        // Immediately: should NOT alert
        assert!(!state.should_alert("agent-a", now));

        // 5 minutes later: still in cooldown
        let five_mins = now + chrono::Duration::seconds(300);
        assert!(!state.should_alert("agent-a", five_mins));

        // 11 minutes later: cooldown elapsed
        let eleven_mins = now + chrono::Duration::seconds(660);
        assert!(state.should_alert("agent-a", eleven_mins));
    }

    #[test]
    fn test_busy_alert_state_per_agent() {
        let mut state = BusyAlertState::new(600);
        let now = chrono::Utc::now();
        state.record_alert("agent-a", now);

        // Different agent should still be alertable
        assert!(state.should_alert("agent-b", now));
        // Same agent should be in cooldown
        assert!(!state.should_alert("agent-a", now));
    }

    #[test]
    fn test_busy_alert_state_clear() {
        let mut state = BusyAlertState::new(600);
        let now = chrono::Utc::now();
        state.record_alert("agent-a", now);
        assert!(!state.should_alert("agent-a", now));

        state.clear("agent-a");
        assert!(state.should_alert("agent-a", now));
    }
}
