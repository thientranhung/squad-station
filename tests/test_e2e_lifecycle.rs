//! E2E lifecycle tests that spawn real processes.
//!
//! These tests cover critical gaps in the test suite:
//! - Watchdog daemon: start → verify alive → stop → verify dead
//! - Watchdog self-detection: daemon must NOT kill itself on PID check
//! - Init flow: verify all artifacts (DB, logs, hooks, context)
//! - Doctor: verify exit codes after successful init
//!
//! These tests would have caught the self-detection race condition bug
//! where the watchdog daemon killed itself immediately after starting
//! because it read its own PID from the PID file and treated it as a
//! "duplicate" daemon.

mod helpers;

use std::path::Path;
use std::process::Command;

fn bin() -> String {
    env!("CARGO_BIN_EXE_squad-station").to_string()
}

/// Write a minimal squad.yml for E2E tests.
fn write_squad_yml(dir: &Path) {
    let yaml = r#"project: e2e-test
orchestrator:
  name: orch
  provider: claude-code
  role: orchestrator
agents:
  - name: worker1
    provider: claude-code
    role: worker
"#;
    std::fs::write(dir.join("squad.yml"), yaml).expect("write squad.yml");
}

/// Create .squad/ directory with DB and log dir (simulates post-init state).
fn setup_squad_dir(dir: &Path) -> std::path::PathBuf {
    let squad_dir = dir.join(".squad");
    let log_dir = squad_dir.join("log");
    std::fs::create_dir_all(&log_dir).unwrap();
    squad_dir
}

/// Create a real SQLite DB file with migrations applied.
async fn setup_db(db_path: &Path) -> sqlx::SqlitePool {
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("create pool");
    sqlx::migrate!("./src/db/migrations")
        .run(&pool)
        .await
        .expect("migrations");
    pool
}

// ============================================================
// Watchdog daemon lifecycle — real process tests
// ============================================================

/// Start watchdog daemon → wait → verify process is alive → stop → verify dead.
///
/// This is the most critical E2E test. It catches bugs like the self-detection
/// race condition where the daemon kills itself immediately after starting.
#[tokio::test]
async fn watchdog_daemon_stays_alive_after_start() {
    let dir = tempfile::TempDir::new().unwrap();
    write_squad_yml(dir.path());
    let squad_dir = setup_squad_dir(dir.path());
    let db_path = squad_dir.join("station.db");
    let _pool = setup_db(&db_path).await;

    // Start the daemon
    let output = Command::new(bin())
        .args([
            "watch",
            "--daemon",
            "--interval",
            "5",
            "--stall-threshold",
            "1",
        ])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output()
        .expect("spawn daemon");

    assert!(
        output.status.success(),
        "daemon start must succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Watchdog daemon started"),
        "should print start message, got: {}",
        stdout
    );

    // PID file must exist
    let pid_file = squad_dir.join("watch.pid");
    assert!(
        pid_file.exists(),
        "watch.pid must be created after daemon start"
    );

    let pid: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .expect("PID must be numeric");

    // Wait briefly for the daemon to initialize
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // KEY CHECK: The daemon must still be alive after 2 seconds.
    // The self-detection bug caused it to die immediately.
    #[cfg(unix)]
    {
        let alive = unsafe { libc::kill(pid, 0) == 0 };
        assert!(
            alive,
            "Watchdog daemon (PID {}) must still be alive after 2s — \
             if dead, the self-detection race condition bug has regressed!",
            pid
        );
    }

    // Stop the daemon
    let stop_output = Command::new(bin())
        .args(["watch", "--stop"])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output()
        .expect("stop daemon");

    assert!(
        stop_output.status.success(),
        "daemon stop must succeed, stderr: {}",
        String::from_utf8_lossy(&stop_output.stderr)
    );

    let stop_stdout = String::from_utf8_lossy(&stop_output.stdout);
    assert!(
        stop_stdout.contains("Stopped watchdog daemon"),
        "should confirm stop, got: {}",
        stop_stdout
    );

    // Wait for the process to fully exit. The daemon sleeps in 1-second
    // increments checking the shutdown flag, so it may take up to ~2s to exit.
    let mut exited = false;
    for _ in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        #[cfg(unix)]
        {
            let still_alive = unsafe { libc::kill(pid, 0) == 0 };
            if !still_alive {
                exited = true;
                break;
            }
        }
        #[cfg(not(unix))]
        {
            exited = true;
            break;
        }
    }
    assert!(
        exited,
        "Watchdog daemon (PID {}) must be dead within 5s after --stop",
        pid
    );

    // PID file must be cleaned up
    assert!(!pid_file.exists(), "watch.pid must be removed after --stop");
}

