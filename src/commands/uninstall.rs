use anyhow::Result;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use crate::config;

use super::clean;

/// Remove all hook entries whose command contains "squad-station" from a JSON settings file.
/// Cleans up Stop, Notification, PostToolUse, AfterAgent, SessionStart events.
/// Removes the event key entirely if no entries remain after filtering.
fn remove_squad_hooks(settings_file: &str) -> Result<bool> {
    let path = Path::new(settings_file);
    if !path.exists() {
        return Ok(false);
    }

    let content = std::fs::read_to_string(path)?;
    let mut settings: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Ok(false),
    };

    let hooks_obj = match settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        Some(h) => h,
        None => return Ok(false),
    };

    let events = [
        "Stop",
        "Notification",
        "PostToolUse",
        "AfterAgent",
        "SessionStart",
    ];
    let mut changed = false;

    for event in &events {
        if let Some(entries) = hooks_obj.get(*event).and_then(|e| e.as_array()) {
            // Keep only entries that have NO hook command containing "squad-station"
            let filtered: Vec<serde_json::Value> = entries
                .iter()
                .filter(|entry| {
                    let hooks = entry.get("hooks").and_then(|h| h.as_array());
                    match hooks {
                        None => true, // no hooks array — keep it (not ours)
                        Some(hooks) => !hooks.iter().any(|h| {
                            h.get("command")
                                .and_then(|c| c.as_str())
                                .map(|cmd| {
                                    cmd.contains("squad-station") || cmd.contains(".squad/hooks/")
                                })
                                .unwrap_or(false)
                        }),
                    }
                })
                .cloned()
                .collect();

            if filtered.len() != entries.len() {
                changed = true;
                if filtered.is_empty() {
                    hooks_obj.remove(*event);
                } else {
                    hooks_obj.insert(event.to_string(), serde_json::Value::Array(filtered));
                }
            }
        }
    }

    // Remove "hooks" key entirely if now empty
    if hooks_obj.is_empty() {
        settings.as_object_mut().map(|o| o.remove("hooks"));
    }

    if changed {
        std::fs::write(path, serde_json::to_string_pretty(&settings)?)?;
    }

    Ok(changed)
}

/// Return the provider-specific paths for hooks file, doc file, and orchestrator playbook.
fn provider_paths(provider: &str) -> (&'static str, &'static str, &'static str) {
    match provider {
        "codex" => (
            ".codex/hooks.json",
            "AGENTS.md",
            ".codex/commands/squad-orchestrator.md",
        ),
        "gemini-cli" => (
            ".gemini/settings.json",
            "GEMINI.md",
            ".gemini/commands/squad-orchestrator.md",
        ),
        _ => (
            ".claude/settings.json",
            "CLAUDE.md",
            ".claude/commands/squad-orchestrator.md",
        ),
    }
}

