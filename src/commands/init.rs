use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use owo_colors::OwoColorize;
use owo_colors::Stream;

use crate::{config, db, tmux};

pub async fn run(config_path: PathBuf, json: bool) -> anyhow::Result<()> {
    // 1. Parse squad.yml
    let config = config::load_config(&config_path)?;

    // 2. Pre-flight: detect tmux session name conflicts from other projects.
    //    If squad.yml was copied without changing `project:`, the session names
    //    would collide with a running squad in a different directory.
    //    Skipped in JSON mode (programmatic/CI) and when SQUAD_STATION_DB is set (tests).
    if !json && std::env::var("SQUAD_STATION_DB").is_err() {
        check_session_conflicts(&config)?;
    }

    // 3. Resolve DB path
    let db_path = config::resolve_db_path(&config)?;

    // 4. Connect to DB (creates file + runs migrations)
    let pool = db::connect(&db_path).await?;

    // 4. Register orchestrator with hardcoded role="orchestrator"
    let orch_role = config
        .orchestrator
        .name
        .as_deref()
        .unwrap_or("orchestrator");
    let orch_name = config::build_session_name(&config.project, orch_role);
    db::agents::insert_agent(
        &pool,
        &orch_name,
        &config.orchestrator.provider,
        "orchestrator",
        config.orchestrator.model.as_deref(),
        config.orchestrator.description.as_deref(),
    )
    .await?;

    // 5. Launch orchestrator tmux session
    let orch_launched = if tmux::session_exists(&orch_name) {
        false
    } else {
        // Orchestrator launches at project root.
        // Context loaded via /squad-orchestrator slash command.
        let project_root = db_path
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(std::path::Path::new("."));
        let project_root_str = project_root.to_string_lossy().to_string();
        let cmd = get_launch_command(&config.orchestrator);
        tmux::launch_agent_in_dir(&orch_name, &cmd, &project_root_str)?;
        true
    };
    let orch_skipped = !orch_launched;

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
        let agent_name = config::build_session_name(&config.project, role_suffix);
        if let Err(e) = db::agents::insert_agent(
            &pool,
            &agent_name,
            &agent.provider,
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

        // GAP-05: Workers launch at project root directory
        let project_root = db_path
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(std::path::Path::new("."));
        let project_root_str = project_root.to_string_lossy().to_string();
        let cmd = get_launch_command(agent);
        match tmux::launch_agent_in_dir(&agent_name, &cmd, &project_root_str) {
            Ok(()) => launched += 1,
            Err(e) => failed.push((agent_name.clone(), format!("{e:#}"))),
        }
    }

    // 7. Create monitor session with interactive panes for all agents
    let monitor_name = config::build_session_name(&config.project, "monitor");
    let mut monitor_sessions: Vec<String> = vec![];
    monitor_sessions.push(orch_name.clone());
    for agent in &config.agents {
        let role_suffix = agent.name.as_deref().unwrap_or(&agent.role);
        let agent_name = config::build_session_name(&config.project, role_suffix);
        monitor_sessions.push(agent_name);
    }
    // Kill existing monitor session before recreating
    tmux::kill_session(&monitor_name)?;
    let monitor_created = if !monitor_sessions.is_empty() {
        tmux::create_view_session(&monitor_name, &monitor_sessions).is_ok()
    } else {
        false
    };

    // 8. Output results
    let db_path_str = db_path.display().to_string();

    if json {
        let output = serde_json::json!({
            "launched": launched,
            "skipped": skipped,
            "failed": failed,
            "db_path": db_path_str,
            "monitor": if monitor_created { Some(&monitor_name) } else { None },
        });
        println!("{}", serde_json::to_string(&output)?);
    } else {
        let total_agents = config.agents.len() + 1; // workers + orchestrator
        println!(
            "Initialized squad '{}' with {} agent(s) ({} launched, {} skipped)",
            config.project, total_agents, launched, skipped
        );
        for name in &skipped_names {
            println!("  - {}: already running (skipped)", name);
        }
        for (name, error) in &failed {
            println!("  x {}: {}", name, error);
        }
        println!("  Database: {}", db_path_str);
    }

    // 8. Exit code: return Err only if ALL agents failed (including orchestrator)
    let total = config.agents.len() + 1;
    if !failed.is_empty() && failed.len() == total {
        anyhow::bail!("All {} agent(s) failed to launch", total);
    }

    // 9a. Create .squad/log/ directory for signal and watchdog logs
    {
        let log_dir = db_path
            .parent()
            .unwrap_or(std::path::Path::new(".squad"))
            .join("log");
        let _ = std::fs::create_dir_all(&log_dir);
    }

    // 9. Hook setup: auto-install for ALL providers used in the squad (not just orchestrator).
    // In JSON mode, skip stdout instructions (to preserve machine-parseable output).
    if !json {
        let green = |s: &str| {
            s.if_supports_color(Stream::Stdout, |s| s.green())
                .to_string()
        };
        let cyan = |s: &str| {
            s.if_supports_color(Stream::Stdout, |s| s.cyan())
                .to_string()
        };
        let yellow = |s: &str| {
            s.if_supports_color(Stream::Stdout, |s| s.yellow())
                .to_string()
        };
        let bold = |s: &str| {
            s.if_supports_color(Stream::Stdout, |s| s.bold())
                .to_string()
        };

        println!("\n{}", green("══════════════════════════════════"));
        println!("  {}", bold("Squad Setup Complete"));
        println!("{}\n", green("══════════════════════════════════"));

        // Collect all unique providers across orchestrator + workers
        let mut providers_seen: Vec<String> = vec![config.orchestrator.provider.clone()];
        for agent in &config.agents {
            if !providers_seen.contains(&agent.provider) {
                providers_seen.push(agent.provider.clone());
            }
        }

        let mut any_hooks_installed = false;
        for provider in &providers_seen {
            match auto_install_hooks(provider) {
                Ok(true) => {
                    any_hooks_installed = true;
                    println!("  Hooks: installed for {}", provider);
                }
                Ok(false) => {
                    println!("  Hooks: skipped for {} (unsupported provider)", provider);
                }
                Err(e) => {
                    println!("  Hooks: failed for {} ({})", provider, e);
                }
            }
        }

        if !any_hooks_installed {
            println!("Please manually configure the following hooks to enable task completion signals:\n");
            let hook_providers: &[(&str, &str, &str)] = &[
                (".claude/settings.json", "Stop", "*"),
                (".claude/settings.json", "Notification", "permission_prompt"),
                (".claude/settings.json", "PostToolUse", "AskUserQuestion"),
                (".gemini/settings.json", "AfterAgent", "*"),
                (".gemini/settings.json", "Notification", "*"),
            ];
            for &(settings_path, hook_event, matcher) in hook_providers {
                print_hook_instructions(settings_path, hook_event, matcher);
            }
        }

        let hook_installed = any_hooks_installed;

        // Ask user if they want auto-inject of orchestrator context on session start/compact/clear.
        // Only prompt when base hooks were successfully auto-installed (supported provider).
        if hook_installed {
            println!();
            println!(
                "  {}",
                bold("Remember orchestrator role across /clear and restarts?")
            );
            println!("  If enabled, the orchestrator keeps its context automatically.");
            println!(
                "  If disabled, you must run {} after each /clear.",
                yellow("/squad-orchestrator")
            );
            println!();
            print!("  Enable? [y/N] ");
            std::io::stdout().flush().ok();

            let mut answer = String::new();
            let accepted = std::io::stdin().read_line(&mut answer).is_ok()
                && answer.trim().eq_ignore_ascii_case("y");
            // Ensure result appears on a new line after the prompt
            if !answer.ends_with('\n') {
                println!();
            }
            if accepted {
                let project_root = config_path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(std::path::Path::new("."));
                match install_session_start_hook(&config.orchestrator.provider, project_root) {
                    Ok(true) => println!("  Context auto-inject: installed"),
                    Ok(false) => println!("  Context auto-inject: skipped (unsupported provider)"),
                    Err(e) => println!("  Context auto-inject: failed ({})", e),
                }
            } else {
                println!("  Context auto-inject: skipped");
            }
        }

        println!("\nGenerating orchestrator context...");
        if let Err(e) = crate::commands::context::run(false).await {
            println!("Warning: Failed to generate context files: {}", e);
        }

        // Inject orchestrator bootstrap block into provider project doc file.
        // This block survives /clear and context compact, ensuring the orchestrator
        // always knows its role without user intervention.
        {
            let project_root = config_path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."));
            match crate::commands::context::inject_bootstrap_block(
                project_root,
                &config.orchestrator.provider,
                &orch_name,
            ) {
                Ok(path) => println!("  Bootstrap: injected into {}", path),
                Err(e) => println!("  Bootstrap: failed ({})", e),
            }
        }

        // 9b. Install SDD git workflow rules for all providers
        if let Some(sdd_configs) = &config.sdd {
            let project_root = config_path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."));
            for sdd in sdd_configs {
                match install_sdd_rules(&sdd.name, project_root, &providers_seen) {
                    Ok(installed) if !installed.is_empty() => {
                        for dest in &installed {
                            println!("  SDD rule: installed {} → {}", sdd.name, dest);
                        }
                    }
                    Ok(_) => {
                        println!("  SDD rule: no rule file found for '{}' (looked for .squad/rules/git-workflow-{}.md)", sdd.name, sdd.name);
                    }
                    Err(e) => {
                        println!("  SDD rule: failed for '{}' ({})", sdd.name, e);
                    }
                }
            }
        }

        // 9c. Telegram notification hooks
        {
            let project_root = config_path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(std::path::Path::new("."));

            if config.is_telegram_enabled() {
                // Already configured in squad.yml → install hooks automatically
                ensure_gitignore_env_squad(project_root)?;

                // Check if .env.squad has actual credentials
                if !env_squad_has_credentials(project_root) && std::io::stdin().is_terminal() {
                    // Credentials missing — prompt interactively
                    println!();
                    println!("  Telegram is enabled but .env.squad has no credentials.");
                    prompt_telegram_credentials(project_root)?;
                } else {
                    ensure_env_squad(project_root)?;
                }

                if let Some(tg) = &config.telegram {
                    match install_telegram_hooks(tg, project_root, &providers_seen) {
                        Ok(()) => println!("  Telegram: enabled — you will receive notifications when agents finish tasks or need input"),
                        Err(e) => println!("  Telegram: failed ({})", e),
                    }
                }
            } else if std::io::stdin().is_terminal() {
                // No telegram section in squad.yml → prompt user
                println!();
                print!("  Enable Telegram notifications? [y/N] ");
                std::io::stdout().flush().ok();

                let mut answer = String::new();
                let accepted = std::io::stdin().read_line(&mut answer).is_ok()
                    && answer.trim().eq_ignore_ascii_case("y");
                if !answer.ends_with('\n') {
                    println!();
                }
                if accepted {
                    if prompt_telegram_credentials(project_root)? {
                        ensure_gitignore_env_squad(project_root)?;
                        append_telegram_to_squad_yml(&config_path)?;

                        let tg = config::TelegramConfig {
                            enabled: true,
                            notify_agents: config::NotifyAgents::All("all".to_string()),
                        };
                        match install_telegram_hooks(&tg, project_root, &providers_seen) {
                            Ok(()) => println!("  Telegram: enabled — you will receive notifications when agents finish tasks or need input"),
                            Err(e) => println!("  Telegram: failed ({})", e),
                        }
                    }
                } else {
                    println!("  Telegram: skipped");
                }
            }
        }

        println!("\n{}", bold("Get Started:"));
        println!();
        println!("  1. Attach to the orchestrator session:");
        println!("     {}", cyan(&format!("tmux attach -t {}", orch_name)));
        println!();
        println!("  2. Load the orchestrator context by typing:");
        println!("     {}", yellow("/squad-orchestrator"));
        if monitor_created {
            println!();
            println!("  Monitor all agents (interactive panes):");
            println!("     {}", cyan(&format!("tmux attach -t {}", monitor_name)));
        }
        println!();
        println!("  Monitor all agents (read-only view):");
        println!("     {}", cyan("squad-station view"));
        if config.is_telegram_enabled() {
            println!();
            println!("  Telegram notifications: active (configure agents in squad.yml)");
        }
        println!();

        // Auto-start watchdog daemon for self-healing
        match crate::commands::watch::run(30, 5, true, false).await {
            Ok(()) => println!("  Watchdog: started (30s interval)"),
            Err(e) => {
                let msg = format!("{}", e);
                if msg.contains("already running") {
                    println!("  Watchdog: already running");
                } else {
                    println!("  Watchdog: failed to start ({})", e);
                }
            }
        }
        println!();

        // 10. Post-init health check
        run_health_check(&config, &db_path, &orch_name);
    }

    Ok(())
}

