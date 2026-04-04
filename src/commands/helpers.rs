use crate::{db, tmux};
use owo_colors::OwoColorize;
use owo_colors::Stream;
use sqlx::SqlitePool;
#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Reconcile agent statuses against live tmux sessions.
/// Marks agents as "dead" if their session is gone, or revives to "idle" if session reappears.
pub async fn reconcile_agent_statuses(pool: &SqlitePool) -> anyhow::Result<()> {
    let agents = db::agents::list_agents(pool).await?;
    for agent in &agents {
        // Don't override frozen status — user is in control
        if agent.status == "frozen" {
            continue;
        }
        let session_alive = tmux::session_exists(&agent.name);
        if !session_alive && agent.status != "dead" {
            db::agents::update_agent_status(pool, &agent.name, "dead").await?;
        } else if session_alive && agent.status == "dead" {
            db::agents::update_agent_status(pool, &agent.name, "idle").await?;
        }
    }
    Ok(())
}

/// Format status with human-readable duration since last status change.
pub fn format_status_with_duration(status: &str, status_updated_at: &str) -> String {
    let since = chrono::DateTime::parse_from_rfc3339(status_updated_at)
        .ok()
        .map(|t| {
            let dur = chrono::Utc::now().signed_duration_since(t);
            let mins = dur.num_minutes();
            if mins < 60 {
                format!("{}m", mins)
            } else {
                format!("{}h{}m", mins / 60, mins % 60)
            }
        })
        .unwrap_or_else(|| "?".to_string());
    format!("{} {}", status, since)
}

/// Colorize the status word (not the full status+duration string).
pub fn colorize_agent_status(status: &str) -> String {
    match status {
        "idle" => format!(
            "{}",
            status.if_supports_color(Stream::Stdout, |s| s.green())
        ),
        "busy" => format!(
            "{}",
            status.if_supports_color(Stream::Stdout, |s| s.yellow())
        ),
        "dead" => format!("{}", status.if_supports_color(Stream::Stdout, |s| s.red())),
        "frozen" => format!("{}", status.if_supports_color(Stream::Stdout, |s| s.blue())),
        _ => status.to_string(),
    }
}

/// Best-effort structured log to `.squad/log/<filename>`.
/// Silently ignores failures — must never cause calling command to fail.
/// Includes optional log rotation: truncates to last 500 lines when file exceeds 1 MB.
pub fn log_to_squad(squad_dir: &std::path::Path, filename: &str, line: &str, rotate: bool) {
    let log_dir = squad_dir.join("log");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_file = log_dir.join(filename);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
    {
        use std::io::Write;
        let _ = writeln!(f, "{} {}", chrono::Utc::now().to_rfc3339(), line);
    }
    if rotate {
        if let Ok(meta) = std::fs::metadata(&log_file) {
            if meta.len() > 1_048_576 {
                rotate_log(&log_file);
            }
        }
    }
}

/// Truncate log file to last 500 lines.
pub(crate) fn rotate_log(path: &std::path::Path) {
    if let Ok(content) = std::fs::read_to_string(path) {
        let lines: Vec<&str> = content.lines().collect();
        if lines.len() > 500 {
            let tail = &lines[lines.len() - 500..];
            let _ = std::fs::write(path, tail.join("\n") + "\n");
        }
    }
}

/// Build a padded cell where visible width is based on `raw` length but output uses `colored`.
pub fn pad_colored(raw: &str, colored: &str, width: usize) -> String {
    let raw_len = raw.len();
    let padding = width.saturating_sub(raw_len);
    format!("{}{}", colored, " ".repeat(padding))
}

