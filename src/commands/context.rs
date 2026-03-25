use std::path::Path;

use crate::config::SddConfig;
use crate::db::agents::Agent;
use crate::{config, db};

const BOOTSTRAP_MARKER_START: &str = "<!-- squad-station:bootstrap-start -->";
const BOOTSTRAP_MARKER_END: &str = "<!-- squad-station:bootstrap-end -->";

pub fn build_orchestrator_md(
    agents: &[Agent],
    project_root: &str,
    sdd_configs: &[SddConfig],
) -> String {
    let mut out = String::new();

    // Collect worker agents
    let workers: Vec<&Agent> = agents.iter().filter(|a| a.role != "orchestrator").collect();

    // ── Role ─────────────────────────────────────────────────────────────
    out.push_str("You are the orchestrator. You DO NOT directly write code, modify files, or run workflows.\n");
    out.push_str("You COORDINATE agents on behalf of the user via `squad-station send`.\n\n");

    // ── Tool Restrictions ───────────────────────────────────────────────
    out.push_str("## Tool Restrictions — Tiered\n\n");
    out.push_str("Think like a Project Manager: you read dashboards to make informed decisions, but you delegate all deep work to agents.\n\n");
    out.push_str("### ALLOWED — You may do these directly (no delegation needed)\n\n");
    out.push_str("- `squad-station` CLI commands (send, list, agents, status, peek)\n");
    out.push_str("- `tmux capture-pane -t <agent> -p` — to read agent output after `[SQUAD SIGNAL]`\n");
    if !sdd_configs.is_empty() {
        out.push_str("- Reading SDD playbook(s) listed in PRE-FLIGHT\n");
    }
    out.push_str("- Reading **tracking/status files** (e.g. sprint-status.yaml, epics.md, REQUIREMENTS.md, CHANGELOG) — these are your dashboards\n");
    out.push_str("- `git status`, `git branch` — orientation only (which branch, clean/dirty state)\n");
    out.push_str("- Asking the user for clarification\n\n");
    out.push_str("### MUST DELEGATE — Send these to agents via `squad-station send`\n\n");
    out.push_str("- Reading or analyzing **source code** files (*.rs, *.ts, *.py, etc.)\n");
    out.push_str("- Deep git research (`git log`, `git diff`, `git blame` for analysis)\n");
    out.push_str("- Code search (`grep`, `Grep`, `Glob` for finding code patterns)\n");
    out.push_str("- Generating reports, analysis, or summaries from code\n");
    out.push_str("- Running tests, builds, or any compilation commands\n");
    out.push_str("- Writing, editing, or modifying any file\n");
    out.push_str("- Using the `Agent` tool to spawn subagents\n\n");
    out.push_str("**The principle:** If it touches source code, requires code analysis, or produces artifacts — delegate it. If it reads project status to inform your next routing decision — do it yourself.\n\n");

    // ── PRE-FLIGHT ───────────────────────────────────────────────────────
    out.push_str("## PRE-FLIGHT — Execute IMMEDIATELY before any task\n\n");
    if !sdd_configs.is_empty() {
        out.push_str("> Read the SDD playbook(s) below. These define your WORKING PRINCIPLES — how to delegate tasks, coordinate agents, and follow the methodology. You MUST reference and follow these guidelines throughout the session. Do NOT invent your own workflow.\n\n");
        for sdd in sdd_configs {
            out.push_str(&format!("- [ ] Read `{}`\n", sdd.playbook));
        }
        out.push('\n');
        out.push_str("Only proceed after reading. The playbook defines your workflow.\n\n");
    }
    out.push_str(&format!("- [ ] Project root: `{}`\n", project_root));
    out.push_str("- [ ] Verify agents are alive: `squad-station agents`\n\n");

    // ── Completion Notification ──────────────────────────────────────────
    out.push_str("## Completion Notification (Automatic)\n\n");
    out.push_str("Agents have a stop hook configured. When an agent completes a task, the hook\n");
    out.push_str(
        "**automatically sends a signal** back to your session. You **DO NOT need to**:\n",
    );
    out.push_str("- Continuously poll `tmux capture-pane` to track progress.\n");
    out.push_str("- Run `sleep`, `squad-station list`, or `squad-station agents` in a loop.\n");
    out.push_str("- Use the `Agent` tool to spawn subagents.\n\n");
    out.push_str("After assigning a task, **stop and wait for the signal**:\n\n");
    out.push_str("```\n");
    out.push_str("[SQUAD SIGNAL] Agent '<name>' completed task <id>. Read output: tmux capture-pane -t <name> -p | Next: squad-station status\n");
    out.push_str("```\n\n");

    // ── Context Management ─────────────────────────────────────────────
    out.push_str("## Context Management — `/clear`\n\n");
    out.push_str("You MUST send `/clear` to an agent BEFORE dispatching a new task if ANY of these conditions are true:\n\n");
    out.push_str("### Mandatory `/clear` Triggers\n\n");
    out.push_str("1. **Topic shift** — The new task is on a DIFFERENT topic/feature than the agent's last completed task.\n");
    out.push_str("   Examples: bug fix → new feature, UI work → backend work, different file areas.\n\n");
    out.push_str("2. **Task count threshold** — The agent has completed 3 or more consecutive tasks without a `/clear`.\n");
    out.push_str("   Count resets after each `/clear`.\n\n");
    out.push_str("3. **Agent hint** — The agent's output mentions context issues, suggests clearing,\n");
    out.push_str("   or shows signs of confusion (referencing old/irrelevant code).\n\n");
    out.push_str("### `/clear` Checklist (run BEFORE every `squad-station send`)\n\n");
    out.push_str("□ Is this a topic shift from the agent's last task? → /clear\n");
    out.push_str("□ Has the agent done 3+ tasks since last /clear? → /clear\n");
    out.push_str("□ Did the agent hint at context issues? → /clear\n");
    out.push_str("□ None of the above? → send task directly (no /clear needed)\n\n");
    out.push_str("### How to `/clear`\n\n");
    out.push_str("```bash\nsquad-station send <agent-name> --body \"/clear\"\n```\n\n");
    out.push_str("After `/clear`, the agent has ZERO memory. You MUST re-inject enough context\n");
    out.push_str("in the next task body so the agent can execute independently.\n\n");

    // ── Session Routing ──────────────────────────────────────────────────
    out.push_str("## Session Routing\n\n");
    out.push_str("Based on the nature of the work, independently decide the correct agent:\n\n");
    for agent in &workers {
        let desc = agent.description.as_deref().unwrap_or(&agent.role);
        let model = agent.model.as_deref().unwrap_or(&agent.tool);
        out.push_str(&format!("- **{}** ({}) — {}\n", agent.name, model, desc));
    }
    out.push_str("\n**Routing rules:**\n");
    out.push_str("- Reasoning, architecture, planning, review → brainstorm/planning agent\n");
    out.push_str("- Coding, implement, fix, build, deploy → implementation agent\n");
    out.push_str("- **Parallel** only when tasks are independent. **Sequential** when one output feeds another.\n\n");

    // ── SDD Orchestration ────────────────────────────────────────────────
    if !sdd_configs.is_empty() {
        out.push_str("## SDD Orchestration\n\n");
        out.push_str("The agents have SDD tools (slash commands, workflows) installed in their sessions. **You do NOT.**\n");
        out.push_str("Your job is to send the playbook's commands to the correct agent. Do not run them yourself.\n\n");
        out.push_str("**How it works:**\n");
        out.push_str("1. Read the playbook (PRE-FLIGHT) → identify the workflow steps and their slash commands\n");
        out.push_str("2. For each step: decide which agent handles it (see Session Routing)\n");
        out.push_str("3. Send the slash command as the task body:\n");
        out.push_str("   ```\n");
        if let Some(first_worker) = workers.first() {
            out.push_str(&format!(
                "   squad-station send {} --body \"/command-name\"\n",
                first_worker.name
            ));
        }
        out.push_str("   ```\n");
        out.push_str("4. STOP. Wait for `[SQUAD SIGNAL]`.\n");
        out.push_str("5. Read output → evaluate → send next step to the appropriate agent.\n\n");
        out.push_str("**CRITICAL:**\n");
        out.push_str("- Do NOT send raw task descriptions like \"build the login page\".\n");
        out.push_str("- Do NOT run slash commands, workflows, or Agent subagents yourself.\n");
        out.push_str(
            "- Send the playbook's exact commands. The agent knows how to execute them.\n\n",
        );
    }

    // ── Sending Tasks ────────────────────────────────────────────────────
    out.push_str("## Sending Tasks\n\n");
    out.push_str("```bash\n");
    for agent in &workers {
        out.push_str(&format!(
            "squad-station send {} --body \"<command or task>\"\n",
            agent.name
        ));
    }
    out.push_str("```\n\n");

    // ── Full Context Transfer ────────────────────────────────────────────
    out.push_str("## Full Context Transfer\n\n");
    out.push_str("When transferring results from one agent to another:\n");
    out.push_str("- Capture ENTIRE output: `tmux capture-pane -t <agent> -p -S -`\n");
    out.push_str("- Include complete context in the next task body.\n");
    out.push_str("- **Self-check:** \"If the target agent had NO other context, could it execute correctly?\" If NO → add more.\n\n");

    // ── Workflow Completion Discipline ────────────────────────────────────
    out.push_str("## Workflow Completion Discipline\n\n");
    out.push_str("- **NEVER** interrupt a running agent to move on.\n");
    out.push_str("- **WAIT** for the `[SQUAD SIGNAL]` before evaluating results.\n");
    out.push_str("- Only after the signal → read output → decide next step per playbook.\n\n");

    // ── QA Gate ──────────────────────────────────────────────────────────
    out.push_str("## QA Gate\n\n");
    out.push_str("After receiving `[SQUAD SIGNAL]`:\n");
    out.push_str("1. `tmux capture-pane -t <agent> -p -S -` — read full output\n");
    out.push_str("2. If agent reported errors → analyze the error, determine the fix, and send a follow-up task\n");
    out.push_str("3. If agent asked technical questions → answer from your dashboard knowledge if possible, otherwise delegate research to another agent\n");
    out.push_str("4. If agent asked about requirements where the user's INTENT is genuinely ambiguous → escalate to user\n");
    out.push_str("5. `squad-station list --agent <agent>` — confirm status is `completed`\n");
    out.push_str("6. Run the `/clear` checklist (see Context Management) — if ANY condition matches,\n");
    out.push_str("   send `/clear` to the agent BEFORE dispatching the next task.\n");
    out.push_str("7. Proceed to next step, or report to user ONLY when the ENTIRE workflow is complete.\n\n");

    // ── Agent Roster ─────────────────────────────────────────────────────
    out.push_str("## Agent Roster\n\n");
    out.push_str("| Agent | Model | Role | Description |\n");
    out.push_str("|-------|-------|------|-------------|\n");
    for agent in agents {
        let model = agent.model.as_deref().unwrap_or("\u{2014}");
        let desc = agent.description.as_deref().unwrap_or("\u{2014}");
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            agent.name, model, agent.role, desc
        ));
    }

    out
}

