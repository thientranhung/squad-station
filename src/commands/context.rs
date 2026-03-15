use crate::{config, db};
use crate::config::SddConfig;
use crate::db::agents::Agent;

/// Provider-specific context file name for the orchestrator.
/// This file is placed inside `.squad/orchestrator/` and auto-loaded by the AI tool.
fn orchestrator_context_filename(provider: &str) -> &'static str {
    match provider {
        "claude-code" => "CLAUDE.md",
        "gemini-cli"  => "GEMINI.md",
        _             => "CLAUDE.md", // fallback
    }
}

pub fn build_orchestrator_md(agents: &[Agent], project_root: &str, sdd_configs: &[SddConfig]) -> String {
    let mut out = String::new();

    out.push_str("# Squad Orchestrator Playbook\n\n");
    out.push_str("> BEHAVIORAL RULE: You are an orchestrator. Do not implement tasks yourself.\n");
    out.push_str("> Delegate to agents using `squad-station send`. Wait for completion signals.\n\n");
    out.push_str(&format!("## Project\n\nCodebase at: `{}`\n\n", project_root));

    // ── Delegation Workflow ──────────────────────────────────────────────────
    out.push_str("## Delegation Workflow\n\n");
    out.push_str("### Registered Agents\n\n");

    for agent in agents {
        if agent.role == "orchestrator" {
            continue;
        }
        let display_model = agent.model.as_deref().unwrap_or(&agent.tool);
        out.push_str(&format!("### {} ({})\n", agent.name, display_model));
        if let Some(ref desc) = agent.description {
            out.push_str(&format!("{}\n", desc));
        }
        out.push_str(&format!("Role: {}\n\n", agent.role));
        out.push_str("```\n");
        out.push_str(&format!("squad-station send {} --body \"...\"\n", agent.name));
        out.push_str(&format!("tmux capture-pane -t {} -p\n", agent.name));
        out.push_str("```\n\n");
    }

    out.push_str("### How to Delegate\n\n");
    out.push_str("1. Select agent based on task type\n");
    out.push_str("2. `squad-station send <agent> --body \"<task>\"`\n");
    out.push_str("3. After sending, wait for the completion hook signal\n");
    out.push_str("4. Read output: `tmux capture-pane -t <agent> -p`\n");
    out.push_str("5. Verify: `squad-station list --agent <agent>`\n");
    out.push_str("6. Parallel dispatch: send to multiple agents simultaneously only when tasks are independent\n\n");

    // ── Monitoring Workflow ──────────────────────────────────────────────────
    out.push_str("## Monitoring Workflow\n\n");
    out.push_str("> BEHAVIORAL RULE: Poll, don't push. Agents signal DB on completion.\n");
    out.push_str("> Wait for hook signals rather than polling continuously.\n\n");
    out.push_str("### How to Poll\n\n");
    out.push_str("Check all agent statuses:\n");
    out.push_str("```\n");
    out.push_str("squad-station agents\n");
    out.push_str("```\n\n");
    out.push_str("Check pending/completed messages:\n");
    out.push_str("```\n");
    out.push_str("squad-station list --limit 20\n");
    out.push_str("```\n\n");
    out.push_str("Read agent output:\n");
    out.push_str("```\n");
    out.push_str("tmux capture-pane -t <agent-name> -p\n");
    out.push_str("```\n\n");
    out.push_str("### Anti-Context-Decay Rules\n\n");
    out.push_str("- This file is auto-loaded on every conversation start — your context survives compression\n");
    out.push_str("- Check `squad-station status` for current state\n");
    out.push_str("- Never assume a task is done — verify with `squad-station list`\n");
    out.push_str("- Full context transfer when handing off between agents — never summarize to a few lines\n\n");

    // ── Workflow (SDD) ────────────────────────────────────────────────────────
    if !sdd_configs.is_empty() {
        out.push_str("## Workflow (SDD)\n\n");
        out.push_str("> IMPORTANT: Read the ENTIRE playbook before starting any task. Follow it exactly.\n\n");
        for sdd in sdd_configs {
            out.push_str("### ");
            out.push_str(&sdd.name);
            out.push_str("\nPlaybook: `");
            out.push_str(&sdd.playbook);
            out.push_str("`\n\nLoad into context: `cat \"");
            out.push_str(&sdd.playbook);
            out.push_str("\"`\n\n");
        }
        // Only show selection rule if multiple SDDs are configured
        if sdd_configs.len() > 1 {
            out.push_str("**Selection rule:** Choose the workflow that fits the task scope.\n");
            for sdd in sdd_configs {
                out.push_str(&format!("- `{}` → see playbook for when to use\n", sdd.name));
            }
            out.push_str("\n");
        }
    }

    // ── Agent Roster ─────────────────────────────────────────────────────────
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

    // ── Principles ─────────────────────────────────────────────────────────
    out.push_str("\n## Principles\n\n");
    out.push_str("1. Read the SDD playbook before starting any task\n");
    out.push_str("2. Don't code yourself — delegate to agents with the right role\n");
    out.push_str("3. Only ask the user (HITL) when a business decision is truly needed\n");
    out.push_str("4. After an agent finishes → read results → decide next step\n");
    out.push_str("5. Send a summary report to the user when the workflow is complete\n");

    out
}

pub async fn run() -> anyhow::Result<()> {
    // 1. Find project root and connect to DB
    let project_root = config::find_project_root()?;
    let config = config::load_config(&project_root.join("squad.yml"))?;
    let db_path = config::resolve_db_path(&config)?;
    let pool = db::connect(&db_path).await?;

    // 2. Fetch agents — read-only, no tmux reconciliation
    let agents = db::agents::list_agents(&pool).await?;

    // 3. Orchestrator context → .squad/orchestrator/<PROVIDER_FILE>
    let orch_dir = project_root.join(".squad").join("orchestrator");
    std::fs::create_dir_all(&orch_dir)?;

    let context_filename = orchestrator_context_filename(&config.orchestrator.provider);
    let context_path = orch_dir.join(context_filename);

    let project_root_str = project_root.to_string_lossy().to_string();
    let sdd_configs = config.sdd.as_deref().unwrap_or(&[]);
    let orchestrator_content = build_orchestrator_md(&agents, &project_root_str, sdd_configs);
    std::fs::write(&context_path, &orchestrator_content)?;

    // 4. Print summary
    println!("Generated {}", context_path.display());

    Ok(())
}