/// Spawn a watchdog daemon as a detached background process.
/// Configures setsid, stderr-to-log, and stdin/stdout null.
/// Returns the spawned child on success.
///
/// Shared by `watch --daemon` (initial launch) and `ensure_watchdog` (respawn).
#[cfg(unix)]
pub fn spawn_watchdog_daemon(
    project_root: &std::path::Path,
    interval_secs: u64,
    stall_threshold_mins: u64,
) -> std::io::Result<std::process::Child> {
    let squad_dir = project_root.join(".squad");
    let exe = std::env::current_exe()?;

    let log_dir = squad_dir.join("log");
    let _ = std::fs::create_dir_all(&log_dir);
    let stderr_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join("watch-stderr.log"));

    let mut cmd = std::process::Command::new(exe);
    cmd.arg("watch")
        .arg("--interval")
        .arg(interval_secs.to_string())
        .arg("--stall-threshold")
        .arg(stall_threshold_mins.to_string());
    cmd.current_dir(project_root);
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

    // Create new session so SIGHUP from closing the terminal
    // doesn't propagate to the watchdog daemon.
    // SAFETY: setsid() creates a new session for the child process. It can return -1 if the
    // process is already a session leader, but that's benign here — the child is freshly forked
    // by Command::spawn() and won't be a session leader. Even if it were, the only consequence
    // is that the existing session is kept, which still provides adequate isolation.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }

    cmd.spawn()
}

/// Best-effort watchdog health check. If PID file exists but process is dead,
/// attempt to respawn the daemon. Never fails — watchdog is advisory, not critical path.
/// Called opportunistically from signal.rs and send.rs on every successful operation.
///
/// **Limitation:** Respawn uses hardcoded defaults (interval=30s, stall_threshold=5min).
/// If the user originally started the watchdog with custom values via `watch --interval`
/// or `watch --stall-threshold`, the respawned daemon won't honor those. This is acceptable
/// because ensure_watchdog is a best-effort recovery, not a perfect restart — having a
/// watchdog with defaults is better than no watchdog at all.
pub fn ensure_watchdog(project_root: &std::path::Path) {
    let squad_dir = project_root.join(".squad");
    let pid_file = squad_dir.join("watch.pid");
    if !pid_file.exists() {
        return; // No watchdog was ever started — don't auto-create
    }

    let pid: i32 = match std::fs::read_to_string(&pid_file)
        .ok()
        .and_then(|c| c.trim().parse().ok())
    {
        Some(p) => p,
        None => return,
    };

    #[cfg(unix)]
    {
        // SAFETY: signal 0 is a null signal (no-op probe); pid comes from our own PID file.
        let alive = unsafe { libc::kill(pid, 0) == 0 };
        if alive {
            return; // Watchdog is running
        }

        // Dead watchdog — attempt respawn
        let _ = std::fs::remove_file(&pid_file);

        if let Ok(child) = spawn_watchdog_daemon(project_root, 30, 5) {
            let _ = std::fs::write(&pid_file, child.id().to_string());
            log_to_squad(
                &squad_dir,
                "watch.log",
                &format!(
                    "{:<9} watchdog respawned pid={} by=ensure_watchdog",
                    "INFO",
                    child.id()
                ),
                false,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_status_with_duration_minutes() {
        let now = chrono::Utc::now().to_rfc3339();
        let result = format_status_with_duration("idle", &now);
        assert!(result.starts_with("idle "));
        assert!(result.contains("0m") || result.contains("1m"));
    }

    #[test]
    fn test_format_status_with_duration_hours() {
        let ts = (chrono::Utc::now() - chrono::Duration::minutes(90)).to_rfc3339();
        let result = format_status_with_duration("busy", &ts);
        assert!(result.starts_with("busy "));
        assert!(result.contains("1h30m"), "got: {}", result);
    }

    #[test]
    fn test_format_status_with_duration_hours_format_125m() {
        let ts = (chrono::Utc::now() - chrono::Duration::minutes(125)).to_rfc3339();
        let result = format_status_with_duration("busy", &ts);
        assert!(result.contains("2h5m"), "got: {}", result);
    }

    #[test]
    fn test_format_status_with_duration_invalid_timestamp() {
        let result = format_status_with_duration("dead", "not-a-timestamp");
        assert_eq!(result, "dead ?");
    }

    #[test]
    fn test_colorize_agent_status_all_variants() {
        for status in &["idle", "busy", "dead", "custom"] {
            let result = colorize_agent_status(status);
            assert!(result.contains(status));
        }
    }

    #[test]
    fn test_pad_colored_width() {
        let result = pad_colored("idle 5m", "idle 5m", 20);
        assert_eq!(result.len(), 20);
        assert!(result.starts_with("idle 5m"));
    }
}
