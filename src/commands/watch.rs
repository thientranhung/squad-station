use anyhow::{bail, Result};
#[cfg(unix)]
use std::os::unix::process::CommandExt;

use crate::{commands::reconcile, config, db, tmux};

/// Alert state for Pass 3 prolonged busy detection.
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

/// Nudge state for global stall detection (Pass 2).
/// Tracks nudge count, cooldown, and escalation.
struct NudgeState {
    count: u32,
    last_nudge_at: Option<chrono::DateTime<chrono::Utc>>,
    cooldown_secs: u64,
    max_nudges: u32,
}

impl NudgeState {
    fn new(cooldown_secs: u64, max_nudges: u32) -> Self {
        Self {
            count: 0,
            last_nudge_at: None,
            cooldown_secs,
            max_nudges,
        }
    }

    fn should_nudge(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        if self.count >= self.max_nudges {
            return false;
        }
        match self.last_nudge_at {
            None => true,
            Some(last) => (now - last).num_seconds() > self.cooldown_secs as i64,
        }
    }

    fn record_nudge(&mut self, now: chrono::DateTime<chrono::Utc>) {
        self.count += 1;
        self.last_nudge_at = Some(now);
    }

    fn reset(&mut self) {
        self.count = 0;
        self.last_nudge_at = None;
    }
}

