use crate::{commands::helpers, config, db};
use anyhow::{bail, Result};

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
            let child = helpers::spawn_watchdog_daemon(&cwd, interval_secs, _stall_threshold_mins)?;
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

    log_watch(
        &squad_dir,
        "INFO",
        &format!("watchdog started interval={}s", interval_secs),
    );

    let is_running = || !SHUTDOWN.load(std::sync::atomic::Ordering::Relaxed);

    while is_running() {
        if let Err(e) = tick(&db_path, &squad_dir).await {
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

/// Health check tick: verify tmux sessions are alive, mark dead if missing.
async fn tick(db_path: &std::path::Path, _squad_dir: &std::path::Path) -> Result<()> {
    let pool = db::connect(db_path).await?;

    // Check tmux session liveness for all agents.
    // Marks agents as "dead" if session is gone, revives to "idle" if session reappears.
    helpers::reconcile_agent_statuses(&pool).await?;

    pool.close().await;
    Ok(())
}

fn log_watch(squad_dir: &std::path::Path, level: &str, msg: &str) {
    helpers::log_to_squad(
        squad_dir,
        "watch.log",
        &format!("{:<9} {}", level, msg),
        false,
    );
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
}
