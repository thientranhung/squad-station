use owo_colors::OwoColorize;
use std::io::IsTerminal;

use crate::{config, db, tmux};

pub async fn run(dry_run: bool, json: bool) -> anyhow::Result<()> {
    let config_path = std::path::Path::new("squad.yml");
    let config = config::load_config(config_path)?;
    let db_path = config::resolve_db_path(&config)?;
    let pool = db::connect(&db_path).await?;

    let results = reconcile_agents(&pool, dry_run).await?;

    if json {
        let out = serde_json::json!({
            "reconciled": results.iter().filter(|r| r.action != "skip").count(),
            "agents": results,
            "dry_run": dry_run,
        });
        println!("{}", serde_json::to_string(&out)?);
    } else {
        let reconciled: Vec<&ReconcileResult> =
            results.iter().filter(|r| r.action != "skip").collect();
        if reconciled.is_empty() {
            if std::io::stdout().is_terminal() {
                println!("{} All agents in sync", "✓".green());
            } else {
                println!("All agents in sync");
            }
        } else {
            for r in &reconciled {
                if dry_run {
                    println!("[DRY RUN] {} → {}: {}", r.agent, r.action, r.reason);
                } else if std::io::stdout().is_terminal() {
                    println!(
                        "{} {} → {}: {}",
                        "✓".green(),
                        r.agent,
                        r.action,
                        r.reason
                    );
                } else {
                    println!("{} → {}: {}", r.agent, r.action, r.reason);
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug, serde::Serialize)]
pub struct ReconcileResult {
    pub agent: String,
    pub action: String,
    pub reason: String,
}

/// Reconcile all busy agents. Returns a list of actions taken.
/// This is also called by status and send commands for embedded reconciliation.
pub async fn reconcile_agents(
    pool: &sqlx::SqlitePool,
    dry_run: bool,
) -> anyhow::Result<Vec<ReconcileResult>> {
    let agents = db::agents::list_agents(pool).await?;
    let mut results = Vec::new();

    for agent in &agents {
        if agent.status != "busy" {
            continue;
        }

        // Pre-check: agent is "busy" but has zero processing messages in DB.
        // This is definitive — no heuristics needed. The task completed but agent
        // status was never reset (signal race, duplicate signal, etc).
        // Runs BEFORE the 2-minute grace period because zero-processing is conclusive.
        let processing_count = db::messages::count_processing(pool, &agent.name).await?;
        if processing_count == 0 {
            if !dry_run {
                db::agents::clear_current_task(pool, &agent.name).await?;
                db::agents::update_agent_status(pool, &agent.name, "idle").await?;
            }
            results.push(ReconcileResult {
                agent: agent.name.clone(),
                action: "orphan_reset".to_string(),
                reason: "busy in DB but zero processing messages".to_string(),
            });
            continue;
        }

        // Skip if agent became busy less than 2 minutes ago (probably still working)
        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&agent.status_updated_at) {
            let elapsed = chrono::Utc::now().signed_duration_since(ts);
            if elapsed.num_seconds() < 120 {
                results.push(ReconcileResult {
                    agent: agent.name.clone(),
                    action: "skip".to_string(),
                    reason: format!("busy for only {}s (< 2m threshold)", elapsed.num_seconds()),
                });
                continue;
            }
        }

        if !tmux::session_exists(&agent.name) {
            // Session is dead
            if !dry_run {
                db::agents::update_agent_status(pool, &agent.name, "dead").await?;
            }
            results.push(ReconcileResult {
                agent: agent.name.clone(),
                action: "mark_dead".to_string(),
                reason: "no tmux session".to_string(),
            });
            continue;
        }

        // Agent has processing messages and session is alive — still working.
        // (Task completion is handled exclusively by the signal hook, not by reconcile.)
        results.push(ReconcileResult {
            agent: agent.name.clone(),
            action: "skip".to_string(),
            reason: "agent has live session and processing messages".to_string(),
        });
    }

    Ok(results)
}