use crate::{db, tmux};
use owo_colors::OwoColorize;
use owo_colors::Stream;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use sqlx::SqlitePool;

/// Reconcile agent statuses against live tmux sessions.
/// Marks agents as "dead" if their session is gone, or revives to "idle" if session reappears.
/// Skips db-only agents (e.g. antigravity) that never have tmux sessions.
pub async fn reconcile_agent_statuses(pool: &SqlitePool) -> anyhow::Result<()> {
    let agents = db::agents::list_agents(pool).await?;
    for agent in &agents {
        // Don't override frozen status — user is in control
        if agent.status == "frozen" {
            continue;
        }
        // Skip db-only agents (antigravity) — they never have tmux sessions
        if agent.tool == "antigravity" {
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

/// Build a padded cell where visible width is based on `raw` length but output uses `colored`.
pub fn pad_colored(raw: &str, colored: &str, width: usize) -> String {
    let raw_len = raw.len();
    let padding = width.saturating_sub(raw_len);
    format!("{}{}", colored, " ".repeat(padding))
}

/// Best-effort watchdog health check. If PID file exists but process is dead,
/// attempt to respawn the daemon. Never fails — watchdog is advisory, not critical path.
/// Called opportunistically from signal.rs and send.rs on every successful operation.
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
        let alive = unsafe { libc::kill(pid, 0) == 0 };
        if alive {
            return; // Watchdog is running
        }

        // Dead watchdog — attempt respawn
        let _ = std::fs::remove_file(&pid_file);
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(_) => return,
        };

        let log_dir = squad_dir.join("log");
        let _ = std::fs::create_dir_all(&log_dir);
        let stderr_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_dir.join("watch-stderr.log"));

        let mut cmd = std::process::Command::new(exe);
        cmd.arg("watch")
            .arg("--interval")
            .arg("30")
            .arg("--stall-threshold")
            .arg("5");
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
        // doesn't propagate to the respawned watchdog daemon.
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }

        if let Ok(child) = cmd.spawn() {
            let _ = std::fs::write(&pid_file, child.id().to_string());
            // Best-effort log to watch.log
            let log_file = log_dir.join("watch.log");
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_file)
            {
                use std::io::Write;
                let _ = writeln!(
                    f,
                    "{} {:<9} watchdog respawned pid={} by=ensure_watchdog",
                    chrono::Utc::now().to_rfc3339(),
                    "INFO",
                    child.id()
                );
            }
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