/// Detect the current tmux session name. Returns None if not in tmux.
pub fn detect_tmux_session() -> Option<String> {
    std::process::Command::new("tmux")
        .args(["display-message", "-p", "#S"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let name = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if name.is_empty() {
                    None
                } else {
                    Some(name)
                }
            } else {
                None
            }
        })
}

/// Format orchestrator content for stdout injection based on provider.
/// Claude Code: raw markdown. Gemini CLI: JSON with hookSpecificOutput.additionalContext.
pub fn format_inject_output(provider: &str, content: &str) -> String {
    match provider {
        "gemini-cli" => {
            let json = serde_json::json!({
                "hookSpecificOutput": {
                    "additionalContext": content
                }
            });
            serde_json::to_string(&json).unwrap_or_default()
        }
        _ => content.to_string(),
    }
}

/// Build the lightweight bootstrap block that tells the orchestrator to read
/// its full playbook. This block survives /clear and context compact because
/// it lives in the provider's project-level instructions file (CLAUDE.md / GEMINI.md).
pub fn build_bootstrap_block(orch_session_name: &str, playbook_path: &str) -> String {
    format!(
        "{}\n\
         ## Squad Station — Orchestrator Bootstrap\n\
         \n\
         If your tmux session is `{orch}`, you ARE the Squad Station orchestrator.\n\
         On EVERY new turn (including after /clear or context compact):\n\
         1. Read `{playbook}` — this defines your WORKING RULES. You MUST follow the principles and workflow described in it. Do NOT invent your own approach.\n\
         2. The SDD playbook (referenced in PRE-FLIGHT) defines HOW you work — task delegation, coordination, and discipline. Treat it as mandatory guidelines, not optional reading.\n\
         3. Execute PRE-FLIGHT before proceeding.\n\
         4. Do NOT write code directly. Coordinate agents via `squad-station send`.\n\
         \n\
         If your session name does NOT match `{orch}`, ignore this section.\n\
         {}",
        BOOTSTRAP_MARKER_START,
        BOOTSTRAP_MARKER_END,
        orch = orch_session_name,
        playbook = playbook_path,
    )
}