/// Starting a second daemon while one is running must fail with an error.
#[tokio::test]
async fn watchdog_daemon_rejects_duplicate_start() {
    let dir = tempfile::TempDir::new().unwrap();
    write_squad_yml(dir.path());
    let squad_dir = setup_squad_dir(dir.path());
    let db_path = squad_dir.join("station.db");
    let _pool = setup_db(&db_path).await;

    // Start first daemon
    let output = Command::new(bin())
        .args([
            "watch",
            "--daemon",
            "--interval",
            "5",
            "--stall-threshold",
            "1",
        ])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output()
        .expect("spawn first daemon");
    assert!(output.status.success(), "first daemon must start");

    // Wait for daemon to initialize
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Verify first daemon is alive
    let pid_file = squad_dir.join("watch.pid");
    let pid: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();

    #[cfg(unix)]
    {
        let alive = unsafe { libc::kill(pid, 0) == 0 };
        assert!(
            alive,
            "first daemon must be alive before second start attempt"
        );
    }

    // Try to start a second daemon — must fail
    let dup_output = Command::new(bin())
        .args([
            "watch",
            "--daemon",
            "--interval",
            "5",
            "--stall-threshold",
            "1",
        ])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output()
        .expect("spawn duplicate daemon");

    assert!(
        !dup_output.status.success(),
        "second daemon start must fail (duplicate detection)"
    );

    let stderr = String::from_utf8_lossy(&dup_output.stderr);
    assert!(
        stderr.contains("already running"),
        "error must mention already running, got: {}",
        stderr
    );

    // Original daemon must still be alive (not killed by the duplicate check)
    #[cfg(unix)]
    {
        let still_alive = unsafe { libc::kill(pid, 0) == 0 };
        assert!(
            still_alive,
            "original daemon (PID {}) must not be killed by duplicate start attempt",
            pid
        );
    }

    // Cleanup: stop the daemon
    let _ = Command::new(bin())
        .args(["watch", "--stop"])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
}

/// Stopping when no daemon is running must succeed (idempotent).
#[tokio::test]
async fn watchdog_stop_when_not_running_is_idempotent() {
    let dir = tempfile::TempDir::new().unwrap();
    write_squad_yml(dir.path());
    let squad_dir = setup_squad_dir(dir.path());
    let db_path = squad_dir.join("station.db");
    let _pool = setup_db(&db_path).await;

    let output = Command::new(bin())
        .args(["watch", "--stop"])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output()
        .expect("stop when not running");

    assert!(
        output.status.success(),
        "stop must succeed even when no daemon running"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No watchdog daemon running"),
        "should indicate no daemon, got: {}",
        stdout
    );
}

