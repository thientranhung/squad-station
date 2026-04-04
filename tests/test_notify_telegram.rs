mod helpers;

// ============================================================
// notify-telegram: exit-code integration tests
//
// These run the ACTUAL binary in an isolated temp directory and
// assert the process exits 0 under every error condition.
// This mirrors the real Claude Code hook invocation:
//   squad-station notify-telegram --project-root "/path/to/project" 2>/dev/null; true
// ============================================================

fn bin() -> String {
    env!("CARGO_BIN_EXE_squad-station").to_string()
}

/// Helper: run notify-telegram in a temp dir with optional setup files.
struct HookTestDir {
    dir: tempfile::TempDir,
}

impl HookTestDir {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("create temp dir"),
        }
    }

    /// Write a squad.yml with telegram enabled.
    fn with_squad_yml_telegram_enabled(self) -> Self {
        let yml = r#"
project: test-project
orchestrator:
  name: orch
  provider: claude-code
  model: opus
agents:
  - name: worker
    provider: claude-code
    model: sonnet
    role: implement
telegram:
  enabled: true
  notify_agents: all
"#;
        std::fs::write(self.dir.path().join("squad.yml"), yml).unwrap();
        self
    }

    /// Write a squad.yml WITHOUT telegram config.
    fn with_squad_yml_no_telegram(self) -> Self {
        let yml = r#"
project: test-project
orchestrator:
  name: orch
  provider: claude-code
  model: opus
agents:
  - name: worker
    provider: claude-code
    model: sonnet
    role: implement
"#;
        std::fs::write(self.dir.path().join("squad.yml"), yml).unwrap();
        self
    }

    /// Write a squad.yml with telegram explicitly disabled.
    fn with_squad_yml_telegram_disabled(self) -> Self {
        let yml = r#"
project: test-project
orchestrator:
  name: orch
  provider: claude-code
  model: opus
agents:
  - name: worker
    provider: claude-code
    model: sonnet
    role: implement
telegram:
  enabled: false
  notify_agents: all
"#;
        std::fs::write(self.dir.path().join("squad.yml"), yml).unwrap();
        self
    }

    /// Write a .env.squad with fake credentials.
    fn with_env_squad_fake_creds(self) -> Self {
        let env = "TELE_TOKEN=fake-invalid-token-000\nTELE_CHAT_ID=-999999\n";
        std::fs::write(self.dir.path().join(".env.squad"), env).unwrap();
        self
    }

    /// Write a .env.squad with empty credentials.
    fn with_env_squad_empty_creds(self) -> Self {
        let env = "TELE_TOKEN=\nTELE_CHAT_ID=\n";
        std::fs::write(self.dir.path().join(".env.squad"), env).unwrap();
        self
    }

    /// Run notify-telegram with given args, return Output.
    fn run(&self, args: &[&str]) -> std::process::Output {
        std::process::Command::new(bin())
            .arg("notify-telegram")
            .args(args)
            .current_dir(self.dir.path())
            .output()
            .expect("failed to execute binary")
    }

    /// Run notify-telegram with --project-root (from a different cwd), return Output.
    fn run_with_project_root(&self, args: &[&str]) -> std::process::Output {
        let different_cwd = tempfile::tempdir().expect("create alternate cwd");
        std::process::Command::new(bin())
            .arg("notify-telegram")
            .arg("--project-root")
            .arg(self.dir.path())
            .args(args)
            .current_dir(different_cwd.path())
            .output()
            .expect("failed to execute binary")
    }
}

