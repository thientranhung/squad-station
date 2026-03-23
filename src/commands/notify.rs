use owo_colors::OwoColorize;
use std::io::IsTerminal;

use crate::{config, db, tmux};

/// Best-effort log for notify diagnostics. Mirrors signal.rs log_signal().
/// Uses CWD-relative `.squad/log/signal.log` — same file as signal for unified diagnostics.
fn log_notify(level: &str, agent: &str, msg: &str) {
    let log_dir = std::path::Path::new(".squad").join("log");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_file = log_dir.join("signal.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
    {
        use std::io::Write;
        let _ = writeln!(
            f,
            "{} {:<5} agent={} notify: {}",
            chrono::Utc::now().to_rfc3339(),
            level,
            agent,
            msg
        );
    }
}

pub async fn run(body: String, agent: Option<String>, json: bool) -> anyhow::Result<()> {
    // Resolve agent name: explicit arg or auto-detect from tmux session
    let agent = match agent {
        Some(name) if !name.is_empty() => name,
        _ => {
            // Try to get session name from tmux
            let output = std::process::Command::new("tmux")
                .args(["display-message", "-p", "#S"])
                .output();
            match output {
                Ok(o) if o.status.success() => {
                    let name = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if name.is_empty() {
                        log_notify("GUARD", "(empty)", "reason=no_agent_name");
                        eprintln!("squad-station: notify: no agent name");
                        return Ok(());
                    }
                    name
                }
                _ => {
                    log_notify("GUARD", "(empty)", "reason=tmux_failed");
                    eprintln!("squad-station: notify: cannot detect agent name");
                    return Ok(());
                }
            }
        }
    };

    // Load config and connect to DB
    let config_path = std::path::Path::new("squad.yml");
    let config = match config::load_config(config_path) {
        Ok(c) => c,
        Err(e) => {
            log_notify("GUARD", &agent, &format!("reason=config_error err={e}"));
            eprintln!("squad-station: warning: {e}");
            return Ok(());
        }
    };
    let db_path = match config::resolve_db_path(&config) {
        Ok(p) => p,
        Err(e) => {
            log_notify("GUARD", &agent, &format!("reason=db_path_error err={e}"));
            eprintln!("squad-station: warning: {e}");
            return Ok(());
        }
    };
    let pool = match db::connect(&db_path).await {
        Ok(p) => p,
        Err(e) => {
            log_notify(
                "GUARD",
                &agent,
                &format!("reason=db_connection_failed err={e}"),
            );
            eprintln!("squad-station: warning: DB connection failed: {e}");
            return Ok(());
        }
    };

    // Verify agent exists
    let agent_record = match db::agents::get_agent(&pool, &agent).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            log_notify("GUARD", &agent, "reason=agent_not_found");
            if std::io::stdout().is_terminal() {
                println!("Agent not found: {} (ignored)", agent);
            }
            return Ok(());
        }
        Err(e) => {
            log_notify(
                "GUARD",
                &agent,
                &format!("reason=get_agent_error err={e}"),
            );
            eprintln!("squad-station: warning: get_agent failed: {e}");
            return Ok(());
        }
    };

    // Skip if orchestrator (prevent self-notification loop)
    if agent_record.role == "orchestrator" {
        log_notify("GUARD", &agent, "reason=orchestrator_self_notify");
        if json {
            println!(
                "{}",
                serde_json::json!({"notified": false, "reason": "orchestrator_skip"})
            );
        }
        return Ok(());
    }

    // Find orchestrator and send notification
    let orchestrator = db::agents::get_orchestrator(&pool).await?;
    let notified = if let Some(orch) = orchestrator {
        if orch.tool == "antigravity" {
            log_notify("OK", &agent, "notified=false reason=antigravity");
            false // DB-only orchestrator
        } else if tmux::session_exists(&orch.name) {
            let notification = format!("[SQUAD INPUT NEEDED] Agent '{}': {}", agent, body);
            match tmux::send_keys_literal(&orch.name, &notification).await {
                Ok(()) => {
                    log_notify(
                        "OK",
                        &agent,
                        &format!("notified=true orch={}", orch.name),
                    );
                    true
                }
                Err(e) => {
                    log_notify(
                        "WARN",
                        &agent,
                        &format!("notified=false reason=send_keys_failed err={e}"),
                    );
                    false
                }
            }
        } else {
            log_notify(
                "WARN",
                &agent,
                &format!("notified=false reason=orch_session_missing orch={}", orch.name),
            );
            false
        }
    } else {
        log_notify("WARN", &agent, "notified=false reason=no_orchestrator");
        false
    };

    // Output result (do NOT change task status or agent status)
    if json {
        let out = serde_json::json!({
            "notified": notified,
            "agent": agent,
            "body": body,
        });
        println!("{}", serde_json::to_string(&out)?);
    } else if std::io::stdout().is_terminal() {
        println!(
            "{} Notification sent from {} to orchestrator",
            "✓".green(),
            agent
        );
    } else {
        println!("Notification sent from {} to orchestrator", agent);
    }

    Ok(())
}
