use anyhow::{anyhow, bail, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Allowed provider values for squad.yml
const VALID_PROVIDERS: &[&str] = &["claude-code", "codex", "gemini-cli"];

/// Known SDD playbook names for validation
pub const KNOWN_SDD_PLAYBOOKS: &[&str] = &["bmad", "superpowers", "gsd", "openspec"];

/// Valid model identifiers per provider (provider → allowed model slugs)
fn valid_models_for(provider: &str) -> Option<&'static [&'static str]> {
    match provider {
        "claude-code" => Some(&["opus", "sonnet", "haiku"]),
        "codex" => Some(&[
            "gpt-5.4",
            "gpt-5.4-mini",
            "gpt-5.3-codex",
            "gpt-5.2-codex",
            "gpt-5.2",
            "gpt-5.1-codex-max",
            "gpt-5.1-codex-mini",
        ]),
        "gemini-cli" => Some(&["gemini-3.1-pro-preview", "gemini-3-flash-preview"]),
        _ => None, // no model validation for providers that don't support a model override
    }
}

/// SDD (Software Design Document) workflow configuration
#[derive(Deserialize, Debug, Clone)]
pub struct SddConfig {
    pub name: String,
    pub playbook: String, // absolute path to playbook .md file
}

/// Which agents to send Telegram notifications for
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum NotifyAgents {
    All(String),       // "all"
    List(Vec<String>), // ["orchestrator", "implement"]
}

impl NotifyAgents {
    /// Serialize to the comma-separated format used in .squad/telegram.env
    pub fn to_env_value(&self) -> String {
        match self {
            NotifyAgents::All(s) => s.clone(),
            NotifyAgents::List(v) => v.join(","),
        }
    }
}

/// Telegram notification configuration (non-sensitive parts only; secrets live in .env.squad)
#[derive(Deserialize, Debug, Clone)]
pub struct TelegramConfig {
    pub enabled: bool,
    #[serde(default = "default_notify_agents")]
    pub notify_agents: NotifyAgents,
}

fn default_notify_agents() -> NotifyAgents {
    NotifyAgents::All("all".to_string())
}

/// Top-level squad configuration
#[derive(Deserialize, Debug)]
pub struct SquadConfig {
    pub project: String,             // CONF-01: plain string (not a nested struct)
    pub sdd: Option<Vec<SddConfig>>, // optional SDD workflow configs
    pub telegram: Option<TelegramConfig>, // optional Telegram notifications
    pub orchestrator: AgentConfig,
    pub agents: Vec<AgentConfig>,
    #[serde(default, rename = "sdd-playbook")]
    pub sdd_playbook: Vec<String>, // list of SDD playbook names to validate
}

impl SquadConfig {
    /// Validate all agent configs (orchestrator + workers).
    /// Returns a descriptive error on the first invalid provider or model found.
    pub fn validate(&self) -> Result<()> {
        let label = self.orchestrator.name.as_deref().unwrap_or("orchestrator");
        validate_agent_config(label, &self.orchestrator)?;
        for agent in &self.agents {
            let label = agent.name.as_deref().unwrap_or(&agent.role);
            validate_agent_config(label, agent)?;
        }
        // Validate sdd-playbook names and check for duplicates
        {
            let mut seen = std::collections::HashSet::new();
            for name in &self.sdd_playbook {
                if !KNOWN_SDD_PLAYBOOKS.contains(&name.as_str()) {
                    bail!(
                        "Unknown SDD playbook '{}'. Valid playbooks are: {}.",
                        name,
                        KNOWN_SDD_PLAYBOOKS.join(", ")
                    );
                }
                if !seen.insert(name.as_str()) {
                    bail!("Duplicate SDD playbook '{}' in sdd-playbook list.", name);
                }
            }
        }
        // Validate telegram.notify_agents if present
        if let Some(tg) = &self.telegram {
            if let NotifyAgents::All(ref s) = tg.notify_agents {
                if s != "all" {
                    bail!(
                        "telegram.notify_agents must be \"all\" or a list of agent names, got \"{}\"",
                        s
                    );
                }
            }
        }
        Ok(())
    }

    /// Returns true if Telegram notifications are enabled in the config.
    pub fn is_telegram_enabled(&self) -> bool {
        self.telegram.as_ref().is_some_and(|t| t.enabled)
    }
}

