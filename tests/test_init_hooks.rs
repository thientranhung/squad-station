/// Integration tests for hook generation during `squad-station init`.
///
/// Tests hook generation by calling the public install_*_hooks_pub() functions
/// with absolute paths to temp directories, then verifying the generated file
/// content matches per-provider requirements.
///
/// Bug context:
/// - v0.8.6: Codex PostToolUse hook was still generated (not caught at integration level)
/// - v0.8.7: notify-telegram.sh sent notifications for non-agent sessions
use squad_station::commands::init::{
    install_claude_hooks_pub, install_codex_hooks_pub, install_gemini_hooks_pub,
    install_session_start_hook_pub, install_telegram_hooks_pub,
};
use squad_station::config;

// ============================================================
// Claude Code provider — .claude/settings.json
// ============================================================

#[test]
fn test_hooks_claude_code_full_structure() {
    let tmp = tempfile::TempDir::new().unwrap();
    let settings_file = tmp.path().join(".claude").join("settings.json");
    let settings_str = settings_file.to_str().unwrap();

    assert!(install_claude_hooks_pub(settings_str).unwrap());

    let content = std::fs::read_to_string(&settings_file).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Stop hook: signal completion
    assert!(
        settings["hooks"]["Stop"].is_array(),
        "Claude Code must have Stop hook"
    );
    let stop_cmd = settings["hooks"]["Stop"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(
        stop_cmd.contains("squad-station signal"),
        "Stop hook must call squad-station signal: {stop_cmd}"
    );
    assert!(
        stop_cmd.contains("tmux display-message"),
        "Stop hook must use tmux display-message: {stop_cmd}"
    );
    assert!(
        stop_cmd.contains("2>/dev/null"),
        "Stop hook must redirect stderr: {stop_cmd}"
    );

    // Notification hook: permission_prompt + elicitation_dialog
    let notif = &settings["hooks"]["Notification"];
    assert!(notif.is_array(), "Claude Code must have Notification hook");
    assert_eq!(
        notif.as_array().unwrap().len(),
        2,
        "Notification must have 2 matchers (permission_prompt + elicitation_dialog)"
    );
    assert_eq!(notif[0]["matcher"].as_str().unwrap(), "permission_prompt");
    assert_eq!(notif[1]["matcher"].as_str().unwrap(), "elicitation_dialog");

    // PostToolUse hook: AskUserQuestion
    let ptu = &settings["hooks"]["PostToolUse"];
    assert!(ptu.is_array(), "Claude Code must have PostToolUse hook");
    assert_eq!(ptu[0]["matcher"].as_str().unwrap(), "AskUserQuestion");
    let ptu_cmd = ptu[0]["hooks"][0]["command"].as_str().unwrap();
    assert!(
        ptu_cmd.contains("squad-station notify"),
        "PostToolUse must call squad-station notify: {ptu_cmd}"
    );

    // Claude Code hooks must NOT output JSON (stdout is ignored)
    assert!(
        !stop_cmd.contains("printf"),
        "Claude Code Stop hook must NOT include printf: {stop_cmd}"
    );

    // SessionStart must NOT be auto-installed
    assert!(
        settings["hooks"]["SessionStart"].is_null(),
        "SessionStart must not be installed by base hooks"
    );
}

#[test]
fn test_hooks_claude_code_no_env_var_session_name() {
    let tmp = tempfile::TempDir::new().unwrap();
    let settings_file = tmp.path().join(".claude").join("settings.json");
    install_claude_hooks_pub(settings_file.to_str().unwrap()).unwrap();

    let content = std::fs::read_to_string(&settings_file).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    for event in &["Stop", "Notification", "PostToolUse"] {
        if let Some(entries) = settings["hooks"][event].as_array() {
            for entry in entries {
                if let Some(hook_list) = entry["hooks"].as_array() {
                    for hook in hook_list {
                        let cmd = hook["command"].as_str().unwrap_or("");
                        assert!(
                            !cmd.contains("SQUAD_AGENT_NAME"),
                            "{event}: must NOT use $SQUAD_AGENT_NAME: {cmd}"
                        );
                        assert!(
                            !cmd.contains("TMUX_PANE"),
                            "{event}: must NOT use $TMUX_PANE: {cmd}"
                        );
                        assert!(
                            !cmd.contains("list-panes"),
                            "{event}: must NOT use list-panes: {cmd}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn test_hooks_claude_code_preserves_existing_settings() {
    let tmp = tempfile::TempDir::new().unwrap();
    let claude_dir = tmp.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();

    let existing = serde_json::json!({
        "permissions": {"allow": ["Bash(npm test)", "Read"]},
        "hooks": {
            "PreToolUse": [{"matcher": "Bash", "hooks": []}]
        }
    });
    std::fs::write(
        claude_dir.join("settings.json"),
        serde_json::to_string_pretty(&existing).unwrap(),
    )
    .unwrap();

    let settings_file = claude_dir.join("settings.json");
    install_claude_hooks_pub(settings_file.to_str().unwrap()).unwrap();

    let content = std::fs::read_to_string(&settings_file).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Existing hooks preserved
    assert!(
        settings["hooks"]["PreToolUse"].is_array(),
        "Existing PreToolUse hooks must be preserved"
    );
    // Squad hooks added
    assert!(settings["hooks"]["Stop"].is_array());
    assert!(settings["hooks"]["PostToolUse"].is_array());
    assert!(settings["hooks"]["Notification"].is_array());
}

#[test]
fn test_hooks_claude_code_idempotent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let settings_file = tmp.path().join(".claude").join("settings.json");
    let path = settings_file.to_str().unwrap();

    install_claude_hooks_pub(path).unwrap();
    install_claude_hooks_pub(path).unwrap();

    let content = std::fs::read_to_string(&settings_file).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(
        settings["hooks"]["Stop"].as_array().unwrap().len(),
        1,
        "Stop must have exactly 1 entry after double install"
    );
    assert_eq!(
        settings["hooks"]["Notification"].as_array().unwrap().len(),
        2,
        "Notification must have exactly 2 entries after double install"
    );
    assert_eq!(
        settings["hooks"]["PostToolUse"].as_array().unwrap().len(),
        1,
        "PostToolUse must have exactly 1 entry after double install"
    );
}

/// Regression test: install_claude_hooks must preserve user-added and third-party
/// hook entries. Before the merge fix, it overwrote entire event arrays.
#[test]
fn test_hooks_claude_code_preserves_custom_entries_in_same_event() {
    let tmp = tempfile::TempDir::new().unwrap();
    let claude_dir = tmp.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();

    // Pre-populate with squad signal + telegram + user custom logger in Stop
    let existing = serde_json::json!({
        "hooks": {
            "Stop": [
                {
                    "matcher": "",
                    "hooks": [{"type": "command", "command": "squad-station signal old-path 2>/dev/null"}]
                },
                {
                    "matcher": "",
                    "hooks": [{"type": "command", "command": "squad-station notify-telegram --project-root /foo 2>/dev/null; true"}]
                },
                {
                    "matcher": "",
                    "hooks": [{"type": "command", "command": "/usr/local/bin/my-custom-logger --event stop"}]
                }
            ],
            "Notification": [
                {
                    "matcher": "permission_prompt",
                    "hooks": [{"type": "command", "command": "squad-station notify --body old 2>/dev/null"}]
                },
                {
                    "matcher": "",
                    "hooks": [{"type": "command", "command": "/usr/local/bin/my-slack-notifier"}]
                }
            ],
            "PostToolUse": [
                {
                    "matcher": "AskUserQuestion",
                    "hooks": [{"type": "command", "command": "squad-station notify --body old 2>/dev/null"}]
                },
                {
                    "matcher": "Bash",
                    "hooks": [{"type": "command", "command": "/usr/local/bin/my-audit-log"}]
                }
            ]
        }
    });
    std::fs::write(
        claude_dir.join("settings.json"),
        serde_json::to_string_pretty(&existing).unwrap(),
    )
    .unwrap();

    let settings_file = claude_dir.join("settings.json");
    install_claude_hooks_pub(settings_file.to_str().unwrap()).unwrap();

    let content = std::fs::read_to_string(&settings_file).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Stop: telegram + custom logger preserved, signal updated
    let stop = settings["hooks"]["Stop"].as_array().unwrap();
    let stop_cmds: Vec<&str> = stop
        .iter()
        .filter_map(|e| e["hooks"][0]["command"].as_str())
        .collect();
    assert!(
        stop_cmds.iter().any(|c| c.contains("notify-telegram")),
        "Telegram hook must be preserved in Stop: {stop_cmds:?}"
    );
    assert!(
        stop_cmds.iter().any(|c| c.contains("my-custom-logger")),
        "Custom logger hook must be preserved in Stop: {stop_cmds:?}"
    );
    assert!(
        stop_cmds
            .iter()
            .any(|c| c.contains("squad-station signal") && !c.contains("old-path")),
        "Signal hook must be updated (not old-path) in Stop: {stop_cmds:?}"
    );
    assert_eq!(
        stop.len(),
        3,
        "Stop must have exactly 3 entries: {stop_cmds:?}"
    );

    // Notification: slack notifier preserved, squad entries updated
    let notif = settings["hooks"]["Notification"].as_array().unwrap();
    let notif_cmds: Vec<&str> = notif
        .iter()
        .filter_map(|e| e["hooks"][0]["command"].as_str())
        .collect();
    assert!(
        notif_cmds.iter().any(|c| c.contains("my-slack-notifier")),
        "Slack notifier must be preserved in Notification: {notif_cmds:?}"
    );
    // 1 user entry + 2 squad entries (permission_prompt + elicitation_dialog)
    assert_eq!(
        notif.len(),
        3,
        "Notification must have 3 entries: {notif_cmds:?}"
    );

    // PostToolUse: audit log preserved, squad entry updated
    let ptu = settings["hooks"]["PostToolUse"].as_array().unwrap();
    let ptu_cmds: Vec<&str> = ptu
        .iter()
        .filter_map(|e| e["hooks"][0]["command"].as_str())
        .collect();
    assert!(
        ptu_cmds.iter().any(|c| c.contains("my-audit-log")),
        "Audit log hook must be preserved in PostToolUse: {ptu_cmds:?}"
    );
    assert_eq!(
        ptu.len(),
        2,
        "PostToolUse must have 2 entries: {ptu_cmds:?}"
    );
}

// ============================================================
// Codex provider — .codex/hooks.json
// ============================================================

#[test]
fn test_hooks_codex_full_structure() {
    let tmp = tempfile::TempDir::new().unwrap();
    let hooks_file = tmp.path().join(".codex").join("hooks.json");
    let hooks_str = hooks_file.to_str().unwrap();

    assert!(install_codex_hooks_pub(hooks_str).unwrap());

    let content = std::fs::read_to_string(&hooks_file).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Stop hook: signal completion
    assert!(
        settings["hooks"]["Stop"].is_array(),
        "Codex must have Stop hook"
    );
    let stop_cmd = settings["hooks"]["Stop"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(
        stop_cmd.contains("squad-station signal"),
        "Stop hook must call squad-station signal: {stop_cmd}"
    );
    assert!(
        stop_cmd.contains("tmux display-message"),
        "Stop hook must use tmux display-message: {stop_cmd}"
    );

    // CRITICAL regression guard: Codex must NOT have PostToolUse hook
    assert!(
        settings["hooks"]["PostToolUse"].is_null(),
        "Codex must NOT have PostToolUse hook (--yolo mode, no input prompts)"
    );

    // CRITICAL: Codex must NOT have Notification hook
    assert!(
        settings["hooks"]["Notification"].is_null(),
        "Codex must NOT have Notification hook"
    );

    // Codex hooks must NOT output JSON (not required)
    assert!(
        !stop_cmd.contains("printf"),
        "Codex Stop hook must NOT include printf: {stop_cmd}"
    );

    // SessionStart must NOT be auto-installed
    assert!(
        settings["hooks"]["SessionStart"].is_null(),
        "SessionStart must not be installed by base hooks"
    );
}

#[test]
fn test_hooks_codex_creates_config_toml_feature_flag() {
    let tmp = tempfile::TempDir::new().unwrap();
    let hooks_file = tmp.path().join(".codex").join("hooks.json");
    install_codex_hooks_pub(hooks_file.to_str().unwrap()).unwrap();

    let config_toml = tmp.path().join(".codex/config.toml");
    assert!(
        config_toml.exists(),
        ".codex/config.toml must be created for feature flag"
    );
    let content = std::fs::read_to_string(&config_toml).unwrap();
    assert!(
        content.contains("[features]"),
        "config.toml must contain [features] section: {content}"
    );
    assert!(
        content.contains("codex_hooks = true"),
        "config.toml must contain codex_hooks = true: {content}"
    );
}

#[test]
fn test_hooks_codex_does_not_create_claude_or_gemini_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let hooks_file = tmp.path().join(".codex").join("hooks.json");
    install_codex_hooks_pub(hooks_file.to_str().unwrap()).unwrap();

    assert!(
        !tmp.path().join(".claude/settings.json").exists(),
        "Codex hook install must not create .claude/settings.json"
    );
    assert!(
        !tmp.path().join(".gemini/settings.json").exists(),
        "Codex hook install must not create .gemini/settings.json"
    );
}

#[test]
fn test_hooks_codex_idempotent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let hooks_file = tmp.path().join(".codex").join("hooks.json");
    let path = hooks_file.to_str().unwrap();

    install_codex_hooks_pub(path).unwrap();
    install_codex_hooks_pub(path).unwrap();

    let content = std::fs::read_to_string(&hooks_file).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(
        settings["hooks"]["Stop"].as_array().unwrap().len(),
        1,
        "Stop must have exactly 1 entry after double install"
    );

    let toml_content = std::fs::read_to_string(tmp.path().join(".codex/config.toml")).unwrap();
    assert_eq!(
        toml_content.matches("codex_hooks").count(),
        1,
        "codex_hooks must appear exactly once after double install"
    );
}

/// Regression test: a real project (itunes-app-scraper) had a Codex agent
/// whose .codex/hooks.json still contained the broken PostToolUse/Bash hook
/// from pre-v0.8.6. Running squad-station init did NOT remove it because
/// install_codex_hooks only set the Stop key without cleaning stale keys.
/// This test simulates that exact scenario.
#[test]
fn test_hooks_codex_removes_stale_post_tool_use_from_existing_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let codex_dir = tmp.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();

    // Simulate a pre-v0.8.6 hooks.json with the broken PostToolUse/Bash hook
    let stale_hooks = serde_json::json!({
        "hooks": {
            "Stop": [{
                "matcher": "",
                "hooks": [{"type": "command", "command": "squad-station signal old-path 2>/dev/null"}]
            }],
            "PostToolUse": [{
                "matcher": "Bash",
                "hooks": [{"type": "command", "command": "squad-station notify --body 'Agent needs input' --agent \"$(tmux display-message -p '#S' 2>/dev/null)\" 2>/dev/null"}]
            }],
            "Notification": [{
                "matcher": "permission_prompt",
                "hooks": [{"type": "command", "command": "squad-station notify --body 'old notification'"}]
            }]
        }
    });
    std::fs::write(
        codex_dir.join("hooks.json"),
        serde_json::to_string_pretty(&stale_hooks).unwrap(),
    )
    .unwrap();

    let hooks_file = codex_dir.join("hooks.json");
    install_codex_hooks_pub(hooks_file.to_str().unwrap()).unwrap();

    let content = std::fs::read_to_string(&hooks_file).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    // Stop hook must be updated (not the old command)
    let stop_cmd = settings["hooks"]["Stop"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(
        stop_cmd.contains("squad-station signal"),
        "Stop hook must be updated: {stop_cmd}"
    );
    assert!(
        !stop_cmd.contains("old-path"),
        "Old Stop command must be replaced: {stop_cmd}"
    );

    // PostToolUse must be REMOVED (not just left as-is)
    assert!(
        settings["hooks"]["PostToolUse"].is_null(),
        "Stale PostToolUse hook must be removed from Codex hooks.json, got: {}",
        serde_json::to_string_pretty(&settings["hooks"]).unwrap()
    );

    // Notification must be REMOVED
    assert!(
        settings["hooks"]["Notification"].is_null(),
        "Stale Notification hook must be removed from Codex hooks.json, got: {}",
        serde_json::to_string_pretty(&settings["hooks"]).unwrap()
    );
}

#[test]
fn test_hooks_codex_no_env_var_session_name() {
    let tmp = tempfile::TempDir::new().unwrap();
    let hooks_file = tmp.path().join(".codex").join("hooks.json");
    install_codex_hooks_pub(hooks_file.to_str().unwrap()).unwrap();

    let content = std::fs::read_to_string(&hooks_file).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    let stop_cmd = settings["hooks"]["Stop"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(
        !stop_cmd.contains("SQUAD_AGENT_NAME"),
        "Codex must NOT use $SQUAD_AGENT_NAME: {stop_cmd}"
    );
    assert!(
        !stop_cmd.contains("TMUX_PANE"),
        "Codex must NOT use $TMUX_PANE: {stop_cmd}"
    );
}

// ============================================================
// Gemini CLI provider — .gemini/settings.json
// ============================================================

#[test]
fn test_hooks_gemini_full_structure() {
    let tmp = tempfile::TempDir::new().unwrap();
    let settings_file = tmp.path().join(".gemini").join("settings.json");
    let settings_str = settings_file.to_str().unwrap();

    // Create parent dir (gemini install function expects it or creates it)
    assert!(install_gemini_hooks_pub(settings_str).unwrap());

    let content = std::fs::read_to_string(&settings_file).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    // AfterAgent hook (NOT Stop)
    assert!(
        settings["hooks"]["AfterAgent"].is_array(),
        "Gemini must have AfterAgent hook"
    );
    assert!(
        settings["hooks"]["Stop"].is_null(),
        "Gemini must NOT have Stop hook (uses AfterAgent instead)"
    );

    let signal_cmd = settings["hooks"]["AfterAgent"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(
        signal_cmd.contains("squad-station signal"),
        "AfterAgent must call squad-station signal: {signal_cmd}"
    );
    assert!(
        signal_cmd.contains("tmux display-message"),
        "AfterAgent must use tmux display-message: {signal_cmd}"
    );

    // Gemini hooks MUST output valid JSON
    assert!(
        signal_cmd.contains("printf '{}'"),
        "Gemini hook MUST include printf for JSON stdout: {signal_cmd}"
    );
    assert!(
        signal_cmd.contains(">/dev/null 2>&1"),
        "Gemini must redirect output to /dev/null: {signal_cmd}"
    );

    // Must have name, description, and timeout fields
    let hook_obj = &settings["hooks"]["AfterAgent"][0]["hooks"][0];
    assert_eq!(hook_obj["name"].as_str().unwrap(), "squad-signal");
    assert!(hook_obj["description"].is_string());
    assert_eq!(hook_obj["timeout"].as_u64().unwrap(), 30000);

    // Notification hook
    let notif = &settings["hooks"]["Notification"];
    assert!(notif.is_array(), "Gemini must have Notification hook");
    let notif_cmd = notif[0]["hooks"][0]["command"].as_str().unwrap();
    assert!(notif_cmd.contains("printf '{}'"));
    assert!(notif_cmd.contains("squad-station notify"));

    // SessionStart must NOT be auto-installed
    assert!(settings["hooks"]["SessionStart"].is_null());
}

#[test]
fn test_hooks_gemini_does_not_create_claude_or_codex_files() {
    let tmp = tempfile::TempDir::new().unwrap();
    let settings_file = tmp.path().join(".gemini").join("settings.json");
    install_gemini_hooks_pub(settings_file.to_str().unwrap()).unwrap();

    assert!(!tmp.path().join(".claude/settings.json").exists());
    assert!(!tmp.path().join(".codex/hooks.json").exists());
}

#[test]
fn test_hooks_gemini_no_env_var_session_name() {
    let tmp = tempfile::TempDir::new().unwrap();
    let settings_file = tmp.path().join(".gemini").join("settings.json");
    install_gemini_hooks_pub(settings_file.to_str().unwrap()).unwrap();

    let content = std::fs::read_to_string(&settings_file).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    let signal_cmd = settings["hooks"]["AfterAgent"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert!(
        !signal_cmd.contains("SQUAD_AGENT_NAME"),
        "Gemini must NOT use $SQUAD_AGENT_NAME: {signal_cmd}"
    );
    assert!(
        !signal_cmd.contains("TMUX_PANE"),
        "Gemini must NOT use $TMUX_PANE: {signal_cmd}"
    );
}

// ============================================================
// Mixed-provider — verifies per-provider hook isolation
// ============================================================

#[test]
fn test_hooks_mixed_providers_isolation() {
    let tmp = tempfile::TempDir::new().unwrap();

    install_gemini_hooks_pub(tmp.path().join(".gemini/settings.json").to_str().unwrap()).unwrap();
    install_claude_hooks_pub(tmp.path().join(".claude/settings.json").to_str().unwrap()).unwrap();
    install_codex_hooks_pub(tmp.path().join(".codex/hooks.json").to_str().unwrap()).unwrap();

    // All three provider hook files must exist
    assert!(tmp.path().join(".gemini/settings.json").exists());
    assert!(tmp.path().join(".claude/settings.json").exists());
    assert!(tmp.path().join(".codex/hooks.json").exists());

    // Gemini uses AfterAgent (not Stop)
    let gemini: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(tmp.path().join(".gemini/settings.json")).unwrap(),
    )
    .unwrap();
    assert!(gemini["hooks"]["AfterAgent"].is_array());
    assert!(gemini["hooks"]["Stop"].is_null());

    // Claude Code uses Stop (not AfterAgent) and has PostToolUse
    let claude: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(tmp.path().join(".claude/settings.json")).unwrap(),
    )
    .unwrap();
    assert!(claude["hooks"]["Stop"].is_array());
    assert!(claude["hooks"]["PostToolUse"].is_array());
    assert!(claude["hooks"]["AfterAgent"].is_null());

    // Codex uses Stop and does NOT have PostToolUse or Notification
    let codex: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(tmp.path().join(".codex/hooks.json")).unwrap(),
    )
    .unwrap();
    assert!(codex["hooks"]["Stop"].is_array());
    assert!(
        codex["hooks"]["PostToolUse"].is_null(),
        "Codex must NOT have PostToolUse in mixed-provider setup"
    );
    assert!(
        codex["hooks"]["Notification"].is_null(),
        "Codex must NOT have Notification in mixed-provider setup"
    );
}

// ============================================================
// SessionStart hook — opt-in only, per provider
// ============================================================

#[test]
fn test_session_start_hook_codex_installs_correctly() {
    let tmp = tempfile::TempDir::new().unwrap();
    let codex_dir = tmp.path().join(".codex");
    std::fs::create_dir_all(&codex_dir).unwrap();
    std::fs::write(codex_dir.join("hooks.json"), r#"{"hooks":{"Stop":[]}}"#).unwrap();

    assert!(install_session_start_hook_pub("codex", tmp.path()).unwrap());

    let content = std::fs::read_to_string(codex_dir.join("hooks.json")).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    let ss = &settings["hooks"]["SessionStart"];
    assert!(ss.is_array(), "SessionStart hook must exist for codex");
    let ss_cmd = ss[0]["hooks"][0]["command"].as_str().unwrap();
    assert_eq!(ss_cmd, "squad-station context --inject");
    assert!(settings["hooks"]["Stop"].is_array());
}

#[test]
fn test_session_start_hook_all_providers() {
    let providers = vec![
        ("claude-code", ".claude/settings.json"),
        ("codex", ".codex/hooks.json"),
        ("gemini-cli", ".gemini/settings.json"),
    ];

    for (provider, hook_file) in providers {
        let tmp = tempfile::TempDir::new().unwrap();
        let full_path = tmp.path().join(hook_file);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&full_path, r#"{"hooks":{}}"#).unwrap();

        let result = install_session_start_hook_pub(provider, tmp.path());
        assert!(result.unwrap(), "{provider}: SessionStart must install");

        let content = std::fs::read_to_string(&full_path).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        let ss_cmd = settings["hooks"]["SessionStart"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert_eq!(ss_cmd, "squad-station context --inject");
    }
}

#[test]
fn test_session_start_hook_unknown_provider_returns_false() {
    let tmp = tempfile::TempDir::new().unwrap();
    assert!(!install_session_start_hook_pub("unknown-tool", tmp.path()).unwrap());
}

// ============================================================
// Telegram hooks — install_telegram_hooks_pub (Rust subcommand)
// ============================================================

#[test]
fn test_telegram_hooks_claude_code_uses_rust_command() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path();

    let claude_dir = project_root.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        r#"{"hooks":{"Stop":[{"matcher":"","hooks":[{"type":"command","command":"squad-station signal"}]}]}}"#,
    ).unwrap();

    let tg = config::TelegramConfig {
        enabled: true,
        notify_agents: config::NotifyAgents::All("all".to_string()),
    };
    install_telegram_hooks_pub(&tg, project_root, &["claude-code".to_string()]).unwrap();

    // No shell script or telegram.env — pure Rust subcommand
    assert!(!project_root
        .join(".squad/hooks/notify-telegram.sh")
        .exists());
    assert!(!project_root.join(".squad/telegram.env").exists());

    let content = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();
    let stop = settings["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(stop.len(), 2);
    let tg_cmd = stop[1]["hooks"][0]["command"].as_str().unwrap();
    assert!(
        tg_cmd.contains("squad-station notify-telegram"),
        "Hook must call Rust subcommand: {tg_cmd}"
    );
    assert!(
        !tg_cmd.contains("--event"),
        "Hook must not pass --event: {tg_cmd}"
    );
}

#[test]
fn test_telegram_hooks_appends_to_stop_event() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path();

    let claude_dir = project_root.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        r#"{"hooks":{"Stop":[{"matcher":"","hooks":[{"type":"command","command":"squad-station signal"}]}]}}"#,
    ).unwrap();

    let tg = config::TelegramConfig {
        enabled: true,
        notify_agents: config::NotifyAgents::All("all".to_string()),
    };
    install_telegram_hooks_pub(&tg, project_root, &["claude-code".to_string()]).unwrap();

    let content = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    let stop = settings["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(stop.len(), 2, "Stop must have signal + telegram entries");

    let second_cmd = stop[1]["hooks"][0]["command"].as_str().unwrap();
    assert!(
        second_cmd.contains("squad-station notify-telegram"),
        "Second Stop entry must be telegram: {second_cmd}"
    );
}

#[test]
fn test_telegram_hooks_gemini_format() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path();

    let gemini_dir = project_root.join(".gemini");
    std::fs::create_dir_all(&gemini_dir).unwrap();
    std::fs::write(
        gemini_dir.join("settings.json"),
        r#"{"hooks":{"AfterAgent":[{"matcher":"","hooks":[{"type":"command","command":"squad-station signal"}]}]}}"#,
    ).unwrap();

    let tg = config::TelegramConfig {
        enabled: true,
        notify_agents: config::NotifyAgents::All("all".to_string()),
    };
    install_telegram_hooks_pub(&tg, project_root, &["gemini-cli".to_string()]).unwrap();

    let content = std::fs::read_to_string(gemini_dir.join("settings.json")).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    let after = settings["hooks"]["AfterAgent"].as_array().unwrap();
    assert_eq!(after.len(), 2);

    let tg_cmd = after[1]["hooks"][0]["command"].as_str().unwrap();
    assert!(
        tg_cmd.contains("squad-station notify-telegram"),
        "Gemini telegram hook must use Rust subcommand: {tg_cmd}"
    );
    assert!(
        !tg_cmd.contains("--event"),
        "Gemini hook must not pass --event: {tg_cmd}"
    );
    assert!(
        tg_cmd.contains("printf '{}'"),
        "Gemini telegram hook must include printf: {tg_cmd}"
    );
    assert_eq!(
        after[1]["hooks"][0]["name"].as_str().unwrap(),
        "squad-telegram"
    );
}

#[test]
fn test_telegram_hooks_idempotent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let project_root = tmp.path();

    let claude_dir = project_root.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        r#"{"hooks":{"Stop":[{"matcher":"","hooks":[{"type":"command","command":"squad-station signal"}]}]}}"#,
    ).unwrap();

    let tg = config::TelegramConfig {
        enabled: true,
        notify_agents: config::NotifyAgents::All("all".to_string()),
    };

    install_telegram_hooks_pub(&tg, project_root, &["claude-code".to_string()]).unwrap();
    install_telegram_hooks_pub(&tg, project_root, &["claude-code".to_string()]).unwrap();

    let content = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    let stop = settings["hooks"]["Stop"].as_array().unwrap();
    assert_eq!(
        stop.len(),
        2,
        "Stop must still have exactly 2 entries after double install"
    );
}

#[test]
fn test_telegram_hooks_absolute_path_even_for_relative_root() {
    let tmp = tempfile::TempDir::new().unwrap();
    let abs_root = std::fs::canonicalize(tmp.path()).unwrap();

    let claude_dir = abs_root.join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        r#"{"hooks":{"Stop":[{"matcher":"","hooks":[{"type":"command","command":"squad-station signal"}]}]}}"#,
    ).unwrap();

    let tg = config::TelegramConfig {
        enabled: true,
        notify_agents: config::NotifyAgents::All("all".to_string()),
    };

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(&abs_root).unwrap();
    let result =
        install_telegram_hooks_pub(&tg, std::path::Path::new("."), &["claude-code".to_string()]);
    std::env::set_current_dir(&original_dir).unwrap();
    result.unwrap();

    let content = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();
    let stop = settings["hooks"]["Stop"].as_array().unwrap();
    let tg_cmd = stop[1]["hooks"][0]["command"].as_str().unwrap();

    assert!(
        !tg_cmd.contains("--project-root \".\""),
        "hook must not use relative path '.': {tg_cmd}"
    );
    assert!(
        tg_cmd.contains(&format!("--project-root \"{}\"", abs_root.display())),
        "hook must use absolute --project-root: {tg_cmd}"
    );
    assert!(!tg_cmd.contains("cd "), "hook must not use cd: {tg_cmd}");
}

// ============================================================
// Rust notify_telegram command — unit tests via public helpers
// ============================================================

use squad_station::commands::notify_telegram::{
    agent_matches_filter_pub, format_message_pub, load_env_file_pub,
};

#[test]
fn test_notify_telegram_load_env_file() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), "# comment\nTELE_TOKEN=abc\nTELE_CHAT_ID=-100\n").unwrap();
    let vars = load_env_file_pub(tmp.path());
    assert_eq!(vars.len(), 2);
    assert_eq!(vars[0].0, "TELE_TOKEN");
    assert_eq!(vars[0].1, "abc");
}

