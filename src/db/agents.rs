use sqlx::SqlitePool;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub tool: String, // AGNT-03: renamed from provider
    pub role: String,
    #[allow(dead_code)]
    pub command: Option<String>, // legacy column; schema has NOT NULL but we ignore value
    pub created_at: String,
    pub status: String,
    pub status_updated_at: String,
    pub model: Option<String>,        // AGNT-01
    pub description: Option<String>,  // AGNT-01
    pub current_task: Option<String>, // AGNT-02: FK to messages.id
}

pub async fn insert_agent(
    pool: &SqlitePool,
    name: &str,
    tool: &str,
    role: &str,
    model: Option<&str>,
    description: Option<&str>,
) -> anyhow::Result<()> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT OR IGNORE INTO agents (id, name, tool, role, command, model, description, created_at, status_updated_at) \
         VALUES (?, ?, ?, ?, '', ?, ?, ?, ?)"
    )
    .bind(id)
    .bind(name)
    .bind(tool)
    .bind(role)
    // command = '' — legacy column; value is empty string placeholder
    .bind(model)
    .bind(description)
    .bind(&now)
    .bind(&now) // status_updated_at — consistent RFC3339 format
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_agent(pool: &SqlitePool, name: &str) -> anyhow::Result<Option<Agent>> {
    let agent = sqlx::query_as::<_, Agent>("SELECT * FROM agents WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await?;
    Ok(agent)
}

pub async fn list_agents(pool: &SqlitePool) -> anyhow::Result<Vec<Agent>> {
    let agents = sqlx::query_as::<_, Agent>("SELECT * FROM agents ORDER BY name")
        .fetch_all(pool)
        .await?;
    Ok(agents)
}

/// Find the orchestrator agent (role = 'orchestrator') for notification purposes.
pub async fn get_orchestrator(pool: &SqlitePool) -> anyhow::Result<Option<Agent>> {
    let agent =
        sqlx::query_as::<_, Agent>("SELECT * FROM agents WHERE role = 'orchestrator' LIMIT 1")
            .fetch_optional(pool)
            .await?;
    Ok(agent)
}

/// Update agent lifecycle status. Valid values: "idle" | "busy" | "dead"
pub async fn update_agent_status(
    pool: &SqlitePool,
    name: &str,
    status: &str,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE agents SET status = ?, status_updated_at = ? WHERE name = ?")
        .bind(status)
        .bind(&now)
        .bind(name)
        .execute(pool)
        .await?;
    Ok(())
}
