mod helpers;

// ============================================================
// notify-telegram: exit-code integration tests
//
// These run the ACTUAL binary in an isolated temp directory and
// assert the process exits 0 under every error condition.
// This mirrors the real Claude Code hook invocation:
//   cd <project> && squad-station notify-telegram --event Stop 2>/dev/null; true
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
}

// ────────────────────────────────────────────────────────────
// Test: no squad.yml at all → exit 0
// ────────────────────────────────────────────────────────────
#[test]
fn notify_telegram_exits_0_when_no_squad_yml() {
    let ctx = HookTestDir::new();
    // No squad.yml, no .env.squad — bare temp directory
    let out = ctx.run(&["--event", "Stop"]);
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
    let out = ctx.run(&["--event", "Stop"]);
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
    let out = ctx.run(&["--event", "Stop"]);
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
    // No .env.squad file — credentials will be empty strings
    let out = ctx.run(&["--event", "Stop"]);
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
    let out = ctx.run(&["--event", "Stop"]);
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
    let out = ctx.run(&["--event", "Stop", "--message", "test message"]);
    assert!(
        out.status.success(),
        "must exit 0 when Telegram API returns error, got: {:?}\nstderr: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
}

// ────────────────────────────────────────────────────────────
// Test: all event types exit 0 with fake creds
// ────────────────────────────────────────────────────────────
#[test]
fn notify_telegram_exits_0_for_all_event_types() {
    let ctx = HookTestDir::new()
        .with_squad_yml_telegram_enabled()
        .with_env_squad_fake_creds();

    for event in &[
        "Stop",
        "SessionStart",
        "SessionEnd",
        "Notification",
        "CustomEvent",
    ] {
        let out = ctx.run(&["--event", event, "--message", "test"]);
        assert!(
            out.status.success(),
            "must exit 0 for event '{}', got: {:?}\nstderr: {}",
            event,
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
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