/// Resolve the provider-specific project doc file path and playbook path.
/// Returns (doc_file_relative, playbook_relative).
fn provider_doc_paths(provider: &str) -> (&'static str, &'static str) {
    match provider {
        "gemini-cli" => ("GEMINI.md", ".gemini/commands/squad-orchestrator.toml"),
        _ => ("CLAUDE.md", ".claude/commands/squad-orchestrator.md"),
    }
}

/// Inject the orchestrator bootstrap block into the provider's project doc file.
/// Idempotent: replaces existing block between markers, appends if no markers,
/// creates file if it doesn't exist.
pub fn inject_bootstrap_block(
    project_root: &Path,
    provider: &str,
    orch_session_name: &str,
) -> anyhow::Result<String> {
    let (doc_rel, playbook_rel) = provider_doc_paths(provider);
    let doc_path = project_root.join(doc_rel);
    let block = build_bootstrap_block(orch_session_name, playbook_rel);

    // Ensure parent directory exists
    if let Some(parent) = doc_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = match std::fs::read_to_string(&doc_path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // File doesn't exist — create with just the block
            std::fs::write(&doc_path, &block)?;
            return Ok(doc_rel.to_string());
        }
        Err(e) => return Err(e.into()),
    };

    let new_content = if let (Some(start), Some(end)) = (
        content.find(BOOTSTRAP_MARKER_START),
        content.find(BOOTSTRAP_MARKER_END),
    ) {
        // Replace existing block (start marker through end marker)
        let before = &content[..start];
        let after = &content[end + BOOTSTRAP_MARKER_END.len()..];
        format!("{}{}{}", before, block, after)
    } else {
        // Append block at end
        if content.ends_with('\n') {
            format!("{}\n{}\n", content, block)
        } else {
            format!("{}\n\n{}\n", content, block)
        }
    };

    std::fs::write(&doc_path, new_content)?;
    Ok(doc_rel.to_string())
}

