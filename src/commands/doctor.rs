use crate::{config, db, hook_parser};
use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Serialize)]
pub struct DoctorReport {
    checks: Vec<CheckResult>,
    passed: usize,
    failed: usize,
    total: usize, // excludes Info checks
}

#[derive(Serialize)]
pub struct CheckResult {
    name: String,
    status: CheckStatus,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Serialize, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Pass,
    Fail,
    Info,
}

impl CheckResult {
    fn pass(name: &str, message: impl Into<String>) -> Self {
        CheckResult {
            name: name.to_string(),
            status: CheckStatus::Pass,
            message: message.into(),
            detail: None,
        }
    }

    fn fail(name: &str, message: impl Into<String>) -> Self {
        CheckResult {
            name: name.to_string(),
            status: CheckStatus::Fail,
            message: message.into(),
            detail: None,
        }
    }

    fn fail_detail(name: &str, message: impl Into<String>, detail: impl Into<String>) -> Self {
        CheckResult {
            name: name.to_string(),
            status: CheckStatus::Fail,
            message: message.into(),
            detail: Some(detail.into()),
        }
    }

    fn info(name: &str, message: impl Into<String>) -> Self {
        CheckResult {
            name: name.to_string(),
            status: CheckStatus::Info,
            message: message.into(),
            detail: None,
        }
    }
}

type ConfigResult = (config::SquadConfig, PathBuf);

/// Check 1: Config validation — inner function accepts an explicit root for testability.
fn check_config_at(project_root: &Path) -> (CheckResult, Option<ConfigResult>) {
    match config::load_config(&project_root.join("squad.yml")) {
        Err(e) => (CheckResult::fail("Config", format!("{e}")), None),
        Ok(cfg) => {
            let msg = format!("squad.yml valid (project: {})", cfg.project);
            (
                CheckResult::pass("Config", msg),
                Some((cfg, project_root.to_path_buf())),
            )
        }
    }
}

fn check_config() -> (CheckResult, Option<ConfigResult>) {
    match config::find_project_root() {
        Err(e) => (CheckResult::fail("Config", format!("{e}")), None),
        Ok(project_root) => check_config_at(&project_root),
    }
}

/// Check 2: SDD playbook validation
fn check_sdd_playbooks(config_result: Option<&ConfigResult>) -> CheckResult {
    match config_result {
        None => CheckResult::fail("SDD Playbooks", "skipped (config invalid)"),
        Some((cfg, project_root)) => {
            if cfg.sdd_playbook.is_empty() {
                return CheckResult::pass("SDD Playbooks", "none declared");
            }
            match super::init::validate_sdd_playbooks(&cfg.sdd_playbook, project_root) {
                Ok(()) => {
                    let names = cfg.sdd_playbook.join(", ");
                    CheckResult::pass("SDD Playbooks", format!("{names} verified"))
                }
                Err(e) => CheckResult::fail("SDD Playbooks", format!("{e}")),
            }
        }
    }
}

/// Check 3: tmux availability
fn check_tmux() -> CheckResult {
    match Command::new("tmux").arg("-V").output() {
        Ok(out) if out.status.success() => {
            let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
            CheckResult::pass("tmux", format!("{version} available"))
        }
        _ => CheckResult::fail("tmux", "not found in PATH"),
    }
}

