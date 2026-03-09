use std::path::PathBuf;

use crate::{config, db, tmux};

pub async fn run(config_path: PathBuf, json: bool) -> anyhow::Result<()> {
    // 1. Parse squad.yml
    let config = config::load_config(&config_path)?;

    // 2. Resolve DB path
    let db_path = config::resolve_db_path(&config)?;

    // 3. Connect to DB (creates file + runs migrations)
    let pool = db::connect(&db_path).await?;

    // 4. Register orchestrator with hardcoded role="orchestrator"
    let orch_role = config
        .orchestrator
        .name
        .as_deref()
        .unwrap_or("orchestrator");
    let orch_name = format!(
        "{}-{}-{}",
        config.project, config.orchestrator.tool, orch_role
    );
    db::agents::insert_agent(
        &pool,
        &orch_name,
        &config.orchestrator.tool,
        "orchestrator",
        config.orchestrator.model.as_deref(),
        config.orchestrator.description.as_deref(),
    )
    .await?;

    // 5. Launch orchestrator tmux session (or skip if db-only provider)
    let mut db_only_names: Vec<String> = vec![];
    let orch_launched = if config.orchestrator.is_db_only() {
        // Antigravity: DB-only orchestrator — register to DB only, no tmux session.
        db_only_names.push(orch_name.clone());
        false
    } else if tmux::session_exists(&orch_name) {
        false
    } else {
        tmux::launch_agent(&orch_name, &config.orchestrator.tool)?;
        true
    };
    let orch_skipped = !orch_launched && !config.orchestrator.is_db_only();

    // 6. Register and launch each worker agent — continue on partial failure
    let mut failed: Vec<(String, String)> = vec![];
    let mut skipped_names: Vec<String> = vec![];
    let mut launched: u32 = if orch_launched { 1 } else { 0 };
    let mut skipped: u32 = if orch_skipped { 1 } else { 0 };

    if orch_skipped {
        skipped_names.push(orch_name.clone());
    }

    for agent in &config.agents {
        let role_suffix = agent.name.as_deref().unwrap_or(&agent.role);
        let agent_name = format!("{}-{}-{}", config.project, agent.tool, role_suffix);
        if let Err(e) = db::agents::insert_agent(
            &pool,
            &agent_name,
            &agent.tool,
            &agent.role,
            agent.model.as_deref(),
            agent.description.as_deref(),
        )
        .await
        {
            failed.push((agent_name.clone(), format!("{e:#}")));
            continue;
        }

        if tmux::session_exists(&agent_name) {
            skipped += 1;
            skipped_names.push(agent_name.clone());
            continue; // Idempotent: skip already-running agents
        }

        match tmux::launch_agent(&agent_name, &agent.tool) {
            Ok(()) => launched += 1,
            Err(e) => failed.push((agent_name.clone(), format!("{e:#}"))),
        }
    }

    // 7. Output results
    let db_path_str = db_path.display().to_string();

    if json {
        let output = serde_json::json!({
            "launched": launched,
            "skipped": skipped,
            "failed": failed,
            "db_path": db_path_str,
        });
        println!("{}", serde_json::to_string(&output)?);
    } else {
        println!(
            "Initialized squad '{}' with {} agent(s)",
            config.project, launched
        );
        for name in &skipped_names {
            println!("  - {}: already running (skipped)", name);
        }
        for name in &db_only_names {
            println!("  {}: db-only (antigravity orchestrator — no tmux session)", name);
        }
        for (name, error) in &failed {
            println!("  x {}: {}", name, error);
        }
        println!("  Database: {}", db_path_str);
    }

    // 8. Exit code: return Err only if ALL agents failed (including orchestrator)
    // DB-only orchestrator is excluded from total: it is never launched and never fails.
    let total = config.agents.len() + if config.orchestrator.is_db_only() { 0 } else { 1 };
    if !failed.is_empty() && failed.len() == total {
        anyhow::bail!("All {} agent(s) failed to launch", total);
    }

    // 9. Hook setup: merge into settings.json or print instructions
    // In JSON mode, skip stdout instructions (to preserve machine-parseable output).
    if !json {
        let providers: &[(&str, &str)] = &[
            (".claude/settings.json", "Stop"),
            (".gemini/settings.json", "AfterAgent"),
        ];
        for &(settings_path, hook_event) in providers {
            let path = std::path::Path::new(settings_path);
            if path.exists() {
                match merge_hook_entry(path, hook_event) {
                    Ok(()) => {}
                    Err(e) => {
                        // Graceful: warn but do not abort init
                        eprintln!("  Warning: could not merge hook into {}: {}", settings_path, e);
                        print_hook_instructions(settings_path, hook_event);
                    }
                }
            } else {
                print_hook_instructions(settings_path, hook_event);
            }
        }
    }

    Ok(())
}

fn merge_hook_entry(path: &std::path::Path, event: &str) -> anyhow::Result<()> {
    // 1. Backup
    let bak = path.with_extension("json.bak");
    std::fs::copy(path, &bak)?;

    // 2. Parse (graceful fallback on malformed JSON)
    let content = std::fs::read_to_string(path)?;
    let mut root: serde_json::Value = serde_json::from_str(&content)
        .unwrap_or_else(|_| serde_json::json!({}));

    // 3. Ensure hooks object exists
    if root.get("hooks").is_none() {
        root["hooks"] = serde_json::json!({});
    }

    // 4. Ensure event array exists
    if root["hooks"].get(event).is_none() {
        root["hooks"][event] = serde_json::json!([]);
    }

    // 5. Append entry if not already present (dedup on "command" field)
    let hook_cmd = "squad-station signal $TMUX_PANE";
    let already_present = root["hooks"][event]
        .as_array()
        .map(|arr| {
            arr.iter().any(|entry| {
                entry.get("command").and_then(|c| c.as_str()) == Some(hook_cmd)
            })
        })
        .unwrap_or(false);

    if !already_present {
        let entry = serde_json::json!({ "type": "command", "command": hook_cmd });
        root["hooks"][event]
            .as_array_mut()
            .expect("ensured above")
            .push(entry);
    }

    // 6. Write back (pretty-printed)
    let output = serde_json::to_string_pretty(&root)? + "\n";
    std::fs::write(path, output)?;
    println!("  Updated {} (backup: {}.bak)", path.display(), path.display());
    Ok(())
}

fn print_hook_instructions(settings_path: &str, event: &str) {
    println!(
        "\nHook setup instructions for {}:\n\n  \
        Create the file with the following content, or add to your existing hooks:\n\n  \
        {{\n    \"hooks\": {{\n      \"{}\": [\n        \
        {{ \"type\": \"command\", \"command\": \"squad-station signal $TMUX_PANE\" }}\n      \
        ]\n    }}\n  }}",
        settings_path, event
    );
}