pub async fn run(inject: bool) -> anyhow::Result<()> {
    let project_root = config::find_project_root()?;
    let config = config::load_config(&project_root.join("squad.yml"))?;

    if inject {
        return run_inject(&project_root, &config).await;
    }

    let db_path = config::resolve_db_path(&config)?;
    let pool = db::connect(&db_path).await?;

    let agents = db::agents::list_agents(&pool).await?;

    let project_root_str = project_root.to_string_lossy().to_string();
    let sdd_configs = config.sdd.as_deref().unwrap_or(&[]);
    let prompt_content = build_orchestrator_md(&agents, &project_root_str, sdd_configs);

    // Write slash command in provider-specific format and directory
    let (cmd_subdir, filename, file_content) = match config.orchestrator.provider.as_str() {
        "gemini-cli" => {
            // Gemini CLI: TOML format with description + prompt fields
            let toml = format!(
                "description = \"Squad Station orchestrator — coordinate AI agent squads\"\n\
                 prompt = \"\"\"\n{}\n\"\"\"",
                prompt_content
            );
            (".gemini/commands", "squad-orchestrator.toml", toml)
        }
        _ => {
            // Claude Code: plain markdown
            (".claude/commands", "squad-orchestrator.md", prompt_content)
        }
    };
    let cmd_dir = project_root.join(cmd_subdir);
    std::fs::create_dir_all(&cmd_dir)?;
    let context_path = cmd_dir.join(filename);
    std::fs::write(&context_path, &file_content)?;

    println!("Generated {}", context_path.display());
    Ok(())
}