// ────────────────────────────────────────────────────────────
// Test: no squad.yml at all → exit 0
// ────────────────────────────────────────────────────────────
#[test]
fn notify_telegram_exits_0_when_no_squad_yml() {
    let ctx = HookTestDir::new();
    let out = ctx.run(&[]);
    assert!(
        out.status.success(),
        "must exit 0 when squad.yml is missing, got: {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

// ────────────────────────────────────────────────────────────
// Test: squad.yml exists but has no telegram section → exit 0
// ────────────────────────────────────────────────────────────
#[test]
fn notify_telegram_exits_0_when_no_telegram_config() {
    let ctx = HookTestDir::new().with_squad_yml_no_telegram();
    let out = ctx.run(&[]);
    assert!(
        out.status.success(),
        "must exit 0 when telegram config is absent, got: {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

// ────────────────────────────────────────────────────────────
// Test: telegram explicitly disabled → exit 0
// ────────────────────────────────────────────────────────────
#[test]
fn notify_telegram_exits_0_when_telegram_disabled() {
    let ctx = HookTestDir::new()
        .with_squad_yml_telegram_disabled()
        .with_env_squad_fake_creds();
    let out = ctx.run(&[]);
    assert!(
        out.status.success(),
        "must exit 0 when telegram is disabled, got: {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

// ────────────────────────────────────────────────────────────
// Test: telegram enabled but .env.squad file does not exist → exit 0
// ────────────────────────────────────────────────────────────
#[test]
fn notify_telegram_exits_0_when_env_file_missing() {
    let ctx = HookTestDir::new().with_squad_yml_telegram_enabled();
    let out = ctx.run(&[]);
    assert!(
        out.status.success(),
        "must exit 0 when .env.squad is missing, got: {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

// ────────────────────────────────────────────────────────────
// Test: telegram enabled, .env.squad exists but credentials are empty → exit 0
// ────────────────────────────────────────────────────────────
#[test]
fn notify_telegram_exits_0_when_credentials_empty() {
    let ctx = HookTestDir::new()
        .with_squad_yml_telegram_enabled()
        .with_env_squad_empty_creds();
    let out = ctx.run(&[]);
    assert!(
        out.status.success(),
        "must exit 0 when credentials are empty, got: {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

// ────────────────────────────────────────────────────────────
// Test: telegram enabled, fake creds (API will reject/timeout) → exit 0
// This is the key regression test: the Telegram API returns an
// error (401 Unauthorized) but the hook must still exit 0.
// ────────────────────────────────────────────────────────────
#[test]
fn notify_telegram_exits_0_when_api_returns_error() {
    let ctx = HookTestDir::new()
        .with_squad_yml_telegram_enabled()
        .with_env_squad_fake_creds();
    let out = ctx.run(&[]);
    assert!(
        out.status.success(),
        "must exit 0 when Telegram API returns error, got: {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

// ────────────────────────────────────────────────────────────
// Test: default args (no flags at all) → exit 0
// ────────────────────────────────────────────────────────────
#[test]
fn notify_telegram_exits_0_with_default_args() {
    let ctx = HookTestDir::new();
    let out = ctx.run(&[]);
    assert!(
        out.status.success(),
        "must exit 0 with default args, got: {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

// ────────────────────────────────────────────────────────────
// Test: --project-root locates squad.yml from a different cwd → exit 0
// ────────────────────────────────────────────────────────────
#[test]
fn notify_telegram_exits_0_with_project_root_flag() {
    let ctx = HookTestDir::new()
        .with_squad_yml_telegram_enabled()
        .with_env_squad_fake_creds();
    let out = ctx.run_with_project_root(&[]);
    assert!(
        out.status.success(),
        "must exit 0 with --project-root from different cwd, got: {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    // Verify it actually found squad.yml (not the "cannot load" error)
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("cannot load squad.yml"),
        "--project-root must locate squad.yml, stderr: {stderr}"
    );
}

// ────────────────────────────────────────────────────────────
// Test: --project-root with no squad.yml → exit 0 (graceful)
// ────────────────────────────────────────────────────────────
#[test]
fn notify_telegram_exits_0_with_project_root_no_config() {
    let ctx = HookTestDir::new(); // no squad.yml
    let out = ctx.run_with_project_root(&[]);
    assert!(
        out.status.success(),
        "must exit 0 with --project-root but no config, got: {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}