/// Check 4: SQLite DB health.
/// Skipped with [FAIL] when config is unavailable (no project root = no DB path).
/// DB path mirrors config::resolve_db_path logic: SQUAD_STATION_DB env > project_root/.squad/station.db.
async fn check_database(config_result: Option<&ConfigResult>) -> CheckResult {
    let (_, project_root) = match config_result {
        None => return CheckResult::fail("Database", "skipped (config invalid)"),
        Some(cr) => cr,
    };

    let db_path = if let Ok(env_path) = std::env::var("SQUAD_STATION_DB") {
        PathBuf::from(env_path)
    } else {
        project_root.join(".squad").join("station.db")
    };

    if !db_path.exists() {
        return CheckResult::fail("Database", format!("file not found: {}", db_path.display()));
    }

    match db::connect(&db_path).await {
        Err(e) => CheckResult::fail("Database", format!("cannot open: {e}")),
        Ok(pool) => {
            // Check WAL mode
            let journal_mode: Result<String, _> = sqlx::query_scalar("PRAGMA journal_mode")
                .fetch_one(&pool)
                .await;
            match journal_mode {
                Err(e) => return CheckResult::fail("Database", format!("PRAGMA failed: {e}")),
                Ok(mode) if mode != "wal" => {
                    return CheckResult::fail("Database", format!("expected WAL mode, got: {mode}"))
                }
                _ => {}
            }

            // Count migrations applied
            let migration_count: Result<i64, _> =
                sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations")
                    .fetch_one(&pool)
                    .await;
            match migration_count {
                Err(e) => CheckResult::fail("Database", format!("cannot query migrations: {e}")),
                Ok(n) => CheckResult::pass("Database", format!("healthy ({n} migrations applied)")),
            }
        }
    }
}