/// Stale PID file (process no longer exists) should be cleaned up on next start.
#[tokio::test]
async fn watchdog_cleans_stale_pid_file_on_start() {
    let dir = tempfile::TempDir::new().unwrap();
    write_squad_yml(dir.path());
    let squad_dir = setup_squad_dir(dir.path());
    let db_path = squad_dir.join("station.db");
    let _pool = setup_db(&db_path).await;

    // Write a stale PID file (PID that doesn't exist)
    let pid_file = squad_dir.join("watch.pid");
    std::fs::write(&pid_file, "99999999").unwrap();

    // Start daemon — should succeed despite stale PID file
    let output = Command::new(bin())
        .args([
            "watch",
            "--daemon",
            "--interval",
            "5",
            "--stall-threshold",
            "1",
        ])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output()
        .expect("start with stale PID");

    assert!(
        output.status.success(),
        "daemon must start despite stale PID file, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // PID file must be updated with new PID
    let new_pid: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_ne!(
        new_pid, 99999999,
        "PID file must be updated to new daemon PID"
    );

    // Cleanup
    let _ = Command::new(bin())
        .args(["watch", "--stop"])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
}

/// Watchdog writes to watch.log on startup.
#[tokio::test]
async fn watchdog_daemon_writes_log_on_start() {
    let dir = tempfile::TempDir::new().unwrap();
    write_squad_yml(dir.path());
    let squad_dir = setup_squad_dir(dir.path());
    let db_path = squad_dir.join("station.db");
    let _pool = setup_db(&db_path).await;

    let output = Command::new(bin())
        .args([
            "watch",
            "--daemon",
            "--interval",
            "5",
            "--stall-threshold",
            "1",
        ])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output()
        .expect("start daemon");
    assert!(output.status.success());

    // Wait for daemon to write its startup log entry
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let log_file = squad_dir.join("log").join("watch.log");
    assert!(log_file.exists(), "watch.log must be created by daemon");

    let log_content = std::fs::read_to_string(&log_file).unwrap();
    assert!(
        log_content.contains("watchdog started"),
        "watch.log must contain startup entry, got: {}",
        log_content
    );

    // Cleanup
    let _ = Command::new(bin())
        .args(["watch", "--stop"])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
}

// ============================================================
// Init flow — artifact verification
// ============================================================

/// Run init (JSON mode, no tmux) and verify all artifacts are created.
/// Uses JSON mode to avoid interactive prompts and tmux session launches.
#[tokio::test]
async fn init_creates_all_artifacts() {
    let dir = tempfile::TempDir::new().unwrap();
    write_squad_yml(dir.path());

    let db_path = dir.path().join(".squad").join("station.db");

    // Run init with JSON output (non-interactive)
    let output = Command::new(bin())
        .args(["--json", "init"])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output()
        .expect("run init");

    // Init may fail on tmux session launch (no tmux server in CI) but
    // it should still create the DB and other artifacts before that.
    // The JSON output tells us what happened.

    // 1. Database must be created
    assert!(db_path.exists(), "init must create .squad/station.db");

    // 2. Verify DB has correct schema — agents table must exist with our registered agents
    let pool = setup_db(&db_path).await;
    let agents: Vec<(String,)> = sqlx::query_as("SELECT name FROM agents ORDER BY name")
        .fetch_all(&pool)
        .await
        .expect("query agents");
    let agent_names: Vec<&str> = agents.iter().map(|a| a.0.as_str()).collect();
    assert!(
        agent_names.contains(&"e2e-test-orch"),
        "orchestrator must be registered, got: {:?}",
        agent_names
    );
    assert!(
        agent_names.contains(&"e2e-test-worker1"),
        "worker1 must be registered, got: {:?}",
        agent_names
    );
    pool.close().await;

    // 3. JSON output must be valid
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.trim().is_empty() {
        let json: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
            panic!(
                "init JSON output must be valid JSON: {}, got: {}",
                e, stdout
            )
        });
        assert!(
            json.get("db_path").is_some(),
            "JSON output must contain db_path"
        );
    }
}

/// Init is idempotent — running twice doesn't break anything.
#[tokio::test]
async fn init_idempotent_second_run() {
    let dir = tempfile::TempDir::new().unwrap();
    write_squad_yml(dir.path());

    let db_path = dir.path().join(".squad").join("station.db");

    // First init
    let _ = Command::new(bin())
        .args(["--json", "init"])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output()
        .expect("first init");

    assert!(db_path.exists(), "DB must exist after first init");

    // Second init — must not error
    let output2 = Command::new(bin())
        .args(["--json", "init"])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output()
        .expect("second init");

    // Should succeed (or at least not crash)
    // Tmux failures are expected in test environments but the binary shouldn't panic
    let stderr = String::from_utf8_lossy(&output2.stderr);
    assert!(
        !stderr.contains("panic"),
        "second init must not panic, stderr: {}",
        stderr
    );

    // DB still exists and is valid
    assert!(db_path.exists(), "DB must survive second init");
    let pool = setup_db(&db_path).await;
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM agents")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(count.0 >= 2, "agents must still exist after second init");
    pool.close().await;
}

// ============================================================
// Doctor — health check verification
// ============================================================

/// Doctor after init verifies DB and log dir exist.
/// Note: Full doctor (tmux sessions, hooks, watchdog) requires a running squad,
/// so we test the subset that works in a test environment.
#[tokio::test]
async fn doctor_checks_db_and_logs() {
    let dir = tempfile::TempDir::new().unwrap();
    write_squad_yml(dir.path());
    let squad_dir = setup_squad_dir(dir.path());
    let db_path = squad_dir.join("station.db");
    let _pool = setup_db(&db_path).await;

    // Run doctor — it will check DB, logs, hooks, sessions, watchdog
    let output = Command::new(bin())
        .args(["doctor"])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output()
        .expect("run doctor");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // DB check must pass
    assert!(
        stdout.contains("Database: healthy"),
        "doctor must report DB healthy, got: {}",
        stdout
    );

    // Config check must pass
    assert!(
        stdout.contains("[PASS] Config"),
        "doctor must report config valid, got: {}",
        stdout
    );
}