/// Post-init health check: validate all critical components are properly configured.
/// Prints a clear pass/fail summary with actionable remediation steps.
/// Returns the number of failed checks.
pub fn run_health_check(
    config: &config::SquadConfig,
    db_path: &std::path::Path,
    orch_name: &str,
) -> u32 {
    let green = |s: &str| {
        s.if_supports_color(Stream::Stdout, |s| s.green())
            .to_string()
    };
    let red = |s: &str| s.if_supports_color(Stream::Stdout, |s| s.red()).to_string();
    let yellow = |s: &str| {
        s.if_supports_color(Stream::Stdout, |s| s.yellow())
            .to_string()
    };
    let bold = |s: &str| {
        s.if_supports_color(Stream::Stdout, |s| s.bold())
            .to_string()
    };

    let pass = green("PASS");
    let fail = red("FAIL");
    let warn = yellow("WARN");

    println!("{}", bold("Health Check"));
    println!("{}\n", bold("──────────────────────────────────"));

    let mut pass_count: u32 = 0;
    let mut fail_count: u32 = 0;
    let mut warn_count: u32 = 0;
    let mut remediation: Vec<String> = vec![];

    // 1. Database accessible
    if db_path.exists() {
        println!("  {} Database exists: {}", pass, db_path.display());
        pass_count += 1;
    } else {
        println!("  {} Database missing: {}", fail, db_path.display());
        fail_count += 1;
        remediation.push("Database file was not created. Re-run `squad-station init`.".into());
    }

    // 2. Log directory
    let log_dir = db_path
        .parent()
        .unwrap_or(std::path::Path::new(".squad"))
        .join("log");
    if log_dir.exists() {
        println!("  {} Log directory: {}", pass, log_dir.display());
        pass_count += 1;
    } else {
        println!("  {} Log directory missing: {}", fail, log_dir.display());
        fail_count += 1;
        remediation.push(format!(
            "Create log directory: mkdir -p {}",
            log_dir.display()
        ));
    }

    // 3. Hooks config files — check each provider used
    let mut providers_seen: Vec<String> = vec![config.orchestrator.provider.clone()];
    for agent in &config.agents {
        if !providers_seen.contains(&agent.provider) {
            providers_seen.push(agent.provider.clone());
        }
    }

    for provider in &providers_seen {
        let settings_path = match provider.as_str() {
            "claude-code" => Some(".claude/settings.json"),
            "codex" => Some(".codex/hooks.json"),
            "gemini-cli" => Some(".gemini/settings.json"),
            _ => None,
        };

        if let Some(path) = settings_path {
            let full_path = std::path::Path::new(path);
            if full_path.exists() {
                // Verify it contains squad-station hooks
                match std::fs::read_to_string(full_path) {
                    Ok(content) => {
                        if content.contains("squad-station signal") {
                            println!("  {} Hooks ({}) — signal hook present", pass, provider);
                            pass_count += 1;
                        } else {
                            println!(
                                "  {} Hooks ({}) — {} exists but missing signal hook",
                                fail, provider, path
                            );
                            fail_count += 1;
                            remediation.push(format!(
                                "Re-install hooks: delete {} and re-run `squad-station init`",
                                path
                            ));
                        }

                        if content.contains("squad-station notify") {
                            println!("  {} Hooks ({}) — notify hook present", pass, provider);
                            pass_count += 1;
                        } else {
                            println!(
                                "  {} Hooks ({}) — {} exists but missing notify hook",
                                fail, provider, path
                            );
                            fail_count += 1;
                            remediation.push(format!(
                                "Re-install hooks: delete {} and re-run `squad-station init`",
                                path
                            ));
                        }
                    }
                    Err(e) => {
                        println!(
                            "  {} Hooks ({}) — cannot read {}: {}",
                            fail, provider, path, e
                        );
                        fail_count += 1;
                        remediation.push(format!("Fix file permissions on {}", path));
                    }
                }
            } else {
                println!("  {} Hooks ({}) — {} not found", fail, provider, path);
                fail_count += 1;
                remediation.push(format!(
                    "Hooks not injected for {}. Re-run `squad-station init` or manually create {}",
                    provider, path
                ));
            }
        } else {
            println!(
                "  {} Hooks ({}) — unsupported provider, manual setup required",
                warn, provider
            );
            warn_count += 1;
        }
    }

    // 4. Orchestrator context file
    let context_path = match config.orchestrator.provider.as_str() {
        "codex" => ".codex/commands/squad-orchestrator.md",
        "gemini-cli" => ".gemini/commands/squad-orchestrator.toml",
        _ => ".claude/commands/squad-orchestrator.md",
    };
    if std::path::Path::new(context_path).exists() {
        println!("  {} Orchestrator context: {}", pass, context_path);
        pass_count += 1;
    } else {
        println!("  {} Orchestrator context missing: {}", fail, context_path);
        fail_count += 1;
        remediation.push("Regenerate context: `squad-station context`".into());
    }

    // 5. Tmux sessions alive
    if tmux::session_exists(orch_name) {
        println!("  {} Orchestrator session: {}", pass, orch_name);
        pass_count += 1;
    } else {
        println!("  {} Orchestrator session not running: {}", fail, orch_name);
        fail_count += 1;
        remediation.push(format!(
            "Orchestrator tmux session '{}' is not running. Re-run `squad-station init`.",
            orch_name
        ));
    }

    for agent in &config.agents {
        let role_suffix = agent.name.as_deref().unwrap_or(&agent.role);
        let agent_name = config::build_session_name(&config.project, role_suffix);
        if tmux::session_exists(&agent_name) {
            println!("  {} Agent session: {}", pass, agent_name);
            pass_count += 1;
        } else {
            println!("  {} Agent session not running: {}", fail, agent_name);
            fail_count += 1;
            remediation.push(format!(
                "Agent tmux session '{}' is not running. Re-run `squad-station init`.",
                agent_name
            ));
        }
    }

    // 6. Watchdog running
    let pid_file = db_path
        .parent()
        .unwrap_or(std::path::Path::new(".squad"))
        .join("watch.pid");
    let watchdog_alive = if pid_file.exists() {
        // Check if the PID is actually running
        match std::fs::read_to_string(&pid_file) {
            Ok(pid_str) => {
                if let Ok(pid) = pid_str.trim().parse::<i32>() {
                    // kill -0 checks if process exists without sending a signal
                    unsafe { libc::kill(pid, 0) == 0 }
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    } else {
        false
    };

    if watchdog_alive {
        println!("  {} Watchdog daemon running", pass);
        pass_count += 1;
    } else {
        println!("  {} Watchdog daemon not running", warn);
        warn_count += 1;
        remediation.push("Watchdog not running. Start it: `squad-station watch --daemon`".into());
    }

    // 7. Telegram notifications (only if enabled)
    if config.is_telegram_enabled() {
        let script_path = db_path
            .parent()
            .unwrap_or(std::path::Path::new(".squad"))
            .join("hooks")
            .join("notify-telegram.sh");
        if script_path.exists() {
            println!("  {} Telegram script: {}", pass, script_path.display());
            pass_count += 1;
        } else {
            println!(
                "  {} Telegram script missing: {}",
                fail,
                script_path.display()
            );
            fail_count += 1;
            remediation.push("Re-run `squad-station init` to install Telegram hooks.".into());
        }

        // Check .env.squad for credentials (reuse existing helper)
        let project_root = db_path
            .parent()
            .and_then(|p| p.parent())
            .unwrap_or(std::path::Path::new("."));
        if env_squad_has_credentials(project_root) {
            println!("  {} Telegram credentials configured", pass);
            pass_count += 1;
        } else if project_root.join(".env.squad").exists() {
            println!(
                "  {} Telegram credentials incomplete — fill in .env.squad",
                warn
            );
            warn_count += 1;
        } else {
            println!(
                "  {} .env.squad missing — create it with TELE_TOKEN and TELE_CHAT_ID",
                warn
            );
            warn_count += 1;
        }

        // Check .env.squad in .gitignore
        let gitignore_path = project_root.join(".gitignore");
        let gitignore_content = std::fs::read_to_string(&gitignore_path).unwrap_or_default();
        if gitignore_content.lines().any(|l| l.trim() == ".env.squad") {
            println!("  {} .env.squad in .gitignore", pass);
            pass_count += 1;
        } else {
            println!(
                "  {} .env.squad NOT in .gitignore — credentials may be exposed",
                warn
            );
            warn_count += 1;
            remediation.push("Add `.env.squad` to .gitignore to protect credentials.".into());
        }
    }

    // Summary
    println!();
    if fail_count == 0 && warn_count == 0 {
        println!(
            "  {} All {} checks passed — squad is fully operational!",
            green("✓"),
            pass_count
        );
    } else if fail_count == 0 {
        println!(
            "  {} {}/{} passed, {} warning(s)",
            yellow("~"),
            pass_count,
            pass_count + warn_count,
            warn_count
        );
    } else {
        println!(
            "  {} {}/{} passed, {} failed, {} warning(s)",
            red("✗"),
            pass_count,
            pass_count + fail_count + warn_count,
            fail_count,
            warn_count
        );
        println!("\n  {}", bold("Remediation:"));
        for (i, step) in remediation.iter().enumerate() {
            println!("  {}. {}", i + 1, step);
        }
    }
    println!();

    fail_count
}

/// Check if planned tmux session names already exist as live tmux sessions.
/// Simple name-based check — if session exists, it's a conflict.
fn check_session_conflicts(config: &config::SquadConfig) -> anyhow::Result<()> {
    let live = tmux::list_live_session_names();
    if live.is_empty() {
        return Ok(());
    }

    let mut planned: Vec<String> = vec![];
    let orch_suffix = config
        .orchestrator
        .name
        .as_deref()
        .unwrap_or("orchestrator");
    planned.push(config::build_session_name(&config.project, orch_suffix));
    for agent in &config.agents {
        let suffix = agent.name.as_deref().unwrap_or(&agent.role);
        planned.push(config::build_session_name(&config.project, suffix));
    }

    let conflicts: Vec<&String> = planned.iter().filter(|n| live.contains(n)).collect();

    if !conflicts.is_empty() {
        eprintln!();
        eprintln!("  ⚠️  Session name conflict — these tmux sessions are already running:");
        eprintln!();
        for name in &conflicts {
            eprintln!("       • {}", name);
        }
        eprintln!();
        eprintln!("  Likely cause: squad.yml was copied without changing the `project:` field.");
        eprintln!("  Fix: update `project:` in squad.yml to a unique name, then re-run `squad-station init`.");
        eprintln!();
        anyhow::bail!("Aborting init — session name conflict");
    }

    Ok(())
}

fn auto_install_hooks(provider: &str) -> anyhow::Result<bool> {
    match provider {
        "claude-code" => install_claude_hooks(".claude/settings.json"),
        "codex" => install_codex_hooks(".codex/hooks.json"),
        "gemini-cli" => install_gemini_hooks(".gemini/settings.json"),
        _ => Ok(false), // unknown provider: skip auto-install
    }
}

/// Read or create a settings JSON file, returning the parsed value.
/// Creates a .bak backup if the file already exists.
fn read_or_create_settings(settings_file: &str) -> anyhow::Result<serde_json::Value> {
    let settings_path = std::path::Path::new(settings_file);

    if let Some(parent) = settings_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match std::fs::read_to_string(settings_path) {
        Ok(content) => {
            std::fs::write(settings_path.with_extension("json.bak"), &content)?;
            match serde_json::from_str(&content) {
                Ok(v) => Ok(v),
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse {}: {}. Starting fresh.",
                        settings_file, e
                    );
                    Ok(serde_json::json!({}))
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(serde_json::json!({})),
        Err(e) => Err(e.into()),
    }
}

/// Returns a shell command substitution that resolves the current tmux session name.
/// Produces `$(tmux display-message -p '#S' 2>/dev/null)` — a server-side query that
/// works reliably in all hook contexts (Claude Code Stop hooks, Gemini CLI AfterAgent,
/// tmux run-shell). If the command fails (e.g. running outside tmux), expands to an
/// empty string which signal.rs GUARD-1 handles with logging.
fn agent_name_subshell() -> &'static str {
    r#"$(tmux display-message -p '#S' 2>/dev/null)"#
}

/// Install Claude Code hooks: Stop (signal) + Notification (notify) + PostToolUse (AskUserQuestion)
fn install_claude_hooks(settings_file: &str) -> anyhow::Result<bool> {
    let mut settings = read_or_create_settings(settings_file)?;
    let resolve = agent_name_subshell();

    // Claude Code: stdout is ignored, stderr goes to /dev/null. Always exit 0.
    // signal.rs handles its own logging internally via log_signal() — no shell redirect needed.
    // Previous approach used `2>>.squad/log/signal.log` which broke when CWD != project root.
    let signal_cmd = format!(r#"squad-station signal "{}" 2>/dev/null"#, resolve);
    let notify_cmd = format!(
        r#"squad-station notify --body 'Agent needs input' --agent "{}" 2>/dev/null"#,
        resolve
    );

    // Stop hook — agent finished task → signal completion
    settings["hooks"]["Stop"] = serde_json::json!([{
        "matcher": "",
        "hooks": [{"type": "command", "command": signal_cmd}]
    }]);

    // Notification hook — agent needs permission approval → notify orchestrator
    // Only permission_prompt triggers notify. idle_prompt must NOT trigger notify
    // because idle = agent finished and is waiting for next task, which causes a
    // notification loop: idle → notify orchestrator → orchestrator sends task → idle → notify...
    settings["hooks"]["Notification"] = serde_json::json!([
        {
            "matcher": "permission_prompt",
            "hooks": [{"type": "command", "command": notify_cmd}]
        },
        {
            "matcher": "elicitation_dialog",
            "hooks": [{"type": "command", "command": notify_cmd}]
        }
    ]);

    // PostToolUse hook — agent is asking the user a question → notify orchestrator.
    // Orchestrator reads the actual question via capture-pane.
    settings["hooks"]["PostToolUse"] = serde_json::json!([
        {
            "matcher": "AskUserQuestion",
            "hooks": [{"type": "command", "command": notify_cmd}]
        }
    ]);

    std::fs::write(settings_file, serde_json::to_string_pretty(&settings)?)?;
    Ok(true)
}

/// Install Codex hooks: Stop (signal) + PostToolUse (notify for Bash tool)
///
/// Codex hooks are very similar to Claude Code:
/// - Uses Stop event for completion signals (same as Claude Code)
/// - Stdout is not required to be JSON (exit 0 with no output = success)
/// - Hooks configured in `.codex/hooks.json` (not settings.json)
/// - Matcher patterns use regex (e.g. "Bash", "startup|resume")
fn install_codex_hooks(settings_file: &str) -> anyhow::Result<bool> {
    let mut settings = read_or_create_settings(settings_file)?;
    let resolve = agent_name_subshell();

    // Codex: stdout is not required to be JSON. exit 0 = success. Same as Claude Code.
    let signal_cmd = format!(r#"squad-station signal "{}" 2>/dev/null"#, resolve);
    let notify_cmd = format!(
        r#"squad-station notify --body 'Agent needs input' --agent "{}" 2>/dev/null"#,
        resolve
    );

    // Stop hook — agent finished turn → signal completion
    settings["hooks"]["Stop"] = serde_json::json!([{
        "matcher": "",
        "hooks": [{"type": "command", "command": signal_cmd}]
    }]);

    // PostToolUse hook — notify orchestrator when Bash tool runs
    // (Codex currently only supports Bash tool for PreToolUse/PostToolUse)
    settings["hooks"]["PostToolUse"] = serde_json::json!([
        {
            "matcher": "Bash",
            "hooks": [{"type": "command", "command": notify_cmd}]
        }
    ]);

    std::fs::write(settings_file, serde_json::to_string_pretty(&settings)?)?;

    // Codex requires [features] codex_hooks = true in config.toml to activate hooks.
    // Without this, Codex ignores hooks.json entirely.
    ensure_codex_feature_flag(settings_file)?;

    Ok(true)
}

/// Ensure `.codex/config.toml` has `[features] codex_hooks = true`.
/// Derives the config.toml path from the hooks.json path (sibling file).
fn ensure_codex_feature_flag(hooks_file: &str) -> anyhow::Result<()> {
    let hooks_path = std::path::Path::new(hooks_file);
    let config_toml = hooks_path
        .parent()
        .unwrap_or(std::path::Path::new(".codex"))
        .join("config.toml");

    let content = std::fs::read_to_string(&config_toml).unwrap_or_default();

    // Already enabled — nothing to do
    if content.contains("codex_hooks") {
        return Ok(());
    }

    // Append the feature flag (preserve existing content)
    let mut new_content = content.clone();
    if !new_content.is_empty() && !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    if !new_content.contains("[features]") {
        new_content.push_str("\n[features]\n");
    }
    new_content.push_str("codex_hooks = true\n");

    std::fs::write(&config_toml, new_content)?;
    Ok(())
}

/// Install Gemini CLI hooks: AfterAgent (signal) + Notification (notify)
///
/// Critical Gemini CLI differences:
/// - Uses AfterAgent (not Stop) for completion signals
/// - Stdout MUST be valid JSON (golden rule) — all signal output goes to log file
/// - printf '{}' outputs empty JSON object = "continue normally"
/// - Uses ${AGENT:-__none__} to avoid shell short-circuit skipping printf
fn install_gemini_hooks(settings_file: &str) -> anyhow::Result<bool> {
    let mut settings = read_or_create_settings(settings_file)?;
    let resolve = agent_name_subshell();

    // Gemini CLI: ALL signal output redirected to /dev/null. stdout MUST be valid JSON.
    // signal.rs handles its own logging internally — shell redirect only suppresses output.
    // Previous approach used `>>.squad/log/signal.log` which broke when CWD != project root.
    let signal_cmd = format!(
        r#"squad-station signal "{}" >/dev/null 2>&1; printf '{{}}'"#,
        resolve
    );
    let notify_cmd = format!(
        r#"squad-station notify --body 'Agent needs input' --agent "{}" >/dev/null 2>&1; printf '{{}}'"#,
        resolve
    );

    settings["hooks"]["AfterAgent"] = serde_json::json!([{
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": signal_cmd,
            "name": "squad-signal",
            "description": "Signal task completion to squad-station",
            "timeout": 30000
        }]
    }]);

    settings["hooks"]["Notification"] = serde_json::json!([{
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": notify_cmd,
            "name": "squad-notify",
            "description": "Forward permission prompt to orchestrator",
            "timeout": 30000
        }]
    }]);

    std::fs::write(settings_file, serde_json::to_string_pretty(&settings)?)?;
    Ok(true)
}

/// Install SessionStart hook for auto-injecting orchestrator context.
/// Called separately from base hooks because it requires user opt-in.
fn install_session_start_hook(
    provider: &str,
    project_root: &std::path::Path,
) -> anyhow::Result<bool> {
    let rel_path = match provider {
        "claude-code" => ".claude/settings.json",
        "codex" => ".codex/hooks.json",
        "gemini-cli" => ".gemini/settings.json",
        _ => return Ok(false),
    };

    let settings_path = project_root.join(rel_path);
    let settings_str = settings_path.to_string_lossy();
    let mut settings = read_or_create_settings(&settings_str)?;
    let inject_cmd = "squad-station context --inject";

    settings["hooks"]["SessionStart"] = serde_json::json!([{
        "matcher": "",
        "hooks": [{"type": "command", "command": inject_cmd}]
    }]);

    std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;
    Ok(true)
}

/// Validate that a model string is safe for use as a CLI argument.
/// Only allows alphanumeric characters, dots, dashes, underscores, and colons.
fn is_safe_model_value(model: &str) -> bool {
    !model.is_empty()
        && model
            .chars()
            .all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_' || c == ':')
}

/// Build the launch command for a tmux session based on provider and model.
/// Claude Code: `claude --dangerously-skip-permissions --model <model>`
/// Gemini CLI: `gemini -y --model <model>`
/// Unknown/no model: plain `zsh` shell
fn get_launch_command(agent: &config::AgentConfig) -> String {
    match agent.provider.as_str() {
        "claude-code" => {
            let mut cmd = "claude --dangerously-skip-permissions".to_string();
            if let Some(model) = &agent.model {
                if is_safe_model_value(model) {
                    cmd.push_str(&format!(" --model {}", model));
                } else {
                    eprintln!(
                        "squad-station: warning: skipping unsafe model value: {:?}",
                        model
                    );
                }
            }
            cmd
        }
        "codex" => {
            let mut cmd = "codex --yolo".to_string();
            if let Some(model) = &agent.model {
                if is_safe_model_value(model) {
                    cmd.push_str(&format!(" --model {}", model));
                } else {
                    eprintln!(
                        "squad-station: warning: skipping unsafe model value: {:?}",
                        model
                    );
                }
            }
            cmd
        }
        "gemini-cli" => {
            let mut cmd = "gemini -y".to_string();
            if let Some(model) = &agent.model {
                if is_safe_model_value(model) {
                    cmd.push_str(&format!(" --model {}", model));
                } else {
                    eprintln!(
                        "squad-station: warning: skipping unsafe model value: {:?}",
                        model
                    );
                }
            }
            cmd
        }
        _ => "zsh".to_string(), // Unknown provider: open shell, user launches manually
    }
}

// ── Telegram notification hooks ──────────────────────────────────────────────

/// The embedded notify-telegram.sh script, compiled into the binary.
const NOTIFY_TELEGRAM_SH: &str = include_str!("../hooks/notify-telegram.sh");

/// Install Telegram notification hooks for all providers.
/// Writes the notify script, generates .squad/telegram.env, and appends hooks.
fn install_telegram_hooks(
    telegram: &config::TelegramConfig,
    project_root: &std::path::Path,
    providers: &[String],
) -> anyhow::Result<()> {
    // 1. Write notify-telegram.sh to .squad/hooks/
    let hooks_dir = project_root.join(".squad").join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;
    let script_path = hooks_dir.join("notify-telegram.sh");
    std::fs::write(&script_path, NOTIFY_TELEGRAM_SH)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))?;
    }

    // 2. Generate .squad/telegram.env (non-sensitive config from squad.yml)
    let env_content = format!(
        "# Auto-generated by squad-station init — source of truth: squad.yml\nTELE_NOTIFY_AGENTS={}\n",
        telegram.notify_agents.to_env_value()
    );
    std::fs::write(
        project_root.join(".squad").join("telegram.env"),
        env_content,
    )?;

    // 3. Append telegram hook entries to each provider's settings
    let project_root_str = project_root.to_string_lossy();
    let script_abs = format!("{project_root_str}/.squad/hooks/notify-telegram.sh");

    for provider in providers {
        let (settings_file, completion_event) = match provider.as_str() {
            "claude-code" => (".claude/settings.json", "Stop"),
            "codex" => (".codex/hooks.json", "Stop"),
            "gemini-cli" => (".gemini/settings.json", "AfterAgent"),
            _ => continue,
        };

        let settings_path = project_root.join(settings_file);
        if !settings_path.exists() {
            continue; // base hooks not installed yet — skip
        }

        let content = std::fs::read_to_string(&settings_path)?;
        let mut settings: serde_json::Value = serde_json::from_str(&content)?;

        // Build the hook command with SQUAD_PROJECT_ROOT env var
        let hook_cmd = if provider == "gemini-cli" {
            format!(
                r#"SQUAD_PROJECT_ROOT="{project_root_str}" "{script_abs}" >/dev/null 2>&1; printf '{{}}'"#
            )
        } else {
            format!(r#"SQUAD_PROJECT_ROOT="{project_root_str}" "{script_abs}" 2>/dev/null; true"#)
        };

        // Events to hook with telegram notification
        let events = if provider == "gemini-cli" {
            vec![completion_event, "Notification"]
        } else {
            vec![completion_event, "Notification", "PostToolUse"]
        };

        for event in &events {
            append_telegram_hook_entry(&mut settings, event, &hook_cmd, provider == "gemini-cli")?;
        }

        std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;
    }

    Ok(())
}