/// Returns true if `.claude/settings.json` contains a Stop hook whose inner
/// command contains "squad-station signal". Parses JSON rather than naive
/// string search to avoid false-positives from comments or disabled entries.
fn signal_hook_installed(settings_path: &Path) -> Result<bool, String> {
    let content = std::fs::read_to_string(settings_path)
        .map_err(|e| format!("cannot read {}: {e}", settings_path.display()))?;

    let settings: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("invalid JSON: {e}"))?;

    let stop_hooks = match settings
        .get("hooks")
        .and_then(|h| h.get("Stop"))
        .and_then(|s| s.as_array())
    {
        Some(arr) => arr,
        None => return Ok(false),
    };

    // Structure: Stop[*].hooks[*].command
    for entry in stop_hooks {
        if let Some(inner_hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
            for hook in inner_hooks {
                if hook
                    .get("command")
                    .and_then(|c| c.as_str())
                    .map(|cmd| cmd.contains("squad-station signal"))
                    .unwrap_or(false)
                {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

/// Check 5: Hook installation (checks .claude/settings.json)
///
/// Two-phase check:
/// - Phase 1: signal hook presence → fail if missing
/// - Phase 2: scan all hook commands for stale binary paths → fail if any found
fn check_hooks(config_result: Option<&ConfigResult>) -> CheckResult {
    let settings_path = match config_result {
        Some((_, root)) => root.join(".claude/settings.json"),
        None => Path::new(".claude/settings.json").to_path_buf(),
    };

    if !settings_path.exists() {
        return CheckResult::fail("Hooks", format!("{} not found", settings_path.display()));
    }

    // Phase 1: signal hook presence
    match signal_hook_installed(&settings_path) {
        Err(e) => {
            return CheckResult::fail_detail(
                "Hooks",
                format!("cannot read {}", settings_path.display()),
                e,
            )
        }
        Ok(false) => {
            return CheckResult::fail(
                "Hooks",
                format!(
                    "squad-station signal hook missing in {}",
                    settings_path.display()
                ),
            )
        }
        Ok(true) => {} // phase 1 OK, continue to phase 2
    }

    // Phase 2: scan for stale binary paths
    let content = match std::fs::read_to_string(&settings_path) {
        Err(e) => {
            return CheckResult::fail_detail(
                "Hooks",
                format!("cannot read {}", settings_path.display()),
                e.to_string(),
            )
        }
        Ok(c) => c,
    };
    let settings: serde_json::Value = match serde_json::from_str(&content) {
        Err(e) => {
            return CheckResult::fail_detail("Hooks", "invalid JSON in settings", e.to_string())
        }
        Ok(v) => v,
    };

    // Collect stale entries: (location, stale_path)
    let mut stale: Vec<(String, String)> = Vec::new();

    if let Some(hooks_obj) = settings.get("hooks").and_then(|h| h.as_object()) {
        for (event, event_val) in hooks_obj {
            if let Some(event_arr) = event_val.as_array() {
                for (idx, entry) in event_arr.iter().enumerate() {
                    if let Some(inner_hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
                        for inner in inner_hooks {
                            if let Some(cmd) = inner.get("command").and_then(|c| c.as_str()) {
                                if let Some(path) = hook_parser::extract_binary_path(cmd) {
                                    if hook_parser::is_stale(&path) {
                                        let location = format!("{event}[{idx}]");
                                        stale.push((location, path));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if stale.is_empty() {
        return CheckResult::pass("Hooks", "Claude Code hooks installed");
    }

    let n = stale.len();
    let (first_loc, first_path) = &stale[0];
    let message = if n == 1 {
        format!("1 hook binary stale (first: {first_loc})")
    } else {
        format!("{n} hook binaries stale (first: {first_loc})")
    };
    let detail = format!(
        "{first_loc} points to non-existent binary {first_path}. Run squad-station init to regenerate hooks, or manually update the path."
    );
    CheckResult::fail_detail("Hooks", message, detail)
}

/// Check 6: Binary version (always Info)
fn check_version() -> CheckResult {
    let version = env!("CARGO_PKG_VERSION");
    CheckResult::info("Version", format!("squad-station v{version}"))
}

pub async fn run(json: bool) -> Result<()> {
    let mut checks: Vec<CheckResult> = Vec::new();

    // Check 1: Config
    let (config_check, config_result) = check_config();
    checks.push(config_check);

    // Check 2: SDD Playbooks
    checks.push(check_sdd_playbooks(config_result.as_ref()));

    // Check 3: tmux
    checks.push(check_tmux());

    // Check 4: Database
    checks.push(check_database(config_result.as_ref()).await);

    // Check 5: Hooks
    checks.push(check_hooks(config_result.as_ref()));

    // Check 6: Version (Info — excluded from totals)
    checks.push(check_version());

    // Compute totals (exclude Info)
    let non_info: Vec<&CheckResult> = checks
        .iter()
        .filter(|c| c.status != CheckStatus::Info)
        .collect();
    let total = non_info.len();
    let passed = non_info
        .iter()
        .filter(|c| c.status == CheckStatus::Pass)
        .count();
    let failed = non_info
        .iter()
        .filter(|c| c.status == CheckStatus::Fail)
        .count();

    let has_failures = failed > 0;

    if json {
        let report = DoctorReport {
            checks,
            passed,
            failed,
            total,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Squad Station Doctor");
        println!("====================");
        for check in &checks {
            let label = match check.status {
                CheckStatus::Pass => "[PASS]",
                CheckStatus::Fail => "[FAIL]",
                CheckStatus::Info => "[INFO]",
            };
            println!("{} {}: {}", label, check.name, check.message);
            if let Some(ref detail) = check.detail {
                println!("       {detail}");
            }
        }
        println!();
        println!("Result: {passed}/{total} checks passed, {failed} failed");
    }

    if has_failures {
        std::process::exit(1);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn minimal_squad_yml() -> &'static str {
        "project: test-proj\norchestrator:\n  provider: claude-code\n  role: orchestrator\nagents: []\n"
    }

    #[test]
    fn check_result_serialization_pass() {
        let r = CheckResult::pass("Config", "squad.yml valid");
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["status"], "pass");
        assert_eq!(json["name"], "Config");
        assert!(json.get("detail").is_none());
    }

    #[test]
    fn check_result_serialization_fail_with_detail() {
        let r = CheckResult::fail_detail("Database", "cannot open", "permission denied");
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["status"], "fail");
        assert_eq!(json["detail"], "permission denied");
    }

    #[test]
    fn check_result_serialization_info() {
        let r = CheckResult::info("Version", "squad-station v1.0.0");
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["status"], "info");
    }

    #[test]
    fn doctor_report_serialization() {
        let report = DoctorReport {
            checks: vec![
                CheckResult::pass("Config", "ok"),
                CheckResult::fail("Database", "missing"),
                CheckResult::info("Version", "v1.0"),
            ],
            passed: 1,
            failed: 1,
            total: 2,
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["passed"], 1);
        assert_eq!(json["failed"], 1);
        assert_eq!(json["total"], 2);
        assert_eq!(json["checks"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn check_config_missing_returns_fail() {
        let tmp = TempDir::new().unwrap();
        let (result, config_result) = check_config_at(tmp.path());
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(config_result.is_none());
    }

    #[test]
    fn check_config_valid_returns_pass() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("squad.yml"), minimal_squad_yml()).unwrap();
        let (result, config_result) = check_config_at(tmp.path());
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(config_result.is_some());
        assert!(result.message.contains("test-proj"));
    }

    #[test]
    fn check_sdd_playbooks_no_config_returns_fail() {
        let result = check_sdd_playbooks(None);
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.message.contains("skipped"));
    }

    #[test]
    fn check_sdd_playbooks_empty_list_returns_pass() {
        let tmp = TempDir::new().unwrap();
        let cfg: config::SquadConfig = serde_saphyr::from_str(minimal_squad_yml()).unwrap();
        let config_result = (cfg, tmp.path().to_path_buf());
        let result = check_sdd_playbooks(Some(&config_result));
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.message.contains("none declared"));
    }

    #[test]
    fn check_sdd_playbooks_missing_bmad_returns_fail() {
        let tmp = TempDir::new().unwrap();
        let yaml = "project: test\nsdd-playbook:\n  - bmad\norchestrator:\n  provider: claude-code\n  role: orchestrator\nagents: []\n";
        let cfg: config::SquadConfig = serde_saphyr::from_str(yaml).unwrap();
        let config_result = (cfg, tmp.path().to_path_buf());
        let result = check_sdd_playbooks(Some(&config_result));
        assert_eq!(result.status, CheckStatus::Fail);
    }

    #[test]
    fn check_tmux_runs_without_panic() {
        let result = check_tmux();
        assert!(matches!(
            result.status,
            CheckStatus::Pass | CheckStatus::Fail
        ));
    }

    #[test]
    fn check_hooks_missing_settings_returns_fail() {
        let tmp = TempDir::new().unwrap();
        let cfg: config::SquadConfig = serde_saphyr::from_str(minimal_squad_yml()).unwrap();
        let config_result = (cfg, tmp.path().to_path_buf());
        let result = check_hooks(Some(&config_result));
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.message.contains("not found"));
    }

    #[test]
    fn check_hooks_missing_signal_returns_fail() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(claude_dir.join("settings.json"), r#"{"hooks":{}}"#).unwrap();

        let cfg: config::SquadConfig = serde_saphyr::from_str(minimal_squad_yml()).unwrap();
        let config_result = (cfg, tmp.path().to_path_buf());
        let result = check_hooks(Some(&config_result));
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.message.contains("squad-station signal"));
    }

    #[test]
    fn check_hooks_with_signal_hook_returns_pass() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        // Use the real nested structure written by install_claude_hooks
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{"hooks":{"Stop":[{"matcher":"","hooks":[{"type":"command","command":"squad-station signal \"$(tmux display-message -p '#S')\" 2>/dev/null"}]}]}}"#,
        )
        .unwrap();

        let cfg: config::SquadConfig = serde_saphyr::from_str(minimal_squad_yml()).unwrap();
        let config_result = (cfg, tmp.path().to_path_buf());
        let result = check_hooks(Some(&config_result));
        assert_eq!(result.status, CheckStatus::Pass);
    }

    #[test]
    fn check_hooks_signal_in_description_does_not_false_positive() {
        let tmp = TempDir::new().unwrap();
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        // "squad-station signal" appears only in a description field, not in hooks.Stop[*].hooks[*].command
        std::fs::write(
            claude_dir.join("settings.json"),
            r#"{"description":"uses squad-station signal","hooks":{"Stop":[]}}"#,
        )
        .unwrap();

        let cfg: config::SquadConfig = serde_saphyr::from_str(minimal_squad_yml()).unwrap();
        let config_result = (cfg, tmp.path().to_path_buf());
        let result = check_hooks(Some(&config_result));
        assert_eq!(result.status, CheckStatus::Fail);
    }

    #[test]
    fn check_version_is_info() {
        let result = check_version();
        assert_eq!(result.status, CheckStatus::Info);
        assert!(result.message.starts_with("squad-station v"));
    }

    #[tokio::test]
    async fn check_database_no_config_returns_fail_skipped() {
        let result = check_database(None).await;
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.message.contains("skipped"));
    }

    #[tokio::test]
    async fn check_database_missing_file_returns_fail() {
        let tmp = TempDir::new().unwrap();
        let cfg: config::SquadConfig = serde_saphyr::from_str(minimal_squad_yml()).unwrap();
        let config_result = (cfg, tmp.path().to_path_buf());
        let result = check_database(Some(&config_result)).await;
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(result.message.contains("not found"));
    }

    #[tokio::test]
    async fn check_database_healthy_db_returns_pass() {
        let tmp = TempDir::new().unwrap();
        let squad_dir = tmp.path().join(".squad");
        std::fs::create_dir_all(&squad_dir).unwrap();
        let db_path = squad_dir.join("station.db");

        // Create DB with migrations, then drop pool so doctor can reopen it
        let _pool = crate::db::connect(&db_path).await.unwrap();

        let cfg: config::SquadConfig = serde_saphyr::from_str(minimal_squad_yml()).unwrap();
        let config_result = (cfg, tmp.path().to_path_buf());
        let result = check_database(Some(&config_result)).await;
        assert_eq!(result.status, CheckStatus::Pass);
        assert!(result.message.contains("healthy"));
    }

    // ── Phase-2: stale hook binary tests ─────────────────────────────────────

    /// F1 — stale absolute path (real bug reproduction)
    fn f1_stale_settings_json() -> &'static str {
        r#"{"hooks":{"Stop":[{"matcher":"","hooks":[{"type":"command","command":"/Users/tranthien/.cargo/bin/squad-station signal \"$(tmux display-message -p '#S')\" 2>/dev/null"}]}]}}"#
    }

    /// F3 — bare PATH-relative
    fn f3_bare_settings_json() -> &'static str {
        r#"{"hooks":{"Stop":[{"matcher":"","hooks":[{"type":"command","command":"squad-station signal \"$AGENT\""}]}]}}"#
    }

    fn write_settings(tmp_path: &Path, content: &str) -> std::path::PathBuf {
        let claude_dir = tmp_path.join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        let path = claude_dir.join("settings.json");
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn check_hooks_stale_binary_returns_fail() {
        let tmp = TempDir::new().unwrap();
        write_settings(tmp.path(), f1_stale_settings_json());

        let cfg: config::SquadConfig = serde_saphyr::from_str(minimal_squad_yml()).unwrap();
        let config_result = (cfg, tmp.path().to_path_buf());
        let result = check_hooks(Some(&config_result));
        assert_eq!(result.status, CheckStatus::Fail);
        // Should mention stale
        let detail = result.detail.unwrap_or_default();
        assert!(
            detail.contains("Stop[0]"),
            "detail should mention Stop[0], got: {detail}"
        );
        assert!(
            detail.contains("/Users/tranthien/.cargo/bin/squad-station"),
            "detail should contain the stale path, got: {detail}"
        );
    }

    #[test]
    fn check_hooks_bare_name_passes() {
        let tmp = TempDir::new().unwrap();
        write_settings(tmp.path(), f3_bare_settings_json());

        let cfg: config::SquadConfig = serde_saphyr::from_str(minimal_squad_yml()).unwrap();
        let config_result = (cfg, tmp.path().to_path_buf());
        let result = check_hooks(Some(&config_result));
        // bare squad-station is not stale → phase 2 passes; phase 1 passes too
        assert_eq!(result.status, CheckStatus::Pass);
    }

    #[test]
    fn check_hooks_multiple_stale_reports_count() {
        let tmp = TempDir::new().unwrap();
        let two_stale = r#"{"hooks":{"Stop":[
            {"matcher":"","hooks":[{"type":"command","command":"/nope/squad-station signal a 2>/dev/null"}]},
            {"matcher":"","hooks":[{"type":"command","command":"/also/nope/squad-station signal b 2>/dev/null"}]}
        ]}}"#;
        write_settings(&tmp.path(), two_stale);

        let cfg: config::SquadConfig = serde_saphyr::from_str(minimal_squad_yml()).unwrap();
        let config_result = (cfg, tmp.path().to_path_buf());
        let result = check_hooks(Some(&config_result));
        assert_eq!(result.status, CheckStatus::Fail);
        assert!(
            result.message.contains('2'),
            "message should contain count '2', got: {}",
            result.message
        );
    }

    #[cfg(unix)]
    #[test]
    fn check_hooks_fresh_install_passes() {
        use std::os::unix::fs::PermissionsExt;
        // Create a healthy executable file named `squad-station` (path ends with /squad-station)
        let tmp = TempDir::new().unwrap();
        let bin_dir = tmp.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        let bin_file = bin_dir.join("squad-station");
        std::fs::write(&bin_file, b"#!/bin/sh\n").unwrap();
        let mut perms = std::fs::metadata(&bin_file).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&bin_file, perms).unwrap();

        let healthy_path = bin_file.to_str().unwrap();
        // Command must contain "squad-station signal" for phase 1 to pass
        let settings_content = format!(
            r#"{{"hooks":{{"Stop":[{{"matcher":"","hooks":[{{"type":"command","command":"{healthy_path} signal arg 2>/dev/null"}}]}}]}}}}"#
        );
        let claude_dir = tmp.path().join(".claude");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(claude_dir.join("settings.json"), &settings_content).unwrap();

        let cfg: config::SquadConfig = serde_saphyr::from_str(minimal_squad_yml()).unwrap();
        let config_result = (cfg, tmp.path().to_path_buf());
        let result = check_hooks(Some(&config_result));
        // Healthy binary → phase 1 passes (command contains "squad-station signal") and phase 2 passes
        assert_eq!(result.status, CheckStatus::Pass);
    }

    #[test]
    fn check_hooks_stale_in_gemini_afteragent_reports_correct_location() {
        let tmp = TempDir::new().unwrap();
        // Use .claude/settings.json but with AfterAgent event (simulates the scan path)
        let f4_gemini = r#"{"hooks":{"Stop":[{"matcher":"","hooks":[{"type":"command","command":"squad-station signal arg"}]}],"AfterAgent":[{"matcher":"","hooks":[{"type":"command","command":"/stale/squad-station signal \"$AGENT\" >/dev/null 2>&1; printf '{}'"}]}]}}"#;
        write_settings(&tmp.path(), f4_gemini);

        let cfg: config::SquadConfig = serde_saphyr::from_str(minimal_squad_yml()).unwrap();
        let config_result = (cfg, tmp.path().to_path_buf());
        let result = check_hooks(Some(&config_result));
        assert_eq!(result.status, CheckStatus::Fail);
        let detail = result.detail.unwrap_or_default();
        assert!(
            detail.contains("AfterAgent[0]"),
            "detail should mention AfterAgent[0], got: {detail}"
        );
    }
}
