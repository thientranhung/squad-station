use crate::{config, db};
use crate::db::agents::Agent;

fn build_playbook_content(agents: &[Agent]) -> String {
    let mut out = String::new();
    out.push_str("Your Role: \"AI Project Manager & Principal Tech Lead\". You DO NOT write code or modify files directly. You ORCHESTRATE, MONITOR QUALITY, and EVALUATE worker agents on my behalf.\n\n");
    
    out.push_str("### 1. PRE-FLIGHT (MANDATORY)\n");
    out.push_str("- Read the project documentation (e.g., `GEMINI.md`, `README.md`, or architecture docs) to understand the tech stack and guidelines.\n");
    out.push_str("- Do not proceed to assign tasks until you fully understand the current project context.\n\n");
    
    out.push_str("### 2. AVAILABLE WORKER AGENTS\n");
    for agent in agents {
        if agent.role == "orchestrator" {
            continue;
        }
        let model = agent.model.as_deref().unwrap_or(&agent.tool);
        out.push_str(&format!("**{}** (Tool: {}, Model: {})\n", agent.name, agent.tool, model));
        if let Some(ref desc) = agent.description {
            out.push_str(&format!("- Description: {}\n", desc));
        }
        out.push_str(&format!("- Delegate: `squad-station send {} --body \"<task>\"`\n", agent.name));
        out.push_str(&format!("- Read Output: `tmux capture-pane -t {} -p -S -`\n\n", agent.name));
    }
    
    out.push_str("### 3. CORE RULES OF ENGAGEMENT\n");
    out.push_str("- **Delegation:** Always use `squad-station send` to assign work. It automatically handles tmux injection.\n");
    out.push_str("- **Wait for Completion:** Do not poll continuously or interrupt. Wait until the agent finishes (check via `squad-station list` or wait for notification).\n");
    out.push_str("- **Context Handoff (CRITICAL):** When transferring work between agents, you MUST capture the FULL output of the completed agent using `tmux capture-pane ... -p -S -` and include it entirely in the next prompt. DO NOT summarize it sparsely.\n");
    out.push_str("- **Execution Discipline:** Never interrupt an agent mid-task. Wait for their final output before evaluating and deciding the next step.\n");
    out.push_str("- **Quality Assurance:** Verify the agent's output against project documents. If they make a technical mistake within the known scope, correct them. Only forward business-level decisions back to the user.\n");

    out
}

pub async fn run() -> anyhow::Result<()> {
    let config = config::load_config(std::path::Path::new("squad.yml"))?;
    let db_path = config::resolve_db_path(&config)?;
    let pool = db::connect(&db_path).await?;
    let agents = db::agents::list_agents(&pool).await?;

    let content = build_playbook_content(&agents);
    let provider = config.orchestrator.provider.as_str();

    // Clean up old fragmented context files if they exist
    let _ = std::fs::remove_file(".agent/workflows/squad-delegate.md");
    let _ = std::fs::remove_file(".agent/workflows/squad-monitor.md");
    let _ = std::fs::remove_file(".agent/workflows/squad-roster.md");

    let (path_str, final_content) = match provider {
        "gemini-cli" => {
            std::fs::create_dir_all(".gemini/commands")?;
            let toml = format!(
                "description = \"Squad Orchestrator Playbook\"\nprompt = \"\"\"\n{}\n\"\"\"\n",
                content
            );
            (".gemini/commands/squad-orchestrator.toml", toml)
        }
        "antigravity" => {
            std::fs::create_dir_all(".agent/workflows")?;
            let md = format!(
                "---\ndescription: Squad Orchestrator Playbook\n---\n\n{}",
                content
            );
            (".agent/workflows/squad-orchestrator.md", md)
        }
        _ => {
            std::fs::create_dir_all(".agent/workflows")?;
            let md = format!(
                "# Squad Orchestrator Playbook\n\n{}",
                content
            );
            (".agent/workflows/squad-orchestrator.md", md)
        }
    };

    std::fs::write(path_str, final_content)?;
    println!("Generated {}", path_str);

    Ok(())
}