/// Append a telegram hook entry to a specific event in the settings JSON.
/// Skips if a telegram hook is already present (idempotent).
fn append_telegram_hook_entry(
    settings: &mut serde_json::Value,
    event: &str,
    command: &str,
    is_gemini: bool,
) -> anyhow::Result<()> {
    let hooks_obj = settings
        .as_object_mut()
        .and_then(|o| o.get_mut("hooks"))
        .and_then(|h| h.as_object_mut());

    let hooks_obj = match hooks_obj {
        Some(h) => h,
        None => return Ok(()), // no hooks object — skip
    };

    let existing = hooks_obj
        .get(event)
        .and_then(|e| e.as_array())
        .cloned()
        .unwrap_or_default();

    // Check if telegram hook already present
    let already_present = existing.iter().any(|entry| {
        let hooks = entry.get("hooks").and_then(|h| h.as_array());
        hooks
            .map(|hooks| {
                hooks.iter().any(|h| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .map(|cmd| cmd.contains("notify-telegram.sh"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    });

    if already_present {
        return Ok(());
    }

    let new_entry = if is_gemini {
        serde_json::json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": command,
                "name": "squad-telegram",
                "description": "Send Telegram notification",
                "timeout": 30000
            }]
        })
    } else {
        serde_json::json!({
            "matcher": "",
            "hooks": [{"type": "command", "command": command}]
        })
    };

    let mut merged = existing;
    merged.push(new_entry);
    hooks_obj.insert(event.to_string(), serde_json::Value::Array(merged));

    Ok(())
}

/// Write .env.squad template if it doesn't exist. Called when telegram is enabled.
fn ensure_env_squad(project_root: &std::path::Path) -> anyhow::Result<()> {
    let env_path = project_root.join(".env.squad");
    if !env_path.exists() {
        std::fs::write(
            &env_path,
            "# Squad Station — Telegram credentials (do not commit to git)\n\
             TELE_TOKEN=\n\
             TELE_CHAT_ID=\n\
             TELE_TOPIC_ID=\n\
             # TELE_TOPIC_ID is optional — leave empty to skip.\n\
             # When set, messages target that topic/thread in a Telegram group.\n",
        )?;
    }
    Ok(())
}

/// Write .env.squad with actual credentials (from interactive prompt).
fn write_env_squad(
    project_root: &std::path::Path,
    token: &str,
    chat_id: &str,
    topic_id: &str,
) -> anyhow::Result<()> {
    let topic_line = if topic_id.is_empty() {
        "TELE_TOPIC_ID=".to_string()
    } else {
        format!("TELE_TOPIC_ID={}", topic_id)
    };
    std::fs::write(
        project_root.join(".env.squad"),
        format!(
            "# Squad Station — Telegram credentials (do not commit to git)\n\
             TELE_TOKEN={}\n\
             TELE_CHAT_ID={}\n\
             {}\n\
             # TELE_TOPIC_ID is optional — leave empty to skip.\n\
             # When set, messages target that topic/thread in a Telegram group.\n",
            token, chat_id, topic_line
        ),
    )?;
    Ok(())
}

/// Add .env.squad to .gitignore if not already present.
fn ensure_gitignore_env_squad(project_root: &std::path::Path) -> anyhow::Result<()> {
    let gitignore_path = project_root.join(".gitignore");
    let content = std::fs::read_to_string(&gitignore_path).unwrap_or_default();
    if !content.lines().any(|line| line.trim() == ".env.squad") {
        let mut new_content = content;
        if !new_content.is_empty() && !new_content.ends_with('\n') {
            new_content.push('\n');
        }
        new_content.push_str(".env.squad\n");
        std::fs::write(&gitignore_path, new_content)?;
    }
    Ok(())
}

/// Check if .env.squad exists and has non-empty TELE_TOKEN and TELE_CHAT_ID.
fn env_squad_has_credentials(project_root: &std::path::Path) -> bool {
    let env_path = project_root.join(".env.squad");
    match std::fs::read_to_string(&env_path) {
        Ok(content) => {
            let has_token = content
                .lines()
                .any(|l| l.starts_with("TELE_TOKEN=") && l.len() > "TELE_TOKEN=".len());
            let has_chat_id = content
                .lines()
                .any(|l| l.starts_with("TELE_CHAT_ID=") && l.len() > "TELE_CHAT_ID=".len());
            has_token && has_chat_id
        }
        Err(_) => false,
    }
}

/// Prompt user for Telegram Bot Token, Chat ID, and Topic ID.
/// Writes .env.squad with the collected values.
/// Returns true if credentials were provided, false if skipped.
fn prompt_telegram_credentials(project_root: &std::path::Path) -> anyhow::Result<bool> {
    print!("  Bot Token: ");
    std::io::stdout().flush().ok();
    let mut token = String::new();
    std::io::stdin().read_line(&mut token).ok();
    let token = token.trim().to_string();

    print!("  Chat ID: ");
    std::io::stdout().flush().ok();
    let mut chat_id = String::new();
    std::io::stdin().read_line(&mut chat_id).ok();
    let chat_id = chat_id.trim().to_string();

    print!("  Topic ID (Enter to skip): ");
    std::io::stdout().flush().ok();
    let mut topic_id = String::new();
    std::io::stdin().read_line(&mut topic_id).ok();
    let topic_id = topic_id.trim().to_string();

    if !token.is_empty() && !chat_id.is_empty() {
        write_env_squad(project_root, &token, &chat_id, &topic_id)?;
        Ok(true)
    } else {
        println!("  Telegram: skipped (token and chat_id required)");
        Ok(false)
    }
}

/// Append `telegram:` section to squad.yml (string append to preserve formatting).
fn append_telegram_to_squad_yml(config_path: &std::path::Path) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(config_path)?;
    // Check if telegram section already exists
    if content.contains("\ntelegram:") || content.starts_with("telegram:") {
        return Ok(());
    }
    let mut new_content = content;
    if !new_content.ends_with('\n') {
        new_content.push('\n');
    }
    new_content.push_str("\ntelegram:\n  enabled: true\n  notify_agents: all\n");
    std::fs::write(config_path, new_content)?;
    Ok(())
}

