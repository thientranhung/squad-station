use crate::config;

/// Strip matching surrounding quotes (single or double) from a string.
fn strip_quotes(s: &str) -> &str {
    let s = s.trim();
    if s.len() >= 2 {
        if (s.starts_with('"') && s.ends_with('"'))
            || (s.starts_with('\'') && s.ends_with('\''))
        {
            return &s[1..s.len() - 1];
        }
    }
    s
}

/// Load credentials from a simple KEY=VALUE env file (like .env.squad).
/// Skips comments (#) and empty lines. Returns (key, value) pairs.
/// Strips surrounding quotes from values (both single and double).
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
            Some((
                key.trim().to_string(),
                strip_quotes(value.trim()).to_string(),
            ))
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

/// Read hook input JSON from stdin (Claude Code passes this to hook scripts).
/// Returns parsed JSON value, or None if stdin is empty/invalid/TTY.
/// Uses a background thread with timeout to prevent blocking if the pipe never closes.
fn read_hook_input() -> Option<serde_json::Value> {
    use std::io::Read;

    // If stdin is a terminal (not a pipe), skip — no hook input available
    if unsafe { libc::isatty(libc::STDIN_FILENO) } != 0 {
        return None;
    }

    // Read stdin on a background thread with a short timeout.
    // Claude Code pipes hook input JSON immediately, so data is available right away.
    // If nothing arrives within 100ms, stdin is empty or the pipe won't close.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = String::new();
        let _ = std::io::stdin().lock().read_to_string(&mut buf);
        let _ = tx.send(buf);
    });

    match rx.recv_timeout(std::time::Duration::from_millis(100)) {
        Ok(buf) if !buf.trim().is_empty() => serde_json::from_str(&buf).ok(),
        _ => None,
    }
}

/// Extract the last assistant text message from a Claude Code JSONL transcript file.
/// Each line is a JSON object; we find the last one with `"type":"assistant"` and
/// extract `.message.content[].text` where content type is "text".
fn read_last_assistant_message(transcript_path: &str) -> Option<String> {
    let content = std::fs::read_to_string(transcript_path).ok()?;
    // Find the last line containing "type":"assistant"
    let last_assistant_line = content
        .lines()
        .rev()
        .find(|line| line.contains("\"type\":\"assistant\""))?;
    let json: serde_json::Value = serde_json::from_str(last_assistant_line).ok()?;
    // Extract text from .message.content[] where type == "text"
    let contents = json.get("message")?.get("content")?.as_array()?;
    for item in contents {
        if item.get("type")?.as_str()? == "text" {
            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
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

/// Escape HTML special characters for Telegram HTML parse mode.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Format the notification message with optional transcript context.
fn format_message(project_name: &str, agent_name: Option<&str>, transcript: Option<&str>) -> String {
    let header = match agent_name {
        Some(name) => format!("<b>[{project_name}]</b> {name} finished \u{1f3c1}"),
        None => format!("<b>[{project_name}]</b> Agent finished \u{1f3c1}"),
    };

    let formatted = match transcript {
        Some(t) if !t.is_empty() => {
            let escaped = escape_html(t);
            format!("{header}\n\n<pre>{escaped}</pre>")
        }
        _ => header,
    };

    // Telegram 4096 char limit
    if formatted.len() > 4096 {
        format!("{}... <i>(truncated)</i>", &formatted[..4080])
    } else {
        formatted
    }
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
            let status = resp.status();
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| "<unreadable>".to_string());
            eprintln!(
                "squad-station: notify-telegram: API returned {} — {}",
                status, body
            );
        }
        Err(e) => {
            eprintln!("squad-station: notify-telegram: request failed: {e}");
        }
        Ok(_) => {
            eprintln!("squad-station: notify-telegram: message sent successfully");
        }
    }

    Ok(())
}