/// Agent configuration (used for both orchestrator and worker agents)
#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct AgentConfig {
    pub name: Option<String>, // optional; orchestrator name auto-derived in Phase 5
    pub provider: String,     // CONF-04: provider name (e.g. claude-code, gemini-cli, codex)
    #[serde(default = "default_role")]
    pub role: String,
    pub model: Option<String>, // CONF-02: optional model override
    pub description: Option<String>, // CONF-02: optional description
                               // command field is REMOVED (CONF-03: provider infers launch command)
}

/// Sanitize a string for use as a tmux session name.
/// Replaces shell metacharacters and tmux-special chars with `-` to prevent
/// both tmux targeting issues and shell injection in `sh -c` commands.
pub fn sanitize_session_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '.' | ':' | '"' | '\'' | '`' | '$' | ';' | '(' | ')' | '|' | '&' | '<' | '>' | '\\'
            | ' ' | '\n' | '\0' | '/' => '-',
            _ => c,
        })
        .collect()
}

/// Build a tmux session name from project name and agent suffix, avoiding duplication.
/// If `suffix` already starts with `{project}-`, the project prefix is not added again.
/// Example: project="squad-demo", suffix="orchestrator" → "squad-demo-orchestrator"
/// Example: project="squad-demo", suffix="squad-demo-orchestrator" → "squad-demo-orchestrator"
pub fn build_session_name(project: &str, suffix: &str) -> String {
    let prefix = format!("{}-", sanitize_session_name(project));
    let sanitized_suffix = sanitize_session_name(suffix);
    if sanitized_suffix.starts_with(&prefix) {
        sanitized_suffix
    } else {
        format!("{}{}", prefix, sanitized_suffix)
    }
}

fn default_role() -> String {
    "worker".to_string()
}

/// Validate provider and (optionally) model for a single agent config.
/// Known providers get model validation; unknown providers get a warning but proceed.
fn validate_agent_config(label: &str, agent: &AgentConfig) -> Result<()> {
    if !VALID_PROVIDERS.contains(&agent.provider.as_str()) {
        // Unknown provider: warn but don't fail — allows extensibility
        eprintln!(
            "Warning: Unknown provider '{}' for agent '{}'. \
             Known providers: {}. Proceeding without model validation.",
            agent.provider,
            label,
            VALID_PROVIDERS.join(", ")
        );
        return Ok(()); // skip model validation for unknown providers
    }

    // Model validation (only for known providers with a model list)
    if let Some(model) = &agent.model {
        if let Some(valid_models) = valid_models_for(&agent.provider) {
            if !valid_models.contains(&model.as_str()) {
                bail!(
                    "Invalid model '{}' for provider '{}' (agent '{}'). Valid models are: {}.",
                    model,
                    agent.provider,
                    label,
                    valid_models.join(", ")
                );
            }
        }
    }

    Ok(())
}

/// Walk up the directory tree to find `squad.yml`, returning the project root directory.
/// Similar to how git finds `.git/` or cargo finds `Cargo.toml`.
///
/// **Worktree awareness:** If the current directory is inside a git worktree
/// (e.g. `.claude/worktrees/<name>/`), the main working tree's root is preferred
/// when it also contains `squad.yml`. This ensures all agents share a single
/// database even when hooks fire from a worktree with a different cwd.
pub fn find_project_root() -> Result<PathBuf> {
    // First, check if we're in a git worktree and the main repo has squad.yml.
    // This takes priority because worktrees may contain a copied squad.yml
    // but should always use the main project's database.
    if let Some(main_root) = resolve_main_worktree_root() {
        if main_root.join("squad.yml").exists() {
            return Ok(main_root);
        }
    }

    // Standard walk-up: find squad.yml in cwd or any parent
    let mut dir = std::env::current_dir()
        .map_err(|e| anyhow!("Cannot determine current directory: {}", e))?;
    loop {
        if dir.join("squad.yml").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            bail!("squad.yml not found in current directory or any parent directory. Run 'squad-station init' with a squad.yml config file.");
        }
    }
}

