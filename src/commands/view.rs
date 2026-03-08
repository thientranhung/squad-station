use crate::{config, db, tmux};

pub async fn run(json: bool) -> anyhow::Result<()> {
    // 1. Load config + connect
    let config = config::load_config(std::path::Path::new("squad.yml"))?;
    let db_path = config::resolve_db_path(&config)?;
    let pool = db::connect(&db_path).await?;

    // 2. Fetch agents
    let agents = db::agents::list_agents(&pool).await?;

    // 3. Get live tmux sessions
    let live_sessions = tmux::list_live_session_names();

    // 4. Filter: keep only agents whose name appears in live sessions
    let live_agent_names: Vec<String> = agents
        .iter()
        .filter(|a| live_sessions.contains(&a.name))
        .map(|a| a.name.clone())
        .collect();

    if live_agent_names.is_empty() {
        if json {
            println!(r#"{{"message":"No live agent sessions to display."}}"#);
        } else {
            println!("No live agent sessions to display.");
        }
        return Ok(());
    }

    let n = live_agent_names.len();

    // 5. Kill existing squad-view window (idempotent)
    tmux::kill_window("squad-view")?;

    // 6. Create new tiled view window
    tmux::create_view_window("squad-view", &live_agent_names)?;

    if json {
        println!(
            r#"{{"message":"Created squad-view with {} panes","panes":{}}}"#,
            n, n
        );
    } else {
        println!("Created squad-view with {} panes", n);
    }

    Ok(())
}