pub async fn run(
    project: Option<String>,
    project_root_arg: Option<std::path::PathBuf>,
) -> anyhow::Result<()> {
    // 1. Resolve project root: use --project-root if provided, otherwise CWD
    let project_root = match project_root_arg {
        Some(p) => p,
        None => std::env::current_dir().unwrap_or_default(),
    };

    // 2. Load squad.yml to get telegram config
    let config_path = project_root.join("squad.yml");
    let config = match config::load_config(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("squad-station: notify-telegram: cannot load squad.yml: {e}");
            return Ok(());
        }
    };

    // Guard: telegram must be enabled
    let telegram = match &config.telegram {
        Some(t) if t.enabled => t,
        Some(_) => {
            eprintln!("squad-station: notify-telegram: telegram.enabled is false, skipping");
            return Ok(());
        }
        None => {
            eprintln!("squad-station: notify-telegram: no [telegram] section in squad.yml, skipping");
            return Ok(());
        }
    };

    // 3. Load credentials from .env.squad
    let env_path = project_root.join(".env.squad");
    let env_vars = load_env_file(&env_path);

    if env_vars.is_empty() {
        eprintln!(
            "squad-station: notify-telegram: no variables loaded from {}",
            env_path.display()
        );
    }

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
        eprintln!(
            "squad-station: notify-telegram: missing credentials (TELE_TOKEN={}, TELE_CHAT_ID={})",
            if token.is_empty() { "<empty>" } else { "<set>" },
            if chat_id.is_empty() { "<empty>" } else { "<set>" },
        );
        return Ok(());
    }

    // 4. Detect agent (tmux session name) and apply filter
    let agent_name = detect_tmux_session();

    // If notify_agents is not "all", require a tmux session and check against the list
    if !matches!(&telegram.notify_agents, config::NotifyAgents::All(s) if s == "all") {
        let Some(ref name) = agent_name else {
            eprintln!(
                "squad-station: notify-telegram: not inside a tmux session; \
                 notify_agents filter requires a tmux session name to match against"
            );
            return Ok(());
        };
        if !agent_matches_filter(name, &telegram.notify_agents) {
            eprintln!(
                "squad-station: notify-telegram: tmux session '{}' does not match notify_agents filter {:?}, skipping",
                name, telegram.notify_agents
            );
            return Ok(());
        }
    }

    // 5. Read hook input from stdin (Claude Code passes JSON with transcript_path)
    let hook_input = read_hook_input();

    // 6. Determine project name (from hook input cwd, or fallback to project_root)
    let project_name = project.unwrap_or_else(|| {
        // Try cwd from hook input first
        if let Some(ref input) = hook_input {
            if let Some(cwd) = input.get("cwd").and_then(|v| v.as_str()) {
                if let Some(name) = std::path::Path::new(cwd).file_name() {
                    return name.to_string_lossy().to_string();
                }
            }
        }
        project_root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| config.project.clone())
    });

    // 7. Extract transcript message (last assistant response from JSONL)
    let transcript_message = hook_input.as_ref().and_then(|input| {
        let path = input.get("transcript_path")?.as_str()?;
        if path.is_empty() {
            return None;
        }
        eprintln!("squad-station: notify-telegram: reading transcript from {path}");
        read_last_assistant_message(path)
    });

    // Fallback: try .message or .last_message from hook input
    let detail_message = transcript_message.or_else(|| {
        hook_input.as_ref().and_then(|input| {
            input
                .get("message")
                .or_else(|| input.get("last_message"))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty() && *s != "null")
                .map(|s| s.to_string())
        })
    });

    // 8. Format and send
    let text = format_message(&project_name, agent_name.as_deref(), detail_message.as_deref());
    // Silently ignore send errors — hooks must always exit 0
    let _ = send_telegram(token, chat_id, topic_id, &text).await;

    Ok(())
}

// ── Public helpers for testing ─────────────────────────────────────────────

pub fn load_env_file_pub(path: &std::path::Path) -> Vec<(String, String)> {
    load_env_file(path)
}

pub fn agent_matches_filter_pub(agent_name: &str, notify_agents: &config::NotifyAgents) -> bool {
    agent_matches_filter(agent_name, notify_agents)
}

pub fn format_message_pub(project_name: &str, agent_name: Option<&str>, transcript: Option<&str>) -> String {
    format_message(project_name, agent_name, transcript)
}

pub fn escape_html_pub(s: &str) -> String {
    escape_html(s)
}