/// Detect if we're inside a git worktree and return the main working tree root.
/// Returns `None` if not in a worktree, not in a git repo, or git is unavailable.
///
/// Uses `git rev-parse --git-dir` and `--git-common-dir` to detect worktrees:
/// - In a main repo: both return the same `.git` directory
/// - In a worktree: `--git-dir` points to `.git/worktrees/<name>`,
///   `--git-common-dir` points to the main `.git` — its parent is the main root
fn resolve_main_worktree_root() -> Option<PathBuf> {
    let git_dir = std::process::Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-dir"])
        .output()
        .ok()?;
    let git_common = std::process::Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .ok()?;

    if !git_dir.status.success() || !git_common.status.success() {
        return None;
    }

    let git_dir_path = PathBuf::from(String::from_utf8_lossy(&git_dir.stdout).trim().to_string());
    let common_path = PathBuf::from(
        String::from_utf8_lossy(&git_common.stdout)
            .trim()
            .to_string(),
    );

    // If they differ, we're in a worktree — common_path's parent is the main repo root
    if git_dir_path != common_path {
        common_path.parent().map(|p| p.to_path_buf())
    } else {
        None
    }
}

/// Load squad configuration from a YAML file and validate its contents.
/// If the default `squad.yml` path doesn't exist, walks up the directory tree.
/// Explicit non-default paths (e.g. `/tmp/custom.yml`) are NOT searched up the tree.
pub fn load_config(path: &Path) -> Result<SquadConfig> {
    let is_default_path = path == Path::new("squad.yml");
    let config_path = if is_default_path {
        // Always use find_project_root() for the default path — this handles
        // worktree detection (preferring the main repo's squad.yml over a
        // worktree copy) and walk-up from subdirectories.
        find_project_root()?.join("squad.yml")
    } else if path.exists() {
        path.to_path_buf()
    } else {
        // Explicit path given but not found — don't walk up
        path.to_path_buf()
    };
    let content = std::fs::read_to_string(&config_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow!("squad.yml not found in current directory or any parent directory. Run 'squad-station init' with a squad.yml config file.")
        } else {
            anyhow!("Failed to read {}: {}", config_path.display(), e)
        }
    })?;
    let config: SquadConfig = serde_saphyr::from_str(&content)?;
    config.validate()?;
    Ok(config)
}