#[test]
fn test_notify_telegram_agent_filter_all() {
    let filter = config::NotifyAgents::All("all".to_string());
    assert!(agent_matches_filter_pub("any-agent", &filter));
}

#[test]
fn test_notify_telegram_agent_filter_list() {
    let filter = config::NotifyAgents::List(vec!["orchestrator".into(), "implement".into()]);
    assert!(agent_matches_filter_pub("implement", &filter));
    assert!(agent_matches_filter_pub("myproject-orchestrator", &filter));
    assert!(!agent_matches_filter_pub("brainstorm", &filter));
}

#[test]
fn test_notify_telegram_format_message_with_agent() {
    let msg = format_message_pub("myproject", Some("myproject-orchestrator"), None);
    assert!(msg.contains("[myproject]"));
    assert!(msg.contains("myproject-orchestrator"));
    assert!(msg.contains("finished"));
}

#[test]
fn test_notify_telegram_format_message_without_agent() {
    let msg = format_message_pub("proj", None, None);
    assert!(msg.contains("[proj]"));
    assert!(msg.contains("Agent finished"));
}

#[test]
fn test_notify_telegram_format_message_with_transcript() {
    let msg = format_message_pub("proj", Some("worker"), Some("cargo test\nall passed"));
    assert!(msg.contains("[proj]"));
    assert!(msg.contains("<pre>"));
    assert!(msg.contains("cargo test"));
}

#[test]
fn test_notify_telegram_format_message_truncation() {
    let long = "x".repeat(5000);
    let msg = format_message_pub(&long, None, None);
    assert!(msg.contains("(truncated)"));
}

// ============================================================
// Unknown provider — must return false, not error
// ============================================================

#[test]
fn test_hooks_unknown_provider_returns_false() {
    let result = squad_station::commands::init::auto_install_hooks_pub("unknown-provider");
    assert!(!result.unwrap(), "Unknown provider must return Ok(false)");
}