pub fn read_last_assistant_message_pub(path: &str) -> Option<String> {
    read_last_assistant_message(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_quotes_double() {
        assert_eq!(strip_quotes("\"hello\""), "hello");
    }

    #[test]
    fn test_strip_quotes_single() {
        assert_eq!(strip_quotes("'hello'"), "hello");
    }

    #[test]
    fn test_strip_quotes_none() {
        assert_eq!(strip_quotes("hello"), "hello");
    }

    #[test]
    fn test_strip_quotes_mismatched() {
        assert_eq!(strip_quotes("\"hello'"), "\"hello'");
    }

    #[test]
    fn test_strip_quotes_empty() {
        assert_eq!(strip_quotes(""), "");
    }

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
    fn test_load_env_file_strips_quotes() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            tmp.path(),
            "TELE_TOKEN=\"bot123:ABC\"\nTELE_CHAT_ID='-100'\n",
        )
        .unwrap();
        let vars = load_env_file(tmp.path());
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0], ("TELE_TOKEN".into(), "bot123:ABC".into()));
        assert_eq!(vars[1], ("TELE_CHAT_ID".into(), "-100".into()));
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
    fn test_format_message_with_agent() {
        let msg = format_message("myproject", Some("myproject-orchestrator"), None);
        assert!(msg.contains("[myproject]"));
        assert!(msg.contains("myproject-orchestrator"));
        assert!(msg.contains("finished"));
    }

    #[test]
    fn test_format_message_without_agent() {
        let msg = format_message("myproject", None, None);
        assert!(msg.contains("[myproject]"));
        assert!(msg.contains("Agent finished"));
    }

    #[test]
    fn test_format_message_with_transcript() {
        let transcript = "$ cargo test\nrunning 164 tests\ntest result: ok";
        let msg = format_message("myproject", Some("worker"), Some(transcript));
        assert!(msg.contains("[myproject]"));
        assert!(msg.contains("worker"));
        assert!(msg.contains("<pre>"));
        assert!(msg.contains("cargo test"));
        assert!(msg.contains("164 tests"));
    }

    #[test]
    fn test_format_message_transcript_html_escaped() {
        let transcript = "error: <unknown> & 'bad' type";
        let msg = format_message("proj", Some("agent"), Some(transcript));
        assert!(msg.contains("&lt;unknown&gt;"));
        assert!(msg.contains("&amp;"));
    }

    #[test]
    fn test_format_message_truncation() {
        // project_name alone won't exceed 4096, but this tests the guard
        let long_name = "x".repeat(5000);
        let msg = format_message(&long_name, None, None);
        assert!(msg.len() <= 4096 + 30);
        assert!(msg.contains("(truncated)"));
    }

    #[test]
    fn test_escape_html() {
        assert_eq!(escape_html("<b>test</b>"), "&lt;b&gt;test&lt;/b&gt;");
        assert_eq!(escape_html("a & b"), "a &amp; b");
        assert_eq!(escape_html("normal text"), "normal text");
    }

    #[test]
    fn test_read_last_assistant_message() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        // Simulate Claude Code JSONL format
        let jsonl = r#"{"type":"human","message":{"content":[{"type":"text","text":"hello"}]}}
{"type":"assistant","message":{"content":[{"type":"text","text":"First response"}]}}
{"type":"human","message":{"content":[{"type":"text","text":"thanks"}]}}
{"type":"assistant","message":{"content":[{"type":"text","text":"Final detailed response here"}]}}
"#;
        std::fs::write(tmp.path(), jsonl).unwrap();
        let result = read_last_assistant_message(tmp.path().to_str().unwrap());
        assert_eq!(result, Some("Final detailed response here".to_string()));
    }

    #[test]
    fn test_read_last_assistant_message_no_assistant() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let jsonl = r#"{"type":"human","message":{"content":[{"type":"text","text":"hello"}]}}
"#;
        std::fs::write(tmp.path(), jsonl).unwrap();
        let result = read_last_assistant_message(tmp.path().to_str().unwrap());
        assert_eq!(result, None);
    }

    #[test]
    fn test_read_last_assistant_message_missing_file() {
        let result = read_last_assistant_message("/nonexistent/transcript.jsonl");
        assert_eq!(result, None);
    }
}