/// Hook injection mode: output orchestrator context to stdout for SessionStart hooks.
/// Guards: only injects if the current tmux session is the orchestrator.
async fn run_inject(
    project_root: &std::path::Path,
    config: &config::SquadConfig,
) -> anyhow::Result<()> {
    // GUARD 1: Must be in a tmux session
    let session_name = match detect_tmux_session() {
        Some(name) => name,
        None => return Ok(()), // Not in tmux — silent exit
    };

    // GUARD 2: Must be the orchestrator session
    let orch_role = config
        .orchestrator
        .name
        .as_deref()
        .unwrap_or("orchestrator");
    let orch_name = config::sanitize_session_name(&format!("{}-{}", config.project, orch_role));
    if session_name != orch_name {
        return Ok(()); // Not the orchestrator — silent exit (workers get no injection)
    }

    // Generate content
    let db_path = config::resolve_db_path(config)?;
    let pool = db::connect(&db_path).await?;
    let agents = db::agents::list_agents(&pool).await?;

    let project_root_str = project_root.to_string_lossy().to_string();
    let sdd_configs = config.sdd.as_deref().unwrap_or(&[]);
    let content = build_orchestrator_md(&agents, &project_root_str, sdd_configs);

    // Output in provider-appropriate format
    print!("{}", format_inject_output(&config.orchestrator.provider, &content));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bootstrap_block_contains_markers_and_session() {
        let block = build_bootstrap_block("myproject-orchestrator", ".claude/commands/squad-orchestrator.md");
        assert!(block.starts_with(BOOTSTRAP_MARKER_START));
        assert!(block.ends_with(BOOTSTRAP_MARKER_END));
        assert!(block.contains("myproject-orchestrator"));
        assert!(block.contains(".claude/commands/squad-orchestrator.md"));
        assert!(block.contains("Do NOT write code directly"));
    }

    #[test]
    fn test_inject_bootstrap_file_not_exists() {
        let tmp = tempfile::TempDir::new().unwrap();
        let result = inject_bootstrap_block(
            tmp.path(),
            "claude-code",
            "proj-orchestrator",
        )
        .unwrap();

        assert_eq!(result, "CLAUDE.md");

        let content = std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
        assert!(content.contains(BOOTSTRAP_MARKER_START));
        assert!(content.contains(BOOTSTRAP_MARKER_END));
        assert!(content.contains("proj-orchestrator"));
        // Exactly one occurrence of each marker
        assert_eq!(content.matches(BOOTSTRAP_MARKER_START).count(), 1);
        assert_eq!(content.matches(BOOTSTRAP_MARKER_END).count(), 1);
    }

    #[test]
    fn test_inject_bootstrap_file_exists_without_markers() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("CLAUDE.md"), "# My Project\n\nExisting content.\n").unwrap();

        inject_bootstrap_block(tmp.path(), "claude-code", "proj-orchestrator").unwrap();

        let content = std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
        // Original content preserved
        assert!(content.contains("# My Project"));
        assert!(content.contains("Existing content."));
        // Bootstrap block appended
        assert!(content.contains(BOOTSTRAP_MARKER_START));
        assert!(content.contains(BOOTSTRAP_MARKER_END));
        assert!(content.contains("proj-orchestrator"));
        // Only one copy
        assert_eq!(content.matches(BOOTSTRAP_MARKER_START).count(), 1);
    }

    #[test]
    fn test_inject_bootstrap_idempotent_replaces_existing() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("CLAUDE.md"), "# My Project\n").unwrap();

        // First injection
        inject_bootstrap_block(tmp.path(), "claude-code", "proj-orchestrator").unwrap();
        let after_first = std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
        assert_eq!(after_first.matches(BOOTSTRAP_MARKER_START).count(), 1);

        // Second injection with different session name — should replace, not duplicate
        inject_bootstrap_block(tmp.path(), "claude-code", "proj-v2-orchestrator").unwrap();
        let after_second = std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();
        assert_eq!(after_second.matches(BOOTSTRAP_MARKER_START).count(), 1);
        assert_eq!(after_second.matches(BOOTSTRAP_MARKER_END).count(), 1);
        // Old session name gone, new one present
        assert!(!after_second.contains("proj-orchestrator"));
        assert!(after_second.contains("proj-v2-orchestrator"));
        // Original content still there
        assert!(after_second.contains("# My Project"));
    }

    #[test]
    fn test_inject_bootstrap_gemini_provider() {
        let tmp = tempfile::TempDir::new().unwrap();
        let result = inject_bootstrap_block(
            tmp.path(),
            "gemini-cli",
            "proj-orchestrator",
        )
        .unwrap();

        assert_eq!(result, "GEMINI.md");

        let content = std::fs::read_to_string(tmp.path().join("GEMINI.md")).unwrap();
        assert!(content.contains(BOOTSTRAP_MARKER_START));
        assert!(content.contains(".gemini/commands/squad-orchestrator.toml"));
    }

    #[test]
    fn test_inject_bootstrap_preserves_surrounding_content() {
        let tmp = tempfile::TempDir::new().unwrap();

        // File with content before and after where the block will be inserted
        let initial = format!(
            "# Header\n\n{}\nold bootstrap content\n{}\n\n# Footer\n",
            BOOTSTRAP_MARKER_START, BOOTSTRAP_MARKER_END,
        );
        std::fs::write(tmp.path().join("CLAUDE.md"), &initial).unwrap();

        inject_bootstrap_block(tmp.path(), "claude-code", "new-orch").unwrap();
        let content = std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap();

        assert!(content.contains("# Header"));
        assert!(content.contains("# Footer"));
        assert!(content.contains("new-orch"));
        assert!(!content.contains("old bootstrap content"));
        assert_eq!(content.matches(BOOTSTRAP_MARKER_START).count(), 1);
    }
}
