use sqlx::SqlitePool;

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct Message {
    pub id: String,
    pub agent_name: String,
    pub task: String,
    pub status: String,
    pub priority: String,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn insert_message(
    pool: &SqlitePool,
    agent_name: &str,
    task: &str,
    priority: &str,
) -> anyhow::Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO messages (id, agent_name, task, status, priority, created_at, updated_at) VALUES (?, ?, ?, 'pending', ?, ?, ?)"
    )
    .bind(&id)
    .bind(agent_name)
    .bind(task)
    .bind(priority)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(id)
}

/// Mark the most recent pending message for this agent as completed.
/// Returns the number of rows affected (0 = already completed, not an error — MSG-03 idempotency).
///
/// Uses a subquery to identify the target row because SQLite does not support
/// `UPDATE ... ORDER BY ... LIMIT` without a compile-time flag (SQLITE_ENABLE_UPDATE_DELETE_LIMIT).
pub async fn update_status(pool: &SqlitePool, agent_name: &str) -> anyhow::Result<u64> {
    let now = chrono::Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE messages SET status = 'completed', updated_at = ? \
         WHERE id = (\
           SELECT id FROM messages \
           WHERE agent_name = ? AND status = 'pending' \
           ORDER BY created_at DESC LIMIT 1\
         )"
    )
    .bind(&now)
    .bind(agent_name)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn list_messages(
    pool: &SqlitePool,
    agent: Option<&str>,
    status: Option<&str>,
    limit: u32,
) -> anyhow::Result<Vec<Message>> {
    // Build query dynamically with optional WHERE clauses
    let mut query = String::from("SELECT * FROM messages");
    let mut conditions = Vec::new();
    if agent.is_some() {
        conditions.push("agent_name = ?");
    }
    if status.is_some() {
        conditions.push("status = ?");
    }
    if !conditions.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&conditions.join(" AND "));
    }
    query.push_str(" ORDER BY created_at DESC LIMIT ?");

    let mut q = sqlx::query_as::<_, Message>(&query);
    if let Some(a) = agent {
        q = q.bind(a);
    }
    if let Some(s) = status {
        q = q.bind(s);
    }
    q = q.bind(limit);

    let messages = q.fetch_all(pool).await?;
    Ok(messages)
}

/// Peek at the highest-priority pending message for an agent.
/// Priority ordering: urgent > high > normal.
pub async fn peek_message(pool: &SqlitePool, agent_name: &str) -> anyhow::Result<Option<Message>> {
    let message = sqlx::query_as::<_, Message>(
        "SELECT * FROM messages WHERE agent_name = ? AND status = 'pending' ORDER BY CASE priority WHEN 'urgent' THEN 1 WHEN 'high' THEN 2 ELSE 3 END, created_at ASC LIMIT 1"
    )
    .bind(agent_name)
    .fetch_optional(pool)
    .await?;
    Ok(message)
}