// ── Public shims for update.rs ───────────────────────────────────────────────

pub fn get_launch_command_pub(agent: &config::AgentConfig) -> String {
    get_launch_command(agent)
}

pub fn auto_install_hooks_pub(provider: &str) -> anyhow::Result<bool> {
    auto_install_hooks(provider)
}

pub fn install_session_start_hook_pub(
    provider: &str,
    project_root: &std::path::Path,
) -> anyhow::Result<bool> {
    install_session_start_hook(provider, project_root)
}

pub fn install_telegram_hooks_pub(
    telegram: &config::TelegramConfig,
    project_root: &std::path::Path,
    providers: &[String],
) -> anyhow::Result<()> {
    install_telegram_hooks(telegram, project_root, providers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_claude_hooks_includes_post_tool_use() {
        let tmp = tempfile::TempDir::new().unwrap();
        let settings_file = tmp.path().join(".claude").join("settings.json");
        let settings_str = settings_file.to_str().unwrap();

        install_claude_hooks(settings_str).unwrap();

        let content = std::fs::read_to_string(&settings_file).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Verify Stop hook exists
        assert!(settings["hooks"]["Stop"].is_array(), "Stop hook must exist");

        // Verify Notification hook exists with both matchers
        let notif = &settings["hooks"]["Notification"];
        assert!(notif.is_array(), "Notification hook must exist");
        assert_eq!(notif.as_array().unwrap().len(), 2);
        assert_eq!(notif[0]["matcher"].as_str().unwrap(), "permission_prompt");
        assert_eq!(notif[1]["matcher"].as_str().unwrap(), "elicitation_dialog");

        // Verify PostToolUse hook exists with AskUserQuestion matcher
        let ptu = &settings["hooks"]["PostToolUse"];
        assert!(ptu.is_array(), "PostToolUse hook must exist");
        assert_eq!(ptu[0]["matcher"].as_str().unwrap(), "AskUserQuestion");

        // Verify the command calls notify with the standard pattern
        let cmd = ptu[0]["hooks"][0]["command"].as_str().unwrap();
        assert!(
            cmd.contains("squad-station notify"),
            "PostToolUse command must call squad-station notify"
        );

        // Base hooks must NOT include SessionStart (opt-in via install_session_start_hook)
        assert!(
            settings["hooks"]["SessionStart"].is_null(),
            "SessionStart must not be installed by base hooks"
        );
    }

    #[test]
    fn test_install_claude_hooks_uses_tmux_display_message() {
        let tmp = tempfile::TempDir::new().unwrap();
        let settings_file = tmp.path().join(".claude").join("settings.json");
        let settings_str = settings_file.to_str().unwrap();

        install_claude_hooks(settings_str).unwrap();

        let content = std::fs::read_to_string(&settings_file).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        let stop_cmd = settings["hooks"]["Stop"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(
            stop_cmd.contains("display-message"),
            "Stop hook must use tmux display-message for session name: {}",
            stop_cmd
        );
        assert!(
            stop_cmd.contains("2>/dev/null"),
            "Stop hook must redirect stderr to /dev/null: {}",
            stop_cmd
        );
        assert!(
            !stop_cmd.contains("SQUAD_AGENT_NAME"),
            "Stop hook must NOT use $SQUAD_AGENT_NAME (not available in hook context): {}",
            stop_cmd
        );
        assert!(
            !stop_cmd.contains("list-panes"),
            "Stop hook must NOT use list-panes (depends on $TMUX_PANE): {}",
            stop_cmd
        );
        assert!(
            !stop_cmd.contains("TMUX_PANE"),
            "Stop hook must NOT depend on $TMUX_PANE: {}",
            stop_cmd
        );
    }

    #[test]
    fn test_install_claude_hooks_no_json_stdout() {
        let tmp = tempfile::TempDir::new().unwrap();
        let settings_file = tmp.path().join(".claude").join("settings.json");
        let settings_str = settings_file.to_str().unwrap();

        install_claude_hooks(settings_str).unwrap();

        let content = std::fs::read_to_string(&settings_file).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        let stop_cmd = settings["hooks"]["Stop"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(
            !stop_cmd.contains("printf"),
            "Claude Code hook must NOT add printf '{{}}' — stdout is ignored: {}",
            stop_cmd
        );
    }

    #[test]
    fn test_install_claude_hooks_preserves_existing_settings() {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let settings_file = claude_dir.join("settings.json");

        // Pre-populate with existing settings
        let existing = serde_json::json!({
            "customKey": "preserved",
            "hooks": {
                "PreToolUse": [{"matcher": "Bash", "hooks": []}]
            }
        });
        std::fs::write(
            &settings_file,
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let settings_str = settings_file.to_str().unwrap();
        install_claude_hooks(settings_str).unwrap();

        let content = std::fs::read_to_string(&settings_file).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Existing keys preserved
        assert_eq!(settings["customKey"].as_str().unwrap(), "preserved");
        // Existing hooks preserved
        assert!(settings["hooks"]["PreToolUse"].is_array());
        // New hooks added
        assert!(settings["hooks"]["PostToolUse"].is_array());
        assert!(settings["hooks"]["Stop"].is_array());
        assert!(settings["hooks"]["Notification"].is_array());
        // SessionStart must NOT be added by base hooks
        assert!(settings["hooks"]["SessionStart"].is_null());
    }

    #[test]
    fn test_install_gemini_hooks_json_stdout() {
        let tmp = tempfile::TempDir::new().unwrap();
        let gemini_dir = tmp.path().join(".gemini");
        std::fs::create_dir_all(&gemini_dir).unwrap();
        let settings_file = gemini_dir.join("settings.json");
        let settings_str = settings_file.to_str().unwrap();

        install_gemini_hooks(settings_str).unwrap();

        let content = std::fs::read_to_string(&settings_file).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        let signal_cmd = settings["hooks"]["AfterAgent"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        // Must end with printf '{}' for valid JSON stdout
        assert!(
            signal_cmd.contains("printf '{}'"),
            "Gemini hook MUST output valid JSON via printf: {}",
            signal_cmd
        );
        // Must redirect signal output to /dev/null (not to Gemini's stdout)
        assert!(
            signal_cmd.contains(">/dev/null 2>&1"),
            "Gemini hook must redirect signal output to /dev/null: {}",
            signal_cmd
        );
    }

    #[test]
    fn test_install_gemini_hooks_uses_afteragent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let gemini_dir = tmp.path().join(".gemini");
        std::fs::create_dir_all(&gemini_dir).unwrap();
        let settings_file = gemini_dir.join("settings.json");
        let settings_str = settings_file.to_str().unwrap();

        install_gemini_hooks(settings_str).unwrap();

        let content = std::fs::read_to_string(&settings_file).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Must use AfterAgent, NOT Stop
        assert!(
            settings["hooks"]["AfterAgent"].is_array(),
            "Gemini must use AfterAgent hook"
        );
        assert!(
            settings["hooks"]["Stop"].is_null(),
            "Gemini must NOT use Stop hook"
        );
    }

    #[test]
    fn test_install_gemini_hooks_has_name_and_timeout() {
        let tmp = tempfile::TempDir::new().unwrap();
        let gemini_dir = tmp.path().join(".gemini");
        std::fs::create_dir_all(&gemini_dir).unwrap();
        let settings_file = gemini_dir.join("settings.json");
        let settings_str = settings_file.to_str().unwrap();

        install_gemini_hooks(settings_str).unwrap();

        let content = std::fs::read_to_string(&settings_file).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        let hook = &settings["hooks"]["AfterAgent"][0]["hooks"][0];
        assert_eq!(
            hook["name"].as_str().unwrap(),
            "squad-signal",
            "Gemini hook must have name field"
        );
        assert!(
            hook["description"].is_string(),
            "Gemini hook must have description field"
        );
        assert_eq!(
            hook["timeout"].as_u64().unwrap(),
            30000,
            "Gemini hook must have timeout field"
        );
    }

    #[test]
    fn test_install_gemini_hooks_uses_tmux_display_message() {
        let tmp = tempfile::TempDir::new().unwrap();
        let gemini_dir = tmp.path().join(".gemini");
        std::fs::create_dir_all(&gemini_dir).unwrap();
        let settings_file = gemini_dir.join("settings.json");
        let settings_str = settings_file.to_str().unwrap();

        install_gemini_hooks(settings_str).unwrap();

        let content = std::fs::read_to_string(&settings_file).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        let signal_cmd = settings["hooks"]["AfterAgent"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(
            signal_cmd.contains("display-message"),
            "Gemini hook must use tmux display-message for session name: {}",
            signal_cmd
        );
        assert!(
            !signal_cmd.contains("SQUAD_AGENT_NAME"),
            "Gemini hook must NOT use $SQUAD_AGENT_NAME (not available in hook context): {}",
            signal_cmd
        );
        assert!(
            !signal_cmd.contains("list-panes"),
            "Gemini hook must NOT use list-panes: {}",
            signal_cmd
        );
    }

    #[test]
    fn test_install_gemini_hooks_excludes_session_start() {
        let tmp = tempfile::TempDir::new().unwrap();
        let gemini_dir = tmp.path().join(".gemini");
        std::fs::create_dir_all(&gemini_dir).unwrap();
        let settings_file = gemini_dir.join("settings.json");
        let settings_str = settings_file.to_str().unwrap();

        install_gemini_hooks(settings_str).unwrap();

        let content = std::fs::read_to_string(&settings_file).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Verify base hooks exist
        assert!(
            settings["hooks"]["AfterAgent"].is_array(),
            "AfterAgent hook must exist"
        );
        assert!(
            settings["hooks"]["Notification"].is_array(),
            "Notification hook must exist"
        );
        // SessionStart must NOT be installed by base hooks
        assert!(
            settings["hooks"]["SessionStart"].is_null(),
            "SessionStart must not be installed by base hooks"
        );
    }

    #[test]
    fn test_install_codex_hooks_includes_stop_and_post_tool_use() {
        let tmp = tempfile::TempDir::new().unwrap();
        let hooks_file = tmp.path().join(".codex").join("hooks.json");
        let hooks_str = hooks_file.to_str().unwrap();

        install_codex_hooks(hooks_str).unwrap();

        let content = std::fs::read_to_string(&hooks_file).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Verify Stop hook exists
        assert!(settings["hooks"]["Stop"].is_array(), "Stop hook must exist");

        // Verify PostToolUse hook exists with Bash matcher
        let ptu = &settings["hooks"]["PostToolUse"];
        assert!(ptu.is_array(), "PostToolUse hook must exist");
        assert_eq!(ptu[0]["matcher"].as_str().unwrap(), "Bash");

        // Base hooks must NOT include SessionStart (opt-in via install_session_start_hook)
        assert!(
            settings["hooks"]["SessionStart"].is_null(),
            "SessionStart must not be installed by base hooks"
        );
    }

    #[test]
    fn test_install_codex_hooks_uses_tmux_display_message() {
        let tmp = tempfile::TempDir::new().unwrap();
        let hooks_file = tmp.path().join(".codex").join("hooks.json");
        let hooks_str = hooks_file.to_str().unwrap();

        install_codex_hooks(hooks_str).unwrap();

        let content = std::fs::read_to_string(&hooks_file).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        let stop_cmd = settings["hooks"]["Stop"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(
            stop_cmd.contains("display-message"),
            "Codex hook must use tmux display-message for session name: {}",
            stop_cmd
        );
        assert!(
            !stop_cmd.contains("SQUAD_AGENT_NAME"),
            "Codex hook must NOT use $SQUAD_AGENT_NAME: {}",
            stop_cmd
        );
    }

    #[test]
    fn test_install_codex_hooks_no_json_stdout() {
        let tmp = tempfile::TempDir::new().unwrap();
        let hooks_file = tmp.path().join(".codex").join("hooks.json");
        let hooks_str = hooks_file.to_str().unwrap();

        install_codex_hooks(hooks_str).unwrap();

        let content = std::fs::read_to_string(&hooks_file).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        let stop_cmd = settings["hooks"]["Stop"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(
            !stop_cmd.contains("printf"),
            "Codex hook must NOT add printf '{{}}' — stdout is not required to be JSON: {}",
            stop_cmd
        );
    }

    #[test]
    fn test_install_codex_hooks_creates_config_toml_with_feature_flag() {
        let tmp = tempfile::TempDir::new().unwrap();
        let hooks_file = tmp.path().join(".codex").join("hooks.json");
        let hooks_str = hooks_file.to_str().unwrap();

        install_codex_hooks(hooks_str).unwrap();

        let config_toml = tmp.path().join(".codex").join("config.toml");
        assert!(config_toml.exists(), "config.toml must be created");
        let content = std::fs::read_to_string(&config_toml).unwrap();
        assert!(
            content.contains("[features]"),
            "config.toml must contain [features] section"
        );
        assert!(
            content.contains("codex_hooks = true"),
            "config.toml must enable codex_hooks feature flag"
        );
    }

    #[test]
    fn test_install_codex_hooks_preserves_existing_config_toml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let codex_dir = tmp.path().join(".codex");
        std::fs::create_dir_all(&codex_dir).unwrap();

        // Pre-existing config.toml with other settings
        let config_toml = codex_dir.join("config.toml");
        std::fs::write(&config_toml, "[model]\ndefault = \"gpt-5.4\"\n").unwrap();

        let hooks_file = codex_dir.join("hooks.json");
        let hooks_str = hooks_file.to_str().unwrap();

        install_codex_hooks(hooks_str).unwrap();

        let content = std::fs::read_to_string(&config_toml).unwrap();
        assert!(
            content.contains("default = \"gpt-5.4\""),
            "existing config must be preserved"
        );
        assert!(
            content.contains("codex_hooks = true"),
            "feature flag must be appended"
        );
    }

    #[test]
    fn test_install_codex_hooks_idempotent_config_toml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let hooks_file = tmp.path().join(".codex").join("hooks.json");
        let hooks_str = hooks_file.to_str().unwrap();

        // Install twice
        install_codex_hooks(hooks_str).unwrap();
        install_codex_hooks(hooks_str).unwrap();

        let config_toml = tmp.path().join(".codex").join("config.toml");
        let content = std::fs::read_to_string(&config_toml).unwrap();
        let count = content.matches("codex_hooks").count();
        assert_eq!(
            count, 1,
            "codex_hooks must appear exactly once after double install, got {}",
            count
        );
    }

    #[test]
    fn test_install_session_start_hook_codex() {
        let tmp = tempfile::TempDir::new().unwrap();
        let codex_dir = tmp.path().join(".codex");
        std::fs::create_dir_all(&codex_dir).unwrap();
        let hooks_file = codex_dir.join("hooks.json");
        std::fs::write(&hooks_file, r#"{"hooks":{"Stop":[]}}"#).unwrap();

        let result = install_session_start_hook("codex", tmp.path());
        assert!(result.unwrap());

        let content = std::fs::read_to_string(&hooks_file).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        let ss = &settings["hooks"]["SessionStart"];
        assert!(ss.is_array(), "SessionStart hook must exist");
        let ss_cmd = ss[0]["hooks"][0]["command"].as_str().unwrap();
        assert_eq!(ss_cmd, "squad-station context --inject");

        // Existing hooks preserved
        assert!(settings["hooks"]["Stop"].is_array());
    }

    #[test]
    fn test_install_session_start_hook_claude() {
        let tmp = tempfile::TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let settings_file = claude_dir.join("settings.json");
        // Pre-populate with base hooks
        std::fs::write(&settings_file, r#"{"hooks":{"Stop":[]}}"#).unwrap();

        let result = install_session_start_hook("claude-code", tmp.path());
        assert!(result.unwrap());

        let content = std::fs::read_to_string(&settings_file).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        // SessionStart hook installed
        let ss = &settings["hooks"]["SessionStart"];
        assert!(ss.is_array(), "SessionStart hook must exist");
        let ss_cmd = ss[0]["hooks"][0]["command"].as_str().unwrap();
        assert_eq!(ss_cmd, "squad-station context --inject");

        // Existing hooks preserved
        assert!(settings["hooks"]["Stop"].is_array());
    }

    #[test]
    fn test_install_session_start_hook_gemini() {
        let tmp = tempfile::TempDir::new().unwrap();
        let gemini_dir = tmp.path().join(".gemini");
        std::fs::create_dir_all(&gemini_dir).unwrap();
        let settings_file = gemini_dir.join("settings.json");
        std::fs::write(&settings_file, r#"{"hooks":{"AfterAgent":[]}}"#).unwrap();

        let result = install_session_start_hook("gemini-cli", tmp.path());
        assert!(result.unwrap());

        let content = std::fs::read_to_string(&settings_file).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        let ss = &settings["hooks"]["SessionStart"];
        assert!(ss.is_array(), "SessionStart hook must exist");
        let ss_cmd = ss[0]["hooks"][0]["command"].as_str().unwrap();
        assert_eq!(ss_cmd, "squad-station context --inject");

        // Existing hooks preserved
        assert!(settings["hooks"]["AfterAgent"].is_array());
    }

    #[test]
    fn test_install_session_start_hook_unknown_provider_returns_false() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(!install_session_start_hook("unknown-tool", tmp.path()).unwrap());
    }

    #[test]
    fn test_install_sdd_rules_copies_to_claude_and_gemini() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();

        // Create source rule file
        let rules_dir = root.join(".squad").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::write(
            rules_dir.join("git-workflow-get-shit-done.md"),
            "# GSD Git Workflow\nBranch naming: feat/, fix/",
        )
        .unwrap();

        let providers = vec!["claude-code".to_string(), "gemini-cli".to_string()];
        let installed = install_sdd_rules("get-shit-done", root, &providers).unwrap();

        assert_eq!(installed.len(), 2);
        assert!(installed.contains(&".claude/rules/git-workflow-get-shit-done.md".to_string()));
        assert!(installed.contains(&".gemini/rules/git-workflow-get-shit-done.md".to_string()));

        // Verify file contents were copied correctly
        let claude_rule =
            std::fs::read_to_string(root.join(".claude/rules/git-workflow-get-shit-done.md"))
                .unwrap();
        assert!(claude_rule.contains("GSD Git Workflow"));

        let gemini_rule =
            std::fs::read_to_string(root.join(".gemini/rules/git-workflow-get-shit-done.md"))
                .unwrap();
        assert!(gemini_rule.contains("GSD Git Workflow"));
    }

    #[test]
    fn test_install_sdd_rules_missing_source_returns_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        let providers = vec!["claude-code".to_string()];
        let installed = install_sdd_rules("nonexistent-sdd", tmp.path(), &providers).unwrap();
        assert!(installed.is_empty());
    }

    #[test]
    fn test_install_sdd_rules_skips_unsupported_providers() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();

        let rules_dir = root.join(".squad").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::write(
            rules_dir.join("git-workflow-bmad-method.md"),
            "# BMAD Git Workflow",
        )
        .unwrap();

        let providers = vec!["claude-code".to_string()];
        let installed = install_sdd_rules("bmad-method", root, &providers).unwrap();

        assert_eq!(installed.len(), 1);
        assert!(installed[0].contains(".claude/rules"));
    }

    #[test]
    fn test_install_sdd_rules_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();

        let rules_dir = root.join(".squad").join("rules");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::write(
            rules_dir.join("git-workflow-openspec.md"),
            "# OpenSpec Git Workflow",
        )
        .unwrap();

        let providers = vec!["claude-code".to_string()];

        // Run twice — should not fail
        let first = install_sdd_rules("openspec", root, &providers).unwrap();
        let second = install_sdd_rules("openspec", root, &providers).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
    }

    #[test]
    fn test_rules_dir_for_provider() {
        assert_eq!(rules_dir_for_provider("claude-code"), Some(".claude/rules"));
        assert_eq!(rules_dir_for_provider("gemini-cli"), Some(".gemini/rules"));
        assert_eq!(rules_dir_for_provider("unknown"), None);
    }

    #[test]
    fn test_is_safe_model_value_valid() {
        assert!(is_safe_model_value("claude-opus"));
        assert!(is_safe_model_value("gemini-3.1-pro-preview"));
        assert!(is_safe_model_value("gpt_4o:latest"));
    }

    #[test]
    fn test_is_safe_model_value_rejects_injection() {
        assert!(!is_safe_model_value("opus; rm -rf /"));
        assert!(!is_safe_model_value("model$(whoami)"));
        assert!(!is_safe_model_value("model`id`"));
        assert!(!is_safe_model_value(""));
    }

    #[test]
    fn test_get_launch_command_codex() {
        let agent = config::AgentConfig {
            name: Some("coder".to_string()),
            provider: "codex".to_string(),
            role: "worker".to_string(),
            model: Some("gpt-5.4".to_string()),
            description: None,
        };
        assert_eq!(get_launch_command(&agent), "codex --yolo --model gpt-5.4");
    }

    #[test]
    fn test_get_launch_command_codex_no_model() {
        let agent = config::AgentConfig {
            name: Some("coder".to_string()),
            provider: "codex".to_string(),
            role: "worker".to_string(),
            model: None,
            description: None,
        };
        assert_eq!(get_launch_command(&agent), "codex --yolo");
    }

    #[test]
    fn test_get_launch_command_claude() {
        let agent = config::AgentConfig {
            name: Some("impl".to_string()),
            provider: "claude-code".to_string(),
            role: "worker".to_string(),
            model: Some("sonnet".to_string()),
            description: None,
        };
        assert_eq!(
            get_launch_command(&agent),
            "claude --dangerously-skip-permissions --model sonnet"
        );
    }

    // ── Telegram hook tests ──────────────────────────────────────────────

    #[test]
    fn test_install_telegram_creates_script_and_env() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project_root = tmp.path();

        // Create pre-existing claude settings (base hooks must exist first)
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
        install_telegram_hooks(&tg, project_root, &["claude-code".to_string()]).unwrap();

        // Script created
        let script = project_root.join(".squad/hooks/notify-telegram.sh");
        assert!(script.exists(), "notify-telegram.sh must be created");

        // telegram.env created
        let env = project_root.join(".squad/telegram.env");
        assert!(env.exists(), "telegram.env must be created");
        let content = std::fs::read_to_string(&env).unwrap();
        assert!(content.contains("TELE_NOTIFY_AGENTS=all"));
    }

    #[test]
    fn test_install_telegram_appends_to_existing_hooks() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project_root = tmp.path();

        // Create pre-existing claude settings with signal hook
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
        install_telegram_hooks(&tg, project_root, &["claude-code".to_string()]).unwrap();

        let content = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        // Stop should now have 2 entries: original signal + telegram
        let stop = settings["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 2, "Stop must have signal + telegram entries");

        // First entry is the original signal hook
        let first_cmd = stop[0]["hooks"][0]["command"].as_str().unwrap();
        assert!(first_cmd.contains("squad-station signal"));

        // Second entry is the telegram hook
        let second_cmd = stop[1]["hooks"][0]["command"].as_str().unwrap();
        assert!(second_cmd.contains("notify-telegram.sh"));
    }

    #[test]
    fn test_install_telegram_idempotent() {
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

        // Install twice
        install_telegram_hooks(&tg, project_root, &["claude-code".to_string()]).unwrap();
        install_telegram_hooks(&tg, project_root, &["claude-code".to_string()]).unwrap();

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
    fn test_install_telegram_agent_filter_in_env() {
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
            notify_agents: config::NotifyAgents::List(vec![
                "orchestrator".to_string(),
                "implement".to_string(),
            ]),
        };
        install_telegram_hooks(&tg, project_root, &["claude-code".to_string()]).unwrap();

        let content = std::fs::read_to_string(project_root.join(".squad/telegram.env")).unwrap();
        assert!(content.contains("TELE_NOTIFY_AGENTS=orchestrator,implement"));
    }

    #[test]
    fn test_ensure_gitignore_adds_env_squad() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project_root = tmp.path();

        // Create a .gitignore without .env.squad
        std::fs::write(project_root.join(".gitignore"), "target/\n").unwrap();

        ensure_gitignore_env_squad(project_root).unwrap();

        let content = std::fs::read_to_string(project_root.join(".gitignore")).unwrap();
        assert!(content.contains(".env.squad"));
    }

    #[test]
    fn test_ensure_gitignore_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project_root = tmp.path();

        std::fs::write(project_root.join(".gitignore"), "target/\n.env.squad\n").unwrap();

        ensure_gitignore_env_squad(project_root).unwrap();

        let content = std::fs::read_to_string(project_root.join(".gitignore")).unwrap();
        assert_eq!(
            content.matches(".env.squad").count(),
            1,
            "must not duplicate .env.squad"
        );
    }

    #[test]
    fn test_write_env_squad_with_topic_id() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_env_squad(tmp.path(), "tok123", "chat456", "topic789").unwrap();
        let content = std::fs::read_to_string(tmp.path().join(".env.squad")).unwrap();
        assert!(content.contains("TELE_TOKEN=tok123"));
        assert!(content.contains("TELE_CHAT_ID=chat456"));
        assert!(content.contains("TELE_TOPIC_ID=topic789"));
    }

    #[test]
    fn test_write_env_squad_without_topic_id() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_env_squad(tmp.path(), "tok123", "chat456", "").unwrap();
        let content = std::fs::read_to_string(tmp.path().join(".env.squad")).unwrap();
        assert!(content.contains("TELE_TOKEN=tok123"));
        assert!(content.contains("TELE_CHAT_ID=chat456"));
        assert!(content.contains("TELE_TOPIC_ID=\n") || content.contains("TELE_TOPIC_ID=\r\n"));
    }

    #[test]
    fn test_append_telegram_to_squad_yml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let yml = tmp.path().join("squad.yml");
        std::fs::write(
            &yml,
            "project: test\norchestrator:\n  provider: claude-code\nagents: []\n",
        )
        .unwrap();

        append_telegram_to_squad_yml(&yml).unwrap();

        let content = std::fs::read_to_string(&yml).unwrap();
        assert!(content.contains("telegram:"));
        assert!(content.contains("enabled: true"));
        assert!(content.contains("notify_agents: all"));
    }

    #[test]
    fn test_append_telegram_to_squad_yml_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let yml = tmp.path().join("squad.yml");
        std::fs::write(
            &yml,
            "project: test\ntelegram:\n  enabled: true\n  notify_agents: all\n",
        )
        .unwrap();

        append_telegram_to_squad_yml(&yml).unwrap();

        let content = std::fs::read_to_string(&yml).unwrap();
        assert_eq!(
            content.matches("telegram:").count(),
            1,
            "must not duplicate telegram section"
        );
    }

    #[test]
    fn test_install_telegram_gemini_hook_format() {
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
        install_telegram_hooks(&tg, project_root, &["gemini-cli".to_string()]).unwrap();

        let content = std::fs::read_to_string(gemini_dir.join("settings.json")).unwrap();
        let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

        let after = settings["hooks"]["AfterAgent"].as_array().unwrap();
        assert_eq!(after.len(), 2);

        // Gemini hook must end with printf '{}' for JSON stdout requirement
        let tg_cmd = after[1]["hooks"][0]["command"].as_str().unwrap();
        assert!(
            tg_cmd.contains("printf '{}'"),
            "Gemini telegram hook must include printf for JSON stdout: {}",
            tg_cmd
        );
        assert!(
            after[1]["hooks"][0]["name"].as_str().unwrap() == "squad-telegram",
            "Gemini hook must have name field"
        );
    }
}

