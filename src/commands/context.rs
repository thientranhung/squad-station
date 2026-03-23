use crate::config::SddConfig;
use crate::db::agents::Agent;
use crate::{config, db};

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

    // ── Autonomous Mode ─────────────────────────────────────────────────
    out.push_str("## Autonomous Mode — DO NOT stop for trivial decisions\n\n");
    out.push_str("You operate autonomously. Your job is to drive tasks to COMPLETION without stopping.\n\n");
    out.push_str("### Decision Authority\n\n");
    out.push_str("You MUST make these decisions yourself — NEVER ask the user:\n");
    out.push_str("- Which agent to assign a task to (use Session Routing rules)\n");
    out.push_str("- File naming, code structure, and implementation approach\n");
    out.push_str("- How to break down a task into subtasks\n");
    out.push_str("- Whether to run tests, linting, or formatting\n");
    out.push_str("- How to fix build errors, test failures, or lint warnings\n");
    out.push_str("- When to `/clear` an agent's context\n");
    out.push_str("- Ordering and parallelization of independent tasks\n");
    out.push_str("- Technical trade-offs where both options are reasonable\n\n");
    out.push_str("### When to Escalate to User (ONLY these cases)\n\n");
    out.push_str("- **Ambiguous requirements** — the user's intent is genuinely unclear and choosing wrong would waste significant effort\n");
    out.push_str("- **Destructive actions** — deleting data, force-pushing, dropping tables, removing features\n");
    out.push_str("- **External dependencies** — need API keys, credentials, third-party service access\n");
    out.push_str("- **Scope conflict** — the task contradicts existing architecture or another user request\n\n");
    out.push_str("### Driving to Completion\n\n");
    out.push_str("- When an agent completes a task, immediately evaluate the output and dispatch the NEXT step\n");
    out.push_str("- If an agent encounters an error, analyze the error and send a follow-up task with the fix — do NOT ask the user what to do\n");
    out.push_str("- If an agent asks a technical question, answer it yourself based on project context and send the answer back\n");
    out.push_str("- Keep delegating until the ENTIRE user request is fulfilled, not just the first step\n");
    out.push_str("- After all tasks are done, verify the work (run tests, check build) before reporting completion to the user\n\n");

    // ── PRE-FLIGHT ───────────────────────────────────────────────────────
    out.push_str("## PRE-FLIGHT — Execute IMMEDIATELY before any task\n\n");
    if !sdd_configs.is_empty() {
        out.push_str("> Read and fully internalize the SDD playbook(s). Do not skip or skim.\n\n");
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
    out.push_str("Only proactively check (`capture-pane`) if you suspect the agent is stuck for an unusually long time.\n\n");

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
    out.push_str("3. If agent asked technical questions → answer them yourself using project context and send the answer back\n");
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