pub async fn run(
    interval_secs: u64,
    stall_threshold_mins: u64,
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
        // Stale PID file — remove it
        let _ = std::fs::remove_file(&pid_file);
    }

    // --daemon: fork to background
    if daemon {
        #[cfg(unix)]
        {
            use std::process::Command;
            let exe = std::env::current_exe()?;
            let mut cmd = Command::new(exe);
            cmd.arg("watch")
                .arg("--interval")
                .arg(interval_secs.to_string())
                .arg("--stall-threshold")
                .arg(stall_threshold_mins.to_string());
            // Explicitly set CWD to ensure the child finds squad.yml
            cmd.current_dir(std::env::current_dir()?);

            // Redirect stderr to log file instead of /dev/null so startup
            // panics and DB errors are captured for diagnostics.
            let log_dir = squad_dir.join("log");
            let _ = std::fs::create_dir_all(&log_dir);
            let stderr_file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_dir.join("watch-stderr.log"));

            cmd.stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null());
            match stderr_file {
                Ok(f) => {
                    cmd.stderr(std::process::Stdio::from(f));
                }
                Err(_) => {
                    cmd.stderr(std::process::Stdio::null());
                }
            }

            // Create new session so SIGHUP from closing the init terminal
            // doesn't propagate to the watchdog daemon.
            unsafe {
                cmd.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }

            let child = cmd.spawn()?;
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

    let mut nudge_state = NudgeState::new(600, 3); // 10min cooldown, 3 max nudges
    let mut last_msg_count: Option<i64> = None;
    let mut busy_alert_state = BusyAlertState::new(600); // 10min cooldown per agent

    log_watch(
        &squad_dir,
        "INFO",
        &format!(
            "watchdog started interval={}s stall_threshold={}m",
            interval_secs, stall_threshold_mins
        ),
    );

    let is_running = || {
        !SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed)
    };

    while is_running() {
        if let Err(e) = tick(
            &db_path,
            &squad_dir,
            stall_threshold_mins,
            &mut nudge_state,
            &mut last_msg_count,
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
    stall_threshold_mins: u64,
    nudge_state: &mut NudgeState,
    last_msg_count: &mut Option<i64>,
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

    // Check for new message activity (resets nudge state)
    let current_count = db::messages::total_count(&pool).await?;
    if let Some(prev) = last_msg_count {
        if current_count != *prev {
            nudge_state.reset();
        }
    }
    *last_msg_count = Some(current_count);

    // Pass 2: Global stall detection
    let agents = db::agents::list_agents(&pool).await?;
    let non_dead: Vec<_> = agents.iter().filter(|a| a.status != "dead").collect();

    if !non_dead.is_empty() {
        let all_idle = non_dead.iter().all(|a| a.status == "idle");
        let processing_count = db::messages::count_processing_all(&pool).await.unwrap_or(0);

        if all_idle && processing_count == 0 {
            // Check how long since last activity
            let last_activity = db::messages::last_activity_timestamp(&pool).await?;

            if let Some(ref ts) = last_activity {
                if let Ok(last_ts) = chrono::DateTime::parse_from_rfc3339(ts) {
                    let idle_duration = chrono::Utc::now().signed_duration_since(last_ts);
                    let idle_mins = idle_duration.num_minutes();

                    if idle_mins >= stall_threshold_mins as i64 {
                        let now = chrono::Utc::now();
                        if nudge_state.should_nudge(now) {
                            // Find orchestrator and nudge
                            if let Ok(Some(orch)) = db::agents::get_orchestrator(&pool).await {
                                if orch.tool != "antigravity" && tmux::session_exists(&orch.name) {
                                    let msg = match nudge_state.count {
                                        0 => format!(
                                            "[SQUAD WATCHDOG] System idle for {}m — all agents idle, no pending tasks. Run: squad-station status",
                                            idle_mins
                                        ),
                                        1 => format!(
                                            "[SQUAD WATCHDOG] System still idle after nudge ({}m). Review agent status and dispatch work.",
                                            idle_mins
                                        ),
                                        _ => format!(
                                            "[SQUAD WATCHDOG] Final nudge — system idle for {}m. Watchdog stopping nudges. Manual review required.",
                                            idle_mins
                                        ),
                                    };
                                    let _ = tmux::send_keys_literal(&orch.name, &msg);
                                    log_watch(
                                        squad_dir,
                                        "NUDGE",
                                        &format!(
                                            "orch={} idle_mins={} nudge_count={}",
                                            orch.name,
                                            idle_mins,
                                            nudge_state.count + 1
                                        ),
                                    );
                                }
                            }
                            nudge_state.record_nudge(now);
                        } else if nudge_state.count >= nudge_state.max_nudges {
                            log_watch(
                                squad_dir,
                                "STALL",
                                &format!("STALL_UNRESOLVED idle_mins={}", idle_mins),
                            );
                        }
                    }
                }
            }
        }
    }

    // Pass 3: Prolonged busy detection with tiered escalation
    // Tier 1 (10-30min): Log only — long tasks are normal
    // Tier 2 (30min+): Reconcile check — if pane looks idle, auto-heal (signal was lost)
    // Tier 3 (60min+): Notify orchestrator — agent may be stuck, human review needed
    for agent in &agents {
        if agent.status != "busy" {
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

        // Tier 2 (30min+): Reconcile check — is the pane actually idle?
        if tmux::session_exists(&agent.name)
            && reconcile::pane_looks_idle(&agent.name, &agent.tool)
        {
            // Pane is idle but DB says busy → signal was lost. Fix it.
            let completed =
                db::messages::complete_all_processing(&pool, &agent.name).await?;
            db::agents::clear_current_task(&pool, &agent.name).await?;
            db::agents::update_agent_status(&pool, &agent.name, "idle").await?;

            log_watch(
                squad_dir,
                "HEAL",
                &format!(
                    "agent={} busy_minutes={} action=auto_reconcile completed={}",
                    agent.name, busy_mins, completed
                ),
            );

            // Notify orchestrator about the self-heal
            if let Ok(Some(orch)) = db::agents::get_orchestrator(&pool).await {
                if orch.tool != "antigravity" && tmux::session_exists(&orch.name) {
                    let msg = format!(
                        "[SQUAD WATCHDOG] Auto-healed agent '{}' — stuck busy for {}m (signal lost). {} task(s) completed. Run: squad-station status",
                        agent.name, busy_mins, completed
                    );
                    let _ = tmux::send_keys_literal(&orch.name, &msg);
                }
            }

            busy_alert_state.clear(&agent.name);
            continue;
        }

        // Tier 3 (60min+): Notify orchestrator that agent may be stuck
        if busy_mins >= 60 {
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
                        let _ = tmux::send_keys_literal(&orch.name, &msg);
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
        } else {
            // 30-60min: pane not idle, just log
            log_watch(
                squad_dir,
                "WARN",
                &format!(
                    "agent={} busy_minutes={} tier=prolonged_busy pane=active",
                    agent.name, busy_mins
                ),
            );
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
    fn test_nudge_state_first_nudge() {
        let state = NudgeState::new(600, 3);
        assert!(state.should_nudge(chrono::Utc::now()));
    }

    #[test]
    fn test_nudge_state_respects_cooldown() {
        let mut state = NudgeState::new(600, 3);
        let now = chrono::Utc::now();
        state.record_nudge(now);

        // Immediately after nudge: should NOT nudge (cooldown not elapsed)
        assert!(!state.should_nudge(now));

        // 5 minutes later: still in cooldown
        let five_mins = now + chrono::Duration::seconds(300);
        assert!(!state.should_nudge(five_mins));

        // 11 minutes later: cooldown elapsed
        let eleven_mins = now + chrono::Duration::seconds(660);
        assert!(state.should_nudge(eleven_mins));
    }

    #[test]
    fn test_nudge_state_max_nudges() {
        let mut state = NudgeState::new(0, 3); // 0 cooldown for testing
        let base = chrono::Utc::now();

        for i in 0..3 {
            // Advance time by 1 second per nudge to satisfy cooldown check
            let t = base + chrono::Duration::seconds(i as i64 + 1);
            assert!(state.should_nudge(t), "nudge {} should be allowed", i + 1);
            state.record_nudge(t);
        }

        // After 3 nudges: stop regardless of time
        let future = base + chrono::Duration::seconds(100);
        assert!(!state.should_nudge(future));
    }

    #[test]
    fn test_nudge_state_reset_on_activity() {
        let mut state = NudgeState::new(0, 3);
        let now = chrono::Utc::now();

        state.record_nudge(now);
        state.record_nudge(now);
        assert_eq!(state.count, 2);

        state.reset();
        assert_eq!(state.count, 0);
        assert!(state.last_nudge_at.is_none());
        assert!(state.should_nudge(now));
    }

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
