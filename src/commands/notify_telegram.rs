use crate::config;

/// Load credentials from a simple KEY=VALUE env file (like .env.squad).
/// Skips comments (#) and empty lines. Returns (key, value) pairs.
fn load_env_file(path: &std::path::Path) -> Vec<(String, String)> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return vec![];
    };
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

/// Detect the current tmux session name via `tmux display-message -p '#S'`.
fn detect_tmux_session() -> Option<String> {
    let output = std::process::Command::new("tmux")
        .args(["display-message", "-p", "#S"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// Check if the agent (tmux session name) matches the notify_agents filter.
fn agent_matches_filter(agent_name: &str, notify_agents: &config::NotifyAgents) -> bool {
    match notify_agents {
        config::NotifyAgents::All(s) if s == "all" => true,
        config::NotifyAgents::All(_) => false,
        config::NotifyAgents::List(agents) => agents.iter().any(|a| {
            let a = a.trim();
            agent_name == a || agent_name.ends_with(&format!("-{a}"))
        }),
    }
}

/// Format the notification message based on the hook event type.
fn format_message(
    event: &str,
    raw_message: &str,
    project_name: &str,
    transcript: &Option<String>,
) -> String {
    let msg = if raw_message.is_empty() || raw_message == "null" {
        "Notification"
    } else {
        raw_message
    };

    let formatted = match event {
        "SessionStart" => format!("<b>[{project_name}]</b> Session started \u{1f680}"),
        "SessionEnd" => format!("<b>[{project_name}]</b> Session completed \u{2705}"),
        "Stop" => {
            // Try to extract last assistant message from transcript
            let transcript_msg = transcript.as_deref().and_then(extract_transcript_message);
            if let Some(ref t) = transcript_msg {
                format!("<b>[{project_name}]</b> \u{1f3c1} {t}")
            } else if msg != "Notification" {
                format!("<b>[{project_name}]</b> \u{1f3c1} {msg}")
            } else {
                format!("<b>[{project_name}]</b> Response finished \u{1f3c1}")
            }
        }
        "Notification" => format!("<b>[{project_name}]</b> {msg}"),
        _ => format!("<b>[{project_name}]</b> {event}: {msg}"),
    };

    // Telegram 4096 char limit
    if formatted.len() > 4096 {
        format!("{}... <i>(truncated)</i>", &formatted[..4080])
    } else {
        formatted
    }
}

/// Extract the last assistant text message from a Claude transcript JSONL file.
fn extract_transcript_message(path: &str) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    content
        .lines()
        .rev()
        .find(|line| line.contains("\"type\":\"assistant\""))
        .and_then(|line| {
            let v: serde_json::Value = serde_json::from_str(line).ok()?;
            v["message"]["content"]
                .as_array()?
                .iter()
                .find(|c| c["type"].as_str() == Some("text"))
                .and_then(|c| c["text"].as_str().map(|s| s.to_string()))
        })
}

/// Send a message to the Telegram Bot API.
async fn send_telegram(
    token: &str,
    chat_id: &str,
    topic_id: Option<&str>,
    text: &str,
) -> anyhow::Result<()> {
    let url = format!("https://api.telegram.org/bot{token}/sendMessage");

    let mut payload = serde_json::json!({
        "chat_id": chat_id,
        "text": text,
        "parse_mode": "HTML",
        "disable_web_page_preview": true
    });

    if let Some(tid) = topic_id {
        if !tid.is_empty() {
            if let Ok(n) = tid.parse::<i64>() {
                payload["message_thread_id"] = serde_json::json!(n);
            }
        }
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    // Fire-and-forget style: log errors but don't fail
    match client.post(&url).json(&payload).send().await {
        Ok(resp) if !resp.status().is_success() => {
            eprintln!(
                "squad-station: notify-telegram: API returned {}",
                resp.status()
            );
        }
        Err(e) => {
            eprintln!("squad-station: notify-telegram: request failed: {e}");
        }
        _ => {}
    }

    Ok(())
}

pub async fn run(
    event: String,
    message: String,
    project: Option<String>,
    transcript: Option<String>,
) -> anyhow::Result<()> {
    // 1. Load squad.yml to get telegram config
    let config_path = std::path::Path::new("squad.yml");
    let config = match config::load_config(config_path) {
        Ok(c) => c,
        Err(_) => return Ok(()), // no config → silent exit
    };

    // Guard: telegram must be enabled
    let telegram = match &config.telegram {
        Some(t) if t.enabled => t,
        _ => return Ok(()),
    };

    // 2. Load credentials from .env.squad
    let project_root = std::env::current_dir().unwrap_or_default();
    let env_vars = load_env_file(&project_root.join(".env.squad"));

    let token = env_vars
        .iter()
        .find(|(k, _)| k == "TELE_TOKEN")
        .map(|(_, v)| v.as_str())
        .unwrap_or("");
    let chat_id = env_vars
        .iter()
        .find(|(k, _)| k == "TELE_CHAT_ID")
        .map(|(_, v)| v.as_str())
        .unwrap_or("");
    let topic_id = env_vars
        .iter()
        .find(|(k, _)| k == "TELE_TOPIC_ID")
        .map(|(_, v)| v.as_str());

    // Guard: credentials required
    if token.is_empty() || chat_id.is_empty() {
        return Ok(());
    }

    // 3. Detect agent (tmux session name) and apply filter
    let agent_name = detect_tmux_session();

    // If notify_agents is not "all", require a tmux session and check against the list
    if !matches!(&telegram.notify_agents, config::NotifyAgents::All(s) if s == "all") {
        let Some(ref name) = agent_name else {
            return Ok(()); // not in tmux → not an agent → skip
        };
        if !agent_matches_filter(name, &telegram.notify_agents) {
            return Ok(());
        }
    }

    // 4. Determine project name
    let project_name = project.unwrap_or_else(|| {
        project_root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| config.project.clone())
    });

    // 5. Format and send
    let text = format_message(&event, &message, &project_name, &transcript);
    send_telegram(token, chat_id, topic_id, &text).await?;

    Ok(())
}

// ── Public helpers for testing ─────────────────────────────────────────────

pub fn load_env_file_pub(path: &std::path::Path) -> Vec<(String, String)> {
    load_env_file(path)
}

pub fn agent_matches_filter_pub(agent_name: &str, notify_agents: &config::NotifyAgents) -> bool {
    agent_matches_filter(agent_name, notify_agents)
}

pub fn format_message_pub(
    event: &str,
    raw_message: &str,
    project_name: &str,
    transcript: &Option<String>,
) -> String {
    format_message(event, raw_message, project_name, transcript)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_env_file_parses_key_value() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            tmp.path(),
            "# comment\nTELE_TOKEN=abc123\nTELE_CHAT_ID=-100\nTELE_TOPIC_ID=42\n",
        )
        .unwrap();
        let vars = load_env_file(tmp.path());
        assert_eq!(vars.len(), 3);
        assert_eq!(vars[0], ("TELE_TOKEN".into(), "abc123".into()));
        assert_eq!(vars[1], ("TELE_CHAT_ID".into(), "-100".into()));
        assert_eq!(vars[2], ("TELE_TOPIC_ID".into(), "42".into()));
    }

    #[test]
    fn test_load_env_file_skips_comments_and_empty() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "# comment\n\nKEY=val\n").unwrap();
        let vars = load_env_file(tmp.path());
        assert_eq!(vars.len(), 1);
    }

    #[test]
    fn test_load_env_file_missing_file() {
        let vars = load_env_file(std::path::Path::new("/nonexistent/.env.squad"));
        assert!(vars.is_empty());
    }

    #[test]
    fn test_agent_matches_filter_all() {
        let filter = config::NotifyAgents::All("all".to_string());
        assert!(agent_matches_filter("any-agent", &filter));
    }

    #[test]
    fn test_agent_matches_filter_list_exact() {
        let filter = config::NotifyAgents::List(vec!["orchestrator".into(), "implement".into()]);
        assert!(agent_matches_filter("implement", &filter));
        assert!(!agent_matches_filter("brainstorm", &filter));
    }

    #[test]
    fn test_agent_matches_filter_list_suffix() {
        let filter = config::NotifyAgents::List(vec!["orchestrator".into()]);
        assert!(agent_matches_filter("myproject-orchestrator", &filter));
        assert!(!agent_matches_filter("myproject-worker", &filter));
    }

    #[test]
    fn test_agent_matches_filter_non_all_string() {
        let filter = config::NotifyAgents::All("none".to_string());
        assert!(!agent_matches_filter("anything", &filter));
    }

    #[test]
    fn test_format_message_stop_default() {
        let msg = format_message("Stop", "", "myproject", &None);
        assert!(msg.contains("[myproject]"));
        assert!(msg.contains("Response finished"));
    }

    #[test]
    fn test_format_message_stop_with_body() {
        let msg = format_message("Stop", "Task done", "proj", &None);
        assert!(msg.contains("Task done"));
        assert!(msg.contains("\u{1f3c1}"));
    }

    #[test]
    fn test_format_message_session_start() {
        let msg = format_message("SessionStart", "", "proj", &None);
        assert!(msg.contains("Session started"));
        assert!(msg.contains("\u{1f680}"));
    }

    #[test]
    fn test_format_message_session_end() {
        let msg = format_message("SessionEnd", "", "proj", &None);
        assert!(msg.contains("Session completed"));
    }

    #[test]
    fn test_format_message_notification() {
        let msg = format_message("Notification", "hello", "proj", &None);
        assert_eq!(msg, "<b>[proj]</b> hello");
    }

    #[test]
    fn test_format_message_unknown_event() {
        let msg = format_message("CustomEvent", "data", "proj", &None);
        assert_eq!(msg, "<b>[proj]</b> CustomEvent: data");
    }

    #[test]
    fn test_format_message_truncation() {
        let long = "x".repeat(5000);
        let msg = format_message("Notification", &long, "proj", &None);
        assert!(msg.len() <= 4096 + 30); // truncation + suffix
        assert!(msg.contains("(truncated)"));
    }

    #[test]
    fn test_format_message_null_treated_as_notification() {
        let msg = format_message("Notification", "null", "proj", &None);
        assert_eq!(msg, "<b>[proj]</b> Notification");
    }

    #[test]
    fn test_extract_transcript_message() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            tmp.path(),
            r#"{"type":"user","message":"hello"}
{"type":"assistant","message":{"content":[{"type":"text","text":"I completed the task"}]}}
"#,
        )
        .unwrap();
        let result = extract_transcript_message(tmp.path().to_str().unwrap());
        assert_eq!(result.unwrap(), "I completed the task");
    }

    #[test]
    fn test_extract_transcript_missing_file() {
        assert!(extract_transcript_message("/nonexistent/file").is_none());
    }
}