/// Returns the provider-specific rules directory path for a given provider.
/// claude-code → .claude/rules/, gemini-cli → .gemini/rules/, others → None.
fn rules_dir_for_provider(provider: &str) -> Option<&'static str> {
    match provider {
        "claude-code" => Some(".claude/rules"),
        "codex" => Some(".codex/rules"),
        "gemini-cli" => Some(".gemini/rules"),
        _ => None,
    }
}

/// Install SDD git workflow rule file into all provider-specific rules directories.
/// Looks for `.squad/rules/git-workflow-<sdd_name>.md` relative to project_root.
/// Returns a list of destination paths where the rule was installed.
fn install_sdd_rules(
    sdd_name: &str,
    project_root: &std::path::Path,
    providers: &[String],
) -> anyhow::Result<Vec<String>> {
    let rule_filename = format!("git-workflow-{}.md", sdd_name);
    let source = project_root
        .join(".squad")
        .join("rules")
        .join(&rule_filename);

    if !source.exists() {
        return Ok(vec![]);
    }

    let mut installed = vec![];
    for provider in providers {
        if let Some(rules_dir) = rules_dir_for_provider(provider) {
            let dest_dir = project_root.join(rules_dir);
            std::fs::create_dir_all(&dest_dir)?;
            let dest = dest_dir.join(&rule_filename);
            std::fs::copy(&source, &dest)?;
            installed.push(format!("{}/{}", rules_dir, rule_filename));
        }
    }

    Ok(installed)
}

fn print_hook_instructions(settings_path: &str, event: &str, matcher: &str) {
    println!(
        "\nHook setup instructions for {} (event: {}):\n\n  \
        Create the file with the following content, or add to your existing hooks:\n\n  \
        {{\n    \"hooks\": {{\n      \"{}\": [\n        \
        {{ \"matcher\": \"{}\", \"hooks\": [ {{ \"type\": \"command\", \"command\": \"squad-station signal \\\"$(tmux display-message -p '#S' 2>/dev/null)\\\" 2>/dev/null\" }} ] }}\n      \
        ]\n    }}\n  }}",
        settings_path, event, event, matcher
    );
}
