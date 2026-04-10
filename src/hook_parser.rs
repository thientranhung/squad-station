//! Shared parser and predicates for squad-station hook binary path analysis.
//!
//! # Non-goals (not supported, returns `None`):
//! - `env VAR=x <cmd>` prefix wrappers
//! - `sh -c` wrappers
//! - Variable expansion (`$HOME/...`)
//!
//! These require a full shell parser and are out of scope.

/// Extract the binary path from a hook command string.
///
/// Rules:
/// 1. Trim leading whitespace
/// 2. If first char is `'` or `"` → slice to matching close quote (strip quotes from returned path)
/// 3. Else slice to first ASCII whitespace
/// 4. Return `Some(path)` ONLY if path ends with `/squad-station` OR equals `squad-station`
/// 5. Return `None` for unsupported forms (env prefix, sh -c, $VAR expansion)
pub fn extract_binary_path(cmd: &str) -> Option<String> {
    let trimmed = cmd.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    let path = if let Some(rest) = trimmed.strip_prefix('\'') {
        // Single-quoted path
        let close = rest.find('\'')?;
        rest[..close].to_string()
    } else if let Some(rest) = trimmed.strip_prefix('"') {
        // Double-quoted path
        let close = rest.find('"')?;
        rest[..close].to_string()
    } else {
        // Unquoted: slice to first whitespace
        let end = trimmed
            .find(|c: char| c.is_ascii_whitespace())
            .unwrap_or(trimmed.len());
        trimmed[..end].to_string()
    };

    // Only return Some if path ends with `/squad-station` or equals `squad-station`
    if path == "squad-station" || path.ends_with("/squad-station") {
        Some(path)
    } else {
        None
    }
}

/// Returns true if the given path points to a stale (non-executable or missing) binary.
///
/// - Bare `squad-station` (PATH-relative) → never stale
/// - Absolute path that doesn't exist → stale
/// - Absolute path that is a directory → stale
/// - On unix: absolute path that exists but has no execute bit → stale
/// - Absolute path that exists and is executable → not stale
pub fn is_stale(path: &str) -> bool {
    // Bare name is PATH-relative, not stale
    if path == "squad-station" {
        return false;
    }

    // Only handle absolute paths
    if !path.starts_with('/') {
        return false;
    }

    match std::fs::metadata(path) {
        // metadata() follows symlinks: broken symlinks return Err → stale
        Err(_) => true,
        Ok(meta) => {
            if meta.is_dir() {
                return true;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if meta.permissions().mode() & 0o111 == 0 {
                    return true;
                }
            }
            false
        }
    }
}

