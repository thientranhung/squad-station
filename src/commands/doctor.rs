use crate::{config, db};

pub async fn run() -> anyhow::Result<()> {
    let project_root = config::find_project_root()?;
    let cfg = config::load_config(&project_root.join("squad.yml"))?;
    let db_path = config::resolve_db_path(&cfg)?;

    // Ensure DB exists and migrations are applied before checking
    let _pool = db::connect(&db_path).await?;

    let orch_role = cfg
        .orchestrator
        .name
        .as_deref()
        .unwrap_or("orchestrator");
    let orch_name = config::sanitize_session_name(&format!("{}-{}", cfg.project, orch_role));

    let failures = super::init::run_health_check(&cfg, &db_path, &orch_name);

    if failures > 0 {
        std::process::exit(1);
    }
    Ok(())
}