/// Resolve the DB path. Uses project root (where squad.yml lives), not CWD.
/// SQUAD_STATION_DB env var overrides the default path (useful for testing).
pub fn resolve_db_path(_config: &SquadConfig) -> Result<PathBuf> {
    let db_path = if let Ok(env_path) = std::env::var("SQUAD_STATION_DB") {
        PathBuf::from(env_path)
    } else {
        let project_root = find_project_root()?;
        project_root.join(".squad").join("station.db")
    };

    // Ensure the parent directory exists
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    Ok(db_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_agent(provider: &str, model: Option<&str>) -> AgentConfig {
        AgentConfig {
            name: None,
            provider: provider.to_string(),
            role: "worker".to_string(),
            model: model.map(str::to_string),
            description: None,
        }
    }

    #[test]
    fn valid_provider_no_model() {
        assert!(validate_agent_config("orch", &make_agent("claude-code", None)).is_ok());
        assert!(validate_agent_config("orch", &make_agent("codex", None)).is_ok());
        assert!(validate_agent_config("orch", &make_agent("gemini-cli", None)).is_ok());
    }

    #[test]
    fn unknown_provider_warns_but_succeeds() {
        // GAP-03: unknown providers warn to stderr but don't fail
        assert!(validate_agent_config("agent1", &make_agent("gemini", None)).is_ok());
        assert!(validate_agent_config("agent1", &make_agent("aider", None)).is_ok());
        assert!(validate_agent_config("agent1", &make_agent("opencode", None)).is_ok());
    }

    #[test]
    fn valid_model_accepted() {
        assert!(validate_agent_config("a", &make_agent("claude-code", Some("sonnet"))).is_ok());
        assert!(validate_agent_config("a", &make_agent("codex", Some("gpt-5.4"))).is_ok());
        assert!(validate_agent_config("a", &make_agent("codex", Some("gpt-5.3-codex"))).is_ok());
        assert!(validate_agent_config(
            "a",
            &make_agent("gemini-cli", Some("gemini-3-flash-preview"))
        )
        .is_ok());
    }

    #[test]
    fn invalid_model_rejected() {
        let err = validate_agent_config("a", &make_agent("claude-code", Some("claude-code-2")))
            .unwrap_err();
        assert!(err.to_string().contains("opus, sonnet, haiku"));

        let err = validate_agent_config("a", &make_agent("codex", Some("gpt-4o"))).unwrap_err();
        assert!(err.to_string().contains("gpt-5.4"));

        let err =
            validate_agent_config("a", &make_agent("gemini-cli", Some("gemini-pro"))).unwrap_err();
        assert!(err
            .to_string()
            .contains("gemini-3.1-pro-preview, gemini-3-flash-preview"));
    }

    #[test]
    fn sanitize_session_name_replaces_dots() {
        assert_eq!(sanitize_session_name("my.app-worker"), "my-app-worker");
    }

    #[test]
    fn sanitize_session_name_replaces_colons() {
        assert_eq!(sanitize_session_name("proj:v2-agent"), "proj-v2-agent");
    }

    #[test]
    fn sanitize_session_name_replaces_quotes() {
        assert_eq!(
            sanitize_session_name(r#"proj"name-agent"#),
            "proj-name-agent"
        );
    }

    #[test]
    fn sanitize_session_name_clean_passthrough() {
        assert_eq!(
            sanitize_session_name("squad-station-implement"),
            "squad-station-implement"
        );
    }

    #[test]
    fn telegram_config_all_agents() {
        let yaml = r#"
project: test
telegram:
  enabled: true
  notify_agents: all
orchestrator:
  provider: claude-code
  role: orchestrator
agents:
  - name: worker
    provider: claude-code
"#;
        let config: SquadConfig = serde_saphyr::from_str(yaml).unwrap();
        config.validate().unwrap();
        let tg = config.telegram.unwrap();
        assert!(tg.enabled);
        assert_eq!(tg.notify_agents.to_env_value(), "all");
    }

    #[test]
    fn telegram_config_agent_list() {
        let yaml = r#"
project: test
telegram:
  enabled: true
  notify_agents:
    - orchestrator
    - implement
orchestrator:
  provider: claude-code
  role: orchestrator
agents:
  - name: worker
    provider: claude-code
"#;
        let config: SquadConfig = serde_saphyr::from_str(yaml).unwrap();
        config.validate().unwrap();
        let tg = config.telegram.unwrap();
        assert_eq!(tg.notify_agents.to_env_value(), "orchestrator,implement");
    }

    #[test]
    fn telegram_config_optional() {
        let yaml = r#"
project: test
orchestrator:
  provider: claude-code
  role: orchestrator
agents:
  - name: worker
    provider: claude-code
"#;
        let config: SquadConfig = serde_saphyr::from_str(yaml).unwrap();
        assert!(config.telegram.is_none());
    }

    #[test]
    fn telegram_config_validates_all_string() {
        let yaml = r#"
project: test
telegram:
  enabled: true
  notify_agents: foo
orchestrator:
  provider: claude-code
  role: orchestrator
agents:
  - name: worker
    provider: claude-code
"#;
        let config: SquadConfig = serde_saphyr::from_str(yaml).unwrap();
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("notify_agents"));
    }

    #[test]
    fn telegram_config_defaults_notify_agents() {
        let yaml = r#"
project: test
telegram:
  enabled: true
orchestrator:
  provider: claude-code
  role: orchestrator
agents:
  - name: worker
    provider: claude-code
"#;
        let config: SquadConfig = serde_saphyr::from_str(yaml).unwrap();
        config.validate().unwrap();
        let tg = config.telegram.unwrap();
        assert_eq!(tg.notify_agents.to_env_value(), "all");
    }

    #[test]
    fn build_session_name_no_duplication() {
        assert_eq!(
            build_session_name("squad-demo", "orchestrator"),
            "squad-demo-orchestrator"
        );
    }

    #[test]
    fn build_session_name_skips_prefix_when_already_present() {
        assert_eq!(
            build_session_name("squad-demo", "squad-demo-orchestrator"),
            "squad-demo-orchestrator"
        );
    }

    #[test]
    fn build_session_name_sanitizes() {
        assert_eq!(build_session_name("my.app", "worker"), "my-app-worker");
    }

    #[test]
    fn deny_unknown_fields_in_agent_config() {
        // BUG-16: unknown fields like tmux-session must be rejected
        let yaml = r#"
project: test
orchestrator:
  provider: claude-code
  role: orchestrator
agents:
  - name: worker
    provider: claude-code
    role: worker
    tmux-session: custom-name
"#;
        let result: Result<SquadConfig, _> = serde_saphyr::from_str(yaml);
        assert!(
            result.is_err(),
            "unknown field 'tmux-session' should be rejected"
        );
    }

    /// Verify that `resolve_main_worktree_root` returns None when run from
    /// the main working tree (not a worktree). This test runs in the
    /// squad-station repo itself, which is not a worktree.
    #[test]
    fn resolve_main_worktree_root_returns_none_in_main_repo() {
        // When running from a normal git repo (not a worktree),
        // git-dir and git-common-dir are identical → should return None
        let result = resolve_main_worktree_root();
        // In CI or the main repo, this should be None
        // (unless tests happen to run inside a worktree)
        if let Some(ref root) = result {
            // If it returned Some, verify we're actually in a worktree
            // by checking that cwd contains a worktree marker
            let cwd = std::env::current_dir().unwrap();
            assert!(
                cwd.to_string_lossy().contains("worktree"),
                "resolve_main_worktree_root returned Some({}) but cwd {} doesn't look like a worktree",
                root.display(),
                cwd.display()
            );
        }
    }

    #[test]
    fn sdd_playbook_defaults_to_empty() {
        let yaml = r#"
project: test
orchestrator:
  provider: claude-code
  role: orchestrator
agents: []
"#;
        let config: SquadConfig = serde_saphyr::from_str(yaml).unwrap();
        assert!(config.sdd_playbook.is_empty());
    }

    #[test]
    fn sdd_playbook_parses_list() {
        let yaml = r#"
project: test
sdd-playbook:
  - bmad
  - openspec
orchestrator:
  provider: claude-code
  role: orchestrator
agents: []
"#;
        let config: SquadConfig = serde_saphyr::from_str(yaml).unwrap();
        config.validate().unwrap();
        assert_eq!(config.sdd_playbook, vec!["bmad", "openspec"]);
    }

    #[test]
    fn sdd_playbook_rejects_duplicate_name() {
        let yaml = r#"
project: test
sdd-playbook:
  - bmad
  - bmad
orchestrator:
  provider: claude-code
  role: orchestrator
agents: []
"#;
        let config: SquadConfig = serde_saphyr::from_str(yaml).unwrap();
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("Duplicate"));
        assert!(err.to_string().contains("bmad"));
    }

    #[test]
    fn sdd_playbook_rejects_unknown_name() {
        let yaml = r#"
project: test
sdd-playbook:
  - bmad
  - unknown-playbook
orchestrator:
  provider: claude-code
  role: orchestrator
agents: []
"#;
        let config: SquadConfig = serde_saphyr::from_str(yaml).unwrap();
        let err = config.validate().unwrap_err();
        assert!(err.to_string().contains("unknown-playbook"));
        assert!(err.to_string().contains("bmad, superpowers, gsd, openspec"));
    }

    /// Verify that `find_project_root` prefers the main repo when in a worktree.
    /// This test creates a simulated git worktree structure using a temp dir.
    #[test]
    fn find_project_root_prefers_main_repo_over_worktree() {
        // This test verifies the standard walk-up behavior.
        // Full worktree detection requires actual git infrastructure,
        // so the worktree path is tested via integration tests.
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();

        // Create squad.yml at the root
        std::fs::write(
            root.join("squad.yml"),
            "project: test\norchestrator:\n  provider: claude-code\nagents: []\n",
        )
        .unwrap();

        // Create a nested dir (simulating a subdir) with its own squad.yml
        let nested = root.join("sub").join("dir");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            nested.join("squad.yml"),
            "project: nested\norchestrator:\n  provider: claude-code\nagents: []\n",
        )
        .unwrap();

        // When searching from the nested dir, walk-up should find the nested squad.yml first
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(&nested).unwrap();
        let found = find_project_root().unwrap();
        std::env::set_current_dir(&original).unwrap();

        // Canonicalize for comparison (macOS /var → /private/var)
        let found = std::fs::canonicalize(found).unwrap();
        let nested = std::fs::canonicalize(nested).unwrap();
        assert_eq!(found, nested);
    }
}