/// Walk all hook command strings in settings JSON and replace stale squad-station binary paths
/// with `current_bin`. Returns the count of rewrites performed.
///
/// Structure walked: `settings["hooks"][event][*]["hooks"][*]["command"]`
///
/// Preservation guarantees:
/// - Non-squad entries (where `extract_binary_path` returns `None`) are never touched
/// - Args after binary path are byte-exact preserved
/// - Other settings.json top-level keys are untouched (mutate in place)
/// - Bare `squad-station` entries (PATH-relative, not stale) are skipped
pub fn heal_stale_squad_paths(settings: &mut serde_json::Value, current_bin: &str) -> usize {
    let hooks_obj = match settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        Some(obj) => obj,
        None => return 0,
    };

    let mut count = 0;

    for (_event, event_val) in hooks_obj.iter_mut() {
        let event_arr = match event_val.as_array_mut() {
            Some(arr) => arr,
            None => continue,
        };

        for entry in event_arr.iter_mut() {
            let inner_hooks = match entry.get_mut("hooks").and_then(|h| h.as_array_mut()) {
                Some(arr) => arr,
                None => continue,
            };

            for inner in inner_hooks.iter_mut() {
                let cmd = match inner.get("command").and_then(|c| c.as_str()) {
                    Some(s) => s.to_string(),
                    None => continue,
                };

                let trimmed = cmd.trim_start();
                let leading_ws = cmd.len() - trimmed.len();

                // Determine if quoted and find token boundary
                let (old_path, token_end_in_cmd, quote_char) =
                    if let Some(rest) = trimmed.strip_prefix('\'') {
                        match rest.find('\'') {
                            None => continue,
                            Some(close) => {
                                let path = rest[..close].to_string();
                                let token_end = leading_ws + close + 2; // +2 for quotes
                                (path, token_end, Some('\''))
                            }
                        }
                    } else if let Some(rest) = trimmed.strip_prefix('"') {
                        match rest.find('"') {
                            None => continue,
                            Some(close) => {
                                let path = rest[..close].to_string();
                                let token_end = leading_ws + close + 2;
                                (path, token_end, Some('"'))
                            }
                        }
                    } else {
                        let end = trimmed
                            .find(|c: char| c.is_ascii_whitespace())
                            .unwrap_or(trimmed.len());
                        let path = trimmed[..end].to_string();
                        let token_end = leading_ws + end;
                        (path, token_end, None)
                    };

                // Only process squad-station paths
                if old_path != "squad-station" && !old_path.ends_with("/squad-station") {
                    continue;
                }

                // Skip if not stale
                if !is_stale(&old_path) {
                    continue;
                }

                // Skip if already the current binary (no-op)
                if old_path == current_bin {
                    continue;
                }

                // Replace the leading token, preserving args after it
                let suffix = &cmd[token_end_in_cmd..];
                let leading = &cmd[..leading_ws];
                let new_cmd = if let Some(q) = quote_char {
                    // Re-wrap in same quote style
                    format!("{leading}{q}{current_bin}{q}{suffix}")
                } else {
                    format!("{leading}{current_bin}{suffix}")
                };

                inner["command"] = serde_json::Value::String(new_cmd);
                count += 1;
            }
        }
    }

    count
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── extract_binary_path tests ─────────────────────────────────────────────

    #[test]
    fn extract_binary_path_bare_name() {
        assert_eq!(
            extract_binary_path("squad-station signal arg"),
            Some("squad-station".to_string())
        );
    }

    #[test]
    fn extract_binary_path_absolute() {
        assert_eq!(
            extract_binary_path("/a/b/squad-station signal"),
            Some("/a/b/squad-station".to_string())
        );
    }

    #[test]
    fn extract_binary_path_third_party() {
        assert_eq!(extract_binary_path("/usr/bin/other-tool"), None);
    }

    #[test]
    fn extract_binary_path_quoted() {
        assert_eq!(
            extract_binary_path("'/p w/squad-station' signal"),
            Some("/p w/squad-station".to_string())
        );
    }

    #[test]
    fn extract_binary_path_empty() {
        assert_eq!(extract_binary_path(""), None);
    }

    #[test]
    fn extract_binary_path_leading_whitespace() {
        assert_eq!(
            extract_binary_path("  /x/squad-station"),
            Some("/x/squad-station".to_string())
        );
    }

    // ── is_stale tests (unix only) ────────────────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn is_stale_bare_name_is_not_stale() {
        assert!(!is_stale("squad-station"));
    }

    #[cfg(unix)]
    #[test]
    fn is_stale_missing_absolute_path() {
        assert!(is_stale(
            "/this/path/definitely/does/not/exist/squad-station"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn is_stale_existing_non_executable() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path();
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(path, perms).unwrap();
        assert!(is_stale(path.to_str().unwrap()));
    }

    #[cfg(unix)]
    #[test]
    fn is_stale_existing_executable() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path();
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
        assert!(!is_stale(path.to_str().unwrap()));
    }

    #[cfg(unix)]
    #[test]
    fn is_stale_broken_symlink() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let link_path = tmp_dir.path().join("squad-station");
        let nonexistent = tmp_dir.path().join("nonexistent_target");
        std::os::unix::fs::symlink(&nonexistent, &link_path).unwrap();
        // metadata() follows symlinks → broken → Err → stale
        assert!(is_stale(link_path.to_str().unwrap()));
    }

    // ── heal_stale_squad_paths tests ──────────────────────────────────────────

    fn f1_stale_settings() -> serde_json::Value {
        serde_json::json!({
            "hooks": {
                "Stop": [{
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": "/Users/tranthien/.cargo/bin/squad-station signal \"$(tmux display-message -p '#S')\" 2>/dev/null"
                    }]
                }]
            }
        })
    }

    fn f2_mixed_settings() -> serde_json::Value {
        serde_json::json!({
            "hooks": {
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": [{"type": "command", "command": "/nope/squad-station signal \"$AGENT\" 2>/dev/null"}]
                    },
                    {
                        "matcher": "",
                        "hooks": [{"type": "command", "command": "/usr/local/bin/my-custom-hook --flag"}]
                    }
                ]
            }
        })
    }

    fn f3_bare_settings() -> serde_json::Value {
        serde_json::json!({
            "hooks": {
                "Stop": [{
                    "matcher": "",
                    "hooks": [{"type": "command", "command": "squad-station signal \"$AGENT\""}]
                }]
            }
        })
    }

    fn f4_gemini_settings() -> serde_json::Value {
        serde_json::json!({
            "hooks": {
                "AfterAgent": [{
                    "matcher": "",
                    "hooks": [{
                        "type": "command",
                        "command": "/stale/squad-station signal \"$AGENT\" >/dev/null 2>&1; printf '{}'"
                    }]
                }]
            }
        })
    }

    #[test]
    fn heal_rewrites_stale_claude_stop_hook() {
        let mut settings = f1_stale_settings();
        let new_bin = "/Users/tranthien/.squad/bin/squad-station";
        let count = heal_stale_squad_paths(&mut settings, new_bin);
        assert_eq!(count, 1);
        let cmd = settings["hooks"]["Stop"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        // New bin prefix
        assert!(cmd.starts_with(new_bin));
        // Args preserved
        assert!(cmd.contains("signal"));
        assert!(cmd.contains("2>/dev/null"));
        // Old path gone
        assert!(!cmd.contains("/Users/tranthien/.cargo/bin/squad-station"));
    }

    #[test]
    fn heal_preserves_third_party_entries() {
        let mut settings = f2_mixed_settings();
        let new_bin = "/Users/tranthien/.squad/bin/squad-station";
        heal_stale_squad_paths(&mut settings, new_bin);
        // Third-party entry must be byte-exact unchanged
        let third_party_cmd = settings["hooks"]["Stop"][1]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert_eq!(third_party_cmd, "/usr/local/bin/my-custom-hook --flag");
    }

    #[test]
    fn heal_noop_on_bare_name() {
        let mut settings = f3_bare_settings();
        let new_bin = "/Users/tranthien/.squad/bin/squad-station";
        let count = heal_stale_squad_paths(&mut settings, new_bin);
        // bare `squad-station` is not stale → no changes
        assert_eq!(count, 0);
        let cmd = settings["hooks"]["Stop"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert!(cmd.starts_with("squad-station"));
    }

    #[cfg(unix)]
    #[test]
    fn heal_preserves_gemini_printf_suffix() {
        let mut settings = f4_gemini_settings();
        let new_bin = "/Users/tranthien/.squad/bin/squad-station";
        let count = heal_stale_squad_paths(&mut settings, new_bin);
        assert_eq!(count, 1);
        let cmd = settings["hooks"]["AfterAgent"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        // Only leading path swapped
        assert!(cmd.starts_with(new_bin));
        // Redirects and printf preserved
        assert!(cmd.contains(">/dev/null 2>&1; printf '{}'"));
        assert!(cmd.contains("signal"));
    }

    #[cfg(unix)]
    #[test]
    fn heal_noop_on_fresh_install() {
        use std::os::unix::fs::PermissionsExt;
        // Create a healthy (existing + executable) tempfile
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path();
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).unwrap();
        let healthy_bin = path.to_str().unwrap();

        let mut settings = serde_json::json!({
            "hooks": {
                "Stop": [{
                    "matcher": "",
                    "hooks": [{"type": "command", "command": format!("{healthy_bin} signal arg")}]
                }]
            }
        });
        let count = heal_stale_squad_paths(&mut settings, healthy_bin);
        assert_eq!(count, 0);
    }

    #[test]
    fn heal_no_hooks_key() {
        let mut settings = serde_json::json!({});
        let count = heal_stale_squad_paths(&mut settings, "/some/bin/squad-station");
        assert_eq!(count, 0);
    }

    #[test]
    fn heal_returns_rewrite_count() {
        // Two stale entries → count == 2
        let mut settings = serde_json::json!({
            "hooks": {
                "Stop": [
                    {
                        "matcher": "",
                        "hooks": [{"type": "command", "command": "/nope/squad-station signal a 2>/dev/null"}]
                    },
                    {
                        "matcher": "",
                        "hooks": [{"type": "command", "command": "/also/nope/squad-station signal b 2>/dev/null"}]
                    }
                ]
            }
        });
        let new_bin = "/Users/tranthien/.squad/bin/squad-station";
        let count = heal_stale_squad_paths(&mut settings, new_bin);
        assert_eq!(count, 2);
    }
}