pub async fn run(config_path: PathBuf, yes: bool) -> Result<()> {
    let config = config::load_config(&config_path)?;
    let db_path = config::resolve_db_path(&config)?;
    let squad_dir = db_path.parent().unwrap_or(Path::new(".squad"));

    let (hooks_file, _doc_file, orchestrator_md) = provider_paths(&config.orchestrator.provider);

    // Also check orchestrator provider for agent providers (agents may differ)
    let agent_hooks_files: Vec<&str> = config
        .agents
        .iter()
        .map(|a| provider_paths(&a.provider).0)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    if !yes {
        println!();
        println!("This will:");
        println!("  • Kill all squad tmux sessions");
        println!("  • Remove squad-station hooks from {}", hooks_file);
        for f in &agent_hooks_files {
            if *f != hooks_file {
                println!("  • Remove squad-station hooks from {}", f);
            }
        }
        println!("  • Delete {}", orchestrator_md);
        println!("  • Delete .squad/ directory");
        println!("  • squad.yml is preserved");
        println!();
        eprint!("Proceed? [y/N]: ");
        let stdin = io::stdin();
        let mut line = String::new();
        stdin.lock().read_line(&mut line)?;
        if !line.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         Squad Station  •  Uninstall                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // 1. Stop watchdog
    if clean::stop_watchdog(squad_dir) {
        println!("  [STOP]    watchdog daemon");
    }

    // 2. Kill tmux sessions
    let (killed, killed_names, _) = clean::kill_all_sessions(&config)?;
    for name in &killed_names {
        println!("  [KILLED]  tmux session: {}", name);
    }
    if killed == 0 {
        println!("  [SKIP]    no running tmux sessions");
    }

    // 3. Remove hooks from orchestrator provider settings
    match remove_squad_hooks(hooks_file) {
        Ok(true) => println!("  [CLEANED] hooks in {}", hooks_file),
        Ok(false) => println!("  [SKIP]    {} — no squad hooks found", hooks_file),
        Err(e) => eprintln!("  [WARN]    failed to clean {}: {}", hooks_file, e),
    }

    // 4. Remove hooks from agent provider settings (if different provider)
    for f in &agent_hooks_files {
        if *f != hooks_file {
            match remove_squad_hooks(f) {
                Ok(true) => println!("  [CLEANED] hooks in {}", f),
                Ok(false) => println!("  [SKIP]    {} — no squad hooks found", f),
                Err(e) => eprintln!("  [WARN]    failed to clean {}: {}", f, e),
            }
        }
    }

    // 5. Delete squad-orchestrator.md
    let orch_path = Path::new(orchestrator_md);
    if orch_path.exists() {
        std::fs::remove_file(orch_path)?;
        println!("  [DELETED] {}", orchestrator_md);
    } else {
        println!("  [SKIP]    {} — not found", orchestrator_md);
    }

    // 6. Delete .squad/ directory entirely
    if squad_dir.exists() {
        std::fs::remove_dir_all(squad_dir)?;
        println!("  [DELETED] {}/", squad_dir.display());
    } else {
        println!("  [SKIP]    .squad/ — not found");
    }

    // 7. Note about .env.squad (credentials file at project root, not inside .squad/)
    let project_root = squad_dir.parent().unwrap_or(Path::new("."));
    if project_root.join(".env.squad").exists() {
        println!("  [SKIP]    .env.squad preserved (contains credentials)");
    }

    println!();
    println!("  squad.yml preserved — re-run `squad-station init` to reinstall.");
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_remove_squad_hooks_removes_stop_and_notification() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let content = serde_json::json!({
            "hooks": {
                "Stop": [{"matcher": "", "hooks": [{"type": "command", "command": "squad-station signal \"x\" 2>/dev/null"}]}],
                "Notification": [{"matcher": "permission_prompt", "hooks": [{"type": "command", "command": "squad-station notify --body 'x' 2>/dev/null"}]}],
                "OtherHook": [{"matcher": "", "hooks": [{"type": "command", "command": "my-other-tool run"}]}]
            }
        });
        fs::write(&path, serde_json::to_string_pretty(&content).unwrap()).unwrap();

        let result = remove_squad_hooks(path.to_str().unwrap()).unwrap();
        assert!(result);

        let updated: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        // Stop and Notification removed
        assert!(updated["hooks"]["Stop"].is_null());
        assert!(updated["hooks"]["Notification"].is_null());
        // OtherHook preserved
        assert!(updated["hooks"]["OtherHook"].is_array());
    }

    #[test]
    fn test_remove_squad_hooks_preserves_non_squad_entries() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let content = serde_json::json!({
            "hooks": {
                "Stop": [
                    {"matcher": "", "hooks": [{"type": "command", "command": "squad-station signal \"x\""}]},
                    {"matcher": "", "hooks": [{"type": "command", "command": "my-other-stop-hook"}]}
                ]
            }
        });
        fs::write(&path, serde_json::to_string_pretty(&content).unwrap()).unwrap();

        remove_squad_hooks(path.to_str().unwrap()).unwrap();

        let updated: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let stop = updated["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1);
        assert_eq!(
            stop[0]["hooks"][0]["command"].as_str().unwrap(),
            "my-other-stop-hook"
        );
    }

    #[test]
    fn test_remove_squad_hooks_removes_hooks_key_when_empty() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let content = serde_json::json!({
            "hooks": {
                "Stop": [{"matcher": "", "hooks": [{"type": "command", "command": "squad-station signal \"x\""}]}]
            },
            "model": "opus"
        });
        fs::write(&path, serde_json::to_string_pretty(&content).unwrap()).unwrap();

        remove_squad_hooks(path.to_str().unwrap()).unwrap();

        let updated: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(updated.get("hooks").is_none());
        assert_eq!(updated["model"].as_str().unwrap(), "opus");
    }

    #[test]
    fn test_remove_squad_hooks_skips_missing_file() {
        let result = remove_squad_hooks("/nonexistent/path/settings.json").unwrap();
        assert!(!result);
    }

    #[test]
    fn test_provider_paths_claude_code() {
        let (hooks, doc, orch) = super::provider_paths("claude-code");
        assert_eq!(hooks, ".claude/settings.json");
        assert_eq!(doc, "CLAUDE.md");
        assert_eq!(orch, ".claude/commands/squad-orchestrator.md");
    }

    #[test]
    fn test_provider_paths_codex() {
        let (hooks, doc, orch) = super::provider_paths("codex");
        assert_eq!(hooks, ".codex/hooks.json");
        assert_eq!(doc, "AGENTS.md");
        assert_eq!(orch, ".codex/commands/squad-orchestrator.md");
    }

    #[test]
    fn test_provider_paths_gemini() {
        let (hooks, doc, orch) = super::provider_paths("gemini-cli");
        assert_eq!(hooks, ".gemini/settings.json");
        assert_eq!(doc, "GEMINI.md");
        assert_eq!(orch, ".gemini/commands/squad-orchestrator.md");
    }

    #[test]
    fn test_remove_squad_hooks_removes_telegram() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let content = serde_json::json!({
            "hooks": {
                "Stop": [
                    {"matcher": "", "hooks": [{"type": "command", "command": "squad-station signal \"x\" 2>/dev/null"}]},
                    {"matcher": "", "hooks": [{"type": "command", "command": "cd \"/tmp\" && squad-station notify-telegram --event Stop 2>/dev/null; true"}]}
                ],
                "OtherHook": [{"matcher": "", "hooks": [{"type": "command", "command": "my-other-tool"}]}]
            }
        });
        fs::write(&path, serde_json::to_string_pretty(&content).unwrap()).unwrap();

        let result = remove_squad_hooks(path.to_str().unwrap()).unwrap();
        assert!(result);

        let updated: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        // Both squad-station and telegram hooks removed
        assert!(updated["hooks"]["Stop"].is_null());
        // OtherHook preserved
        assert!(updated["hooks"]["OtherHook"].is_array());
    }
}
