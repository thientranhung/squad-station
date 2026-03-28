/// Centralized provider-specific constants and behavior flags.
/// Flat functions — no trait, no dynamic dispatch.
/// Future consideration: refactor to a Provider trait if complexity warrants it.
///
/// Patterns that indicate the provider's TUI is idle and waiting for input.
pub fn idle_patterns(provider: &str) -> Option<&'static [&'static str]> {
    match provider {
        "claude-code" => Some(&["❯"]),
        "codex" => None, // TODO: confirm Codex CLI idle prompt pattern
        "gemini-cli" => Some(&["Type your message"]),
        _ => None,
    }
}

/// Whether /clear triggers the completion hook (Stop/AfterAgent).
/// Claude Code: yes (Stop fires) — root cause of FIFO race.
/// Codex: yes (Stop fires on all turn ends, same as Claude Code).
/// Gemini CLI: no (AfterAgent does not fire on /clear).
pub fn clear_triggers_completion_hook(provider: &str) -> bool {
    matches!(provider, "claude-code" | "codex")
}

/// Provider settings file path relative to project root.
pub fn settings_path(provider: &str) -> Option<&'static str> {
    match provider {
        "claude-code" => Some(".claude/settings.json"),
        "codex" => Some(".codex/hooks.json"),
        "gemini-cli" => Some(".gemini/settings.json"),
        _ => None,
    }
}

/// Whether the provider uses an alternate screen buffer (full-screen TUI).
/// Affects tmux capture-pane strategy: need -a flag for alternate buffer.
pub fn uses_alternate_buffer(provider: &str) -> bool {
    matches!(provider, "gemini-cli")
}

/// Hook event name for task completion signal.
pub fn completion_hook_event(provider: &str) -> Option<&'static str> {
    match provider {
        "claude-code" | "codex" => Some("Stop"),
        "gemini-cli" => Some("AfterAgent"),
        _ => None,
    }
}

/// Whether hook stdout must be valid JSON.
/// Gemini CLI golden rule: stdout must be JSON only.
/// Codex: exit 0 with no output = success (no JSON requirement).
pub fn hook_requires_json_stdout(provider: &str) -> bool {
    matches!(provider, "gemini-cli")
}

/// Commands that execute instantly without producing a provider response turn.
/// These never trigger the completion hook, so DB messages must be auto-completed.
pub fn fire_and_forget_prefixes(provider: &str) -> &'static [&'static str] {
    match provider {
        "claude-code" | "codex" => &["/clear"],
        "gemini-cli" => &["/clear"],
        _ => &["/clear"], // safe default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idle_patterns_claude_code() {
        let patterns = idle_patterns("claude-code").unwrap();
        assert!(patterns.contains(&"❯"));
    }

    #[test]
    fn test_idle_patterns_gemini_cli() {
        let patterns = idle_patterns("gemini-cli").unwrap();
        assert!(patterns.contains(&"Type your message"));
    }

    #[test]
    fn test_idle_patterns_codex() {
        // Codex idle pattern not yet confirmed — returns None
        assert!(idle_patterns("codex").is_none());
    }

    #[test]
    fn test_idle_patterns_unknown() {
        assert!(idle_patterns("unknown-tool").is_none());
    }

    #[test]
    fn test_clear_triggers_completion_hook() {
        assert!(clear_triggers_completion_hook("claude-code"));
        assert!(clear_triggers_completion_hook("codex"));
        assert!(!clear_triggers_completion_hook("gemini-cli"));
        assert!(!clear_triggers_completion_hook("unknown"));
    }

    #[test]
    fn test_settings_path() {
        assert_eq!(settings_path("claude-code"), Some(".claude/settings.json"));
        assert_eq!(settings_path("codex"), Some(".codex/hooks.json"));
        assert_eq!(settings_path("gemini-cli"), Some(".gemini/settings.json"));
        assert!(settings_path("unknown").is_none());
    }

    #[test]
    fn test_uses_alternate_buffer() {
        assert!(!uses_alternate_buffer("claude-code"));
        assert!(!uses_alternate_buffer("codex"));
        assert!(uses_alternate_buffer("gemini-cli"));
    }

    #[test]
    fn test_completion_hook_event() {
        assert_eq!(completion_hook_event("claude-code"), Some("Stop"));
        assert_eq!(completion_hook_event("codex"), Some("Stop"));
        assert_eq!(completion_hook_event("gemini-cli"), Some("AfterAgent"));
        assert!(completion_hook_event("unknown").is_none());
    }

    #[test]
    fn test_hook_requires_json_stdout() {
        assert!(!hook_requires_json_stdout("claude-code"));
        assert!(!hook_requires_json_stdout("codex"));
        assert!(hook_requires_json_stdout("gemini-cli"));
    }

    #[test]
    fn test_fire_and_forget_prefixes() {
        let claude_prefixes = fire_and_forget_prefixes("claude-code");
        assert!(claude_prefixes.contains(&"/clear"));

        let codex_prefixes = fire_and_forget_prefixes("codex");
        assert!(codex_prefixes.contains(&"/clear"));

        let gemini_prefixes = fire_and_forget_prefixes("gemini-cli");
        assert!(gemini_prefixes.contains(&"/clear"));
    }
}
