use crate::{config, db};
use crate::db::agents::Agent;

pub fn build_orchestrator_md(agents: &[Agent]) -> String {
    let mut out = String::new();

    out.push_str("# Squad Orchestrator Playbook\n\n");
    out.push_str("> BEHAVIORAL RULE: You are an orchestrator. Do not implement tasks yourself.\n");
    out.push_str("> Delegate to agents using `squad-station send`. Wait for completion signals.\n");
    out.push_str("> These rules survive context compression — re-read this file if context resets.\n\n");

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
    out.push_str("- If you lose context, re-read `.agent/workflows/squad-orchestrator.md`\n");
    out.push_str("- Check `squad-station status` for current state\n");
    out.push_str("- Never assume a task is done — verify with `squad-station list`\n");
    out.push_str("- Full context transfer when handing off between agents — never summarize to a few lines\n\n");

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

    out
}

pub async fn run() -> anyhow::Result<()> {
    // 1. Connect to DB
    let config = config::load_config(std::path::Path::new("squad.yml"))?;
    let db_path = config::resolve_db_path(&config)?;
    let pool = db::connect(&db_path).await?;

    // 2. Fetch agents — read-only, no tmux reconciliation
    let agents = db::agents::list_agents(&pool).await?;

    // 3. Create directory
    std::fs::create_dir_all(".agent/workflows")?;

    // 4. Write single unified file
    let orchestrator_content = build_orchestrator_md(&agents);
    std::fs::write(".agent/workflows/squad-orchestrator.md", orchestrator_content)?;

    // 5. Print summary
    println!("Generated .agent/workflows/squad-orchestrator.md");

    Ok(())
}