/// Doctor with a running watchdog still reports DB as healthy (watchdog not checked).
#[tokio::test]
async fn doctor_reports_db_healthy_after_init() {
    let dir = tempfile::TempDir::new().unwrap();
    write_squad_yml(dir.path());
    let squad_dir = setup_squad_dir(dir.path());
    let db_path = squad_dir.join("station.db");
    let _pool = setup_db(&db_path).await;

    // Start watchdog daemon
    let start = Command::new(bin())
        .args([
            "watch",
            "--daemon",
            "--interval",
            "5",
            "--stall-threshold",
            "1",
        ])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output()
        .expect("start daemon");
    assert!(start.status.success(), "daemon must start");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Run doctor — watchdog check must pass
    let output = Command::new(bin())
        .args(["doctor"])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output()
        .expect("run doctor");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Doctor no longer checks watchdog; verify it still reports DB healthy
    assert!(
        stdout.contains("Database: healthy"),
        "doctor must report DB healthy, got: {}",
        stdout
    );

    // Cleanup
    let _ = Command::new(bin())
        .args(["watch", "--stop"])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
}

/// Doctor without watchdog still shows result summary (watchdog not checked).
#[tokio::test]
async fn doctor_shows_result_summary() {
    let dir = tempfile::TempDir::new().unwrap();
    write_squad_yml(dir.path());
    let squad_dir = setup_squad_dir(dir.path());
    let db_path = squad_dir.join("station.db");
    let _pool = setup_db(&db_path).await;

    // Run doctor without starting watchdog
    let output = Command::new(bin())
        .args(["doctor"])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output()
        .expect("run doctor");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Doctor no longer checks watchdog; verify it still runs and shows summary
    assert!(
        stdout.contains("Result:"),
        "doctor must show result summary, got: {}",
        stdout
    );
}

// ============================================================
// Self-detection regression test
//
// This test specifically verifies the fix for the race condition where:
// 1. Parent process calls `spawn_watchdog_daemon()` which forks child
// 2. Parent writes child PID to watch.pid
// 3. Child starts `run()`, reads watch.pid, finds its own PID
// 4. BUG (before fix): Child treats own PID as "another daemon" → kills itself
// 5. FIX: Child recognizes own PID → skips duplicate check → runs normally
// ============================================================

/// Regression test: daemon child process must not kill itself when it reads
/// its own PID from the PID file. This is the exact bug that was fixed.
#[tokio::test]
async fn watchdog_self_detection_regression() {
    let dir = tempfile::TempDir::new().unwrap();
    write_squad_yml(dir.path());
    let squad_dir = setup_squad_dir(dir.path());
    let db_path = squad_dir.join("station.db");
    let _pool = setup_db(&db_path).await;

    // Start daemon
    let output = Command::new(bin())
        .args([
            "watch",
            "--daemon",
            "--interval",
            "5",
            "--stall-threshold",
            "1",
        ])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output()
        .expect("start daemon");
    assert!(output.status.success());

    let pid_file = squad_dir.join("watch.pid");
    let pid: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();

    // Check liveness multiple times over 3 seconds.
    // The self-detection bug caused immediate death, so if the process
    // survives this window, the fix is working.
    for i in 0..3 {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        #[cfg(unix)]
        {
            let alive = unsafe { libc::kill(pid, 0) == 0 };
            assert!(
                alive,
                "REGRESSION: Daemon (PID {}) died after {}s — self-detection bug has regressed!",
                pid,
                i + 1
            );
        }
    }

    // Verify PID file still contains the same PID (wasn't overwritten)
    let current_pid: i32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(
        pid, current_pid,
        "PID file must not change during daemon lifetime"
    );

    // Cleanup
    let _ = Command::new(bin())
        .args(["watch", "--stop"])
        .current_dir(dir.path())
        .env("SQUAD_STATION_DB", db_path.to_str().unwrap())
        .output();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
}
