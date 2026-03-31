# Changelog

All notable changes to Squad Station are documented in this file.

## v0.8.5 — Fix signal delivery & DB resolution in git worktrees (2026-03-31)

Fixes the root cause of broken signal delivery when agents run inside a git worktree. `find_project_root()` now detects worktrees and resolves to the main working tree, ensuring all commands share the same database regardless of which cwd they run from.

### Fixed

- **Worktree-aware project root resolution** — `find_project_root()` uses `git rev-parse --git-dir` vs `--git-common-dir` to detect worktrees and prefer the main repo's `squad.yml` and `.squad/station.db`. Previously, running from a worktree would create/use a separate database, causing signals to hit the wrong DB and agents to appear unregistered.
- **`load_config()` always uses `find_project_root()`** for the default `squad.yml` path, ensuring worktree detection applies even when a `squad.yml` exists in the worktree's cwd.

### Added

- `resolve_main_worktree_root()` helper that detects git worktrees via `--git-common-dir`
- Unit tests for worktree resolution behavior

---

## v0.8.4 — Fix Telegram hook silent failure in worktrees (2026-03-31)

Fixes silent Telegram notification failure when Claude Code runs hooks from a git worktree. The generated hook command used a relative path (`"."`) for `SQUAD_PROJECT_ROOT`, which resolves to the wrong directory when the hook runner's cwd differs from the project root.

### Fixed

- **Telegram hook uses absolute paths** — `install_telegram_hooks()` now canonicalizes `project_root` so the generated `SQUAD_PROJECT_ROOT` and script path in `settings.json` are always absolute, regardless of how the caller provides the path.

### Added

- Regression test verifying hook commands contain absolute paths even when called with relative `"."` root.

---

## v0.8.3 — Remove bootstrap injection & fix Telegram spam (2026-03-31)

Fixes two issues disrupting user workflows: the bootstrap block that was auto-injecting into CLAUDE.md on every `squad-station init`, and Telegram notification hooks firing on every tool call instead of only on task completion.

### Fixed

- **Removed bootstrap block injection** — `squad-station init` no longer writes a `<!-- squad-station:bootstrap-start -->` block into the user's CLAUDE.md / GEMINI.md / AGENTS.md. The orchestrator context is available via the slash command file, making the bootstrap redundant.
- **Telegram notifications restricted to Stop event only** — `notify-telegram.sh` was registered on Stop, Notification, and PostToolUse events (Claude/Codex) and AfterAgent + Notification (Gemini), causing massive Telegram spam on every tool call. Now only fires on the completion event (Stop or AfterAgent).

### Removed

- `inject_bootstrap_block()`, `build_bootstrap_block()`, `provider_doc_paths()` from context.rs
- `remove_bootstrap_block()` from uninstall.rs (cleanup no longer needed)
- 9 bootstrap-related unit tests

---

## v0.8.2 — Fix install to always update reference templates (2026-03-30)

Fixes a UX bug where `npx squad-station@latest install` skipped example configs, SDD playbooks, and rules when they already existed, requiring `--force`. These are package-provided reference templates that should always be refreshed on install.

### Fixed

- **examples/ always overwritten** — Example config templates are now refreshed on every install without `--force`
- **sdd/ always overwritten** — Playbook documentation is now refreshed on every install (also fixed in `bin/run.js` which still had the old skip logic)
- **rules/ always overwritten** — Git workflow rules are now refreshed on every install
- **hooks/ preserved** — Hook scripts still require `--force` to overwrite, since users may customize them
- **Removed dead code** — Cleaned up unused `--force` flag parsing in `bin/run.js`

---

## v0.8.1 — Post-release fixes: version sync, code style, example configs (2026-03-30)

Fixes version mismatches introduced in v0.8.0, adds Telegram notification templates to all example configs, and cleans up code style (fmt + clippy).

### Fixed

- **Version sync** — `run.js` and root `package.json` now correctly report v0.8.1 (were stuck on v0.7.23 in v0.8.0)
- **Code style** — Applied `cargo fmt` formatting and fixed clippy `unnecessary_map_or` warning

### Added

- **Telegram config in examples** — All example `squad.yml` files now include the `telegram:` section template
- **Release checklist hardened** — Strengthened version-sync verification steps in the release skill

---

## v0.7.23 — Stronger anti-polling instruction in orchestrator context (2026-03-29)

Reinforced the "no polling" message in `squad-orchestrator.md` so the orchestrator stops using `tmux capture-pane` loops to check agent progress.

### Changed

- **Orchestrator context: "NO POLLING" section** — Replaced soft "you DO NOT need to" with explicit `CRITICAL: DO NOT poll agents`. Shorter, more direct, harder to ignore.

---

## v0.7.22 — Simplify session conflict check (2026-03-29)

Simplified the init session conflict detection — now uses a straightforward name match against live tmux sessions instead of querying each session's working directory.

### Changed

- **Conflict check uses name-only matching** — Replaced CWD-based comparison (`session_cwd` + `canonicalize`) with a simple `list_live_session_names().contains()` check. Much less code, same result.
- Removed unused `tmux::session_cwd()` helper.

---

## v0.7.21 — Init detects tmux session name conflicts (2026-03-29)

`squad-station init` now checks if planned tmux session names are already in use by another project, preventing accidental collisions when `squad.yml` is copied without changing the `project:` field.

### Added

- **Session conflict detection** — Before creating any sessions, `init` queries each planned session's CWD via `tmux display-message`. If a session exists but its working directory differs from the current project root, init aborts with a clear error showing which sessions conflict and where they're running from.
- `tmux::session_cwd()` helper — Returns the active pane's current working directory for a given tmux session.
- **SDD playbook updates** — All bundled playbooks (bmad, gsd, openspec, superpowers) streamlined.

---

## v0.7.20 — Update SDD playbooks (2026-03-29)

Refined all bundled SDD (Solution Design Document) playbooks with streamlined content.

### Changed

- **bmad-playbook.md** — Streamlined BMAD method playbook
- **gsd-playbook.md** — Streamlined GSD playbook
- **openspec-playbook.md** — Streamlined OpenSpec playbook
- **superpowers-playbook.md** — Streamlined Superpowers playbook

---

## v0.7.19 — Fix Codex hooks not firing SQUAD SIGNAL (2026-03-29)

`squad-station update` now installs hooks for all providers (not just the orchestrator's), and Codex agents get the required `config.toml` feature flag so their Stop hook actually fires.

### Fixed

- **Update housekeeping only installed hooks for orchestrator provider** — `run_housekeeping` called `auto_install_hooks` with only the orchestrator's provider (e.g. `claude-code`), skipping worker providers like `codex` or `gemini-cli`. Now it collects all unique providers across orchestrator + workers and installs hooks for each.
- **Codex `config.toml` feature flag not created** — Codex requires `[features] codex_hooks = true` in `.codex/config.toml` to activate its hooks subsystem. Without it, Codex ignores `hooks.json` entirely. `install_codex_hooks` now creates/updates `config.toml` with the feature flag automatically.

### Added

- `ensure_codex_feature_flag()` helper — derives `config.toml` path from `hooks.json`, idempotently appends the feature flag, preserves existing config content.
- 3 new tests: feature flag creation, preservation of existing config, idempotency.

---

## v0.7.18 — Update Regenerates Orchestrator Context (2026-03-29)

`squad-station update` now regenerates `squad-orchestrator.md` after every run, so the orchestrator always sees the current agent list including newly added or removed agents.

### Fixed

- **`squad-orchestrator.md` not updated after `update`** — Adding a new agent via `update` launched the session and updated the monitor, but the orchestrator's context file still listed the old agents. Now `context::run(false)` is called after every `update` (both the changes path and no-changes path), ensuring the agent list, routing rules, and `squad-station send` examples in `squad-orchestrator.md` are always in sync with the actual squad.

---

## v0.7.17 — Update Rebuilds Monitor on Agent Changes (2026-03-29)

`squad-station update` now rebuilds the monitor session whenever the agent set changes, so new agents appear as panes immediately after update.

### Fixed

- **Monitor not updated after adding new agent** — When `update` detected new/removed/provider-changed agents, the monitor session was left stale. Now `ensure_monitor(force: true)` kills and recreates the monitor after any agent changes so its panes always reflect the current squad.
- **No-change path unaffected** — When there are no agent changes, `ensure_monitor(force: false)` still only recreates the monitor if it is dead (same behavior as before).

---

## v0.7.16 — Remove Antigravity Provider (2026-03-29)

Antigravity is not a CLI agent and has been completely removed from squad-station. All providers now use tmux sessions — the DB-only mode is gone.

### Removed

- **`antigravity` provider** — Removed from `VALID_PROVIDERS`, `is_db_only()` method, all conditional branches in `init`, `signal`, `notify`, `helpers`, and `update`
- **`is_db_only()` method** on `AgentConfig` — No longer needed; every provider is tmux-based
- **DB-only orchestrator path** in `init` — Orchestrator always launches in a tmux session
- **Antigravity skip logic** in `signal` and `notify` — Orchestrator notifications no longer have an antigravity bypass
- **12 antigravity-specific tests** across `test_config.rs` and `test_integration.rs`

### Changed

- `VALID_PROVIDERS` is now `["claude-code", "codex", "gemini-cli"]`
- Monitor session in `update` always includes orchestrator (no more `is_db_only` guard)
- `scripts/_common.sh` VALID_PROVIDERS updated to match

---

## v0.7.15 — Update Auto-Recovers Dead Monitor Session (2026-03-29)

`squad-station update` now automatically recreates the monitor session if it is missing or dead, completing the full recovery from the v0.7.12 bug.

### Fixed

- **Monitor session not recovered after update** — Even after v0.7.13 stopped killing the monitor, a monitor that was already dead (killed by the v0.7.12 bug) would remain dead after every `update` call. `update` now calls `ensure_monitor()` which checks if `<project>-monitor` is alive and recreates it via `tmux create_view_session` if not. Prints `[MONITOR] <name> recreated` when it acts.
- **`ensure_monitor` is a no-op when monitor is alive** — safe to call unconditionally; only acts when the session is actually missing.

---

## v0.7.14 — Regression Guard for Update Housekeeping (2026-03-29)

Adds a regression test that enforces `run_housekeeping` must never kill any tmux sessions, preventing a repeat of the v0.7.12 monitor-killing bug.

### Added

- **Regression test `test_housekeeping_never_kills_any_sessions`** — `run_housekeeping` now returns `Vec<String>` (killed sessions) instead of `()`. The test asserts this Vec is always empty. If anyone re-adds `kill_session` to housekeeping, the test fails immediately with a descriptive message.
- **`debug_assert!` in production path** — additional runtime guard in the `run` function to catch violations in debug builds.
- **`run_housekeeping` is now `pub`** — enables direct testing without going through the full async `run` path.

---

## v0.7.13 — Fix Update Kills Monitor Session (2026-03-29)

Fixes a bug where `squad-station update` would kill the monitor tmux session without relaunching it, causing the monitor to go missing after every update call.

### Fixed

- **`squad-station update` kills monitor session** — `run_housekeeping` was killing `<project>-monitor` tmux session as part of cleanup but never relaunching it. Removed the kill-monitor logic since `update` has no responsibility over the monitor session — it should be left running untouched.

---

## v0.7.12 — Update Command (2026-03-29)

Adds `squad-station update` — a soft update that syncs a running squad with changes in `squad.yml` without tearing down existing sessions.

### Added

- **`squad-station update`** — Soft squad update command:
  - Diffs `squad.yml` agents against DB agents and categorises each as: `[NEW]`, `[REMOVED]`, `[WARN] provider changed`, or `[OK] unchanged`
  - Launches new agents that appear in `squad.yml` but not in DB
  - Kills and relaunches agents whose provider has changed
  - Skips agents currently processing a task — prints a warning instead of disrupting live work
  - Leaves unchanged agents running untouched
  - Re-runs hook installation and context regeneration (idempotent housekeeping) on every call
  - Orchestrator is never counted as a "removed" agent — managed separately
- **10 unit tests** covering all classify_changes edge cases and `has_processing_message` logic (TDD)

---

## v0.7.11 — Uninstall Command (2026-03-28)

Adds `squad-station uninstall` to cleanly remove squad-station from a project without leaving behind stale hooks, files, or running sessions.

### Added

- **`squad-station uninstall`** — Full project teardown command:
  - Kills all squad tmux sessions and stops watchdog daemon
  - Removes squad-station hook entries from provider settings (`.claude/settings.json`, `.codex/hooks.json`, `.gemini/settings.json`) — preserves non-squad hooks
  - Removes the bootstrap block (`<!-- squad-station:bootstrap-start/end -->`) from `CLAUDE.md` / `AGENTS.md` / `GEMINI.md`
  - Deletes `squad-orchestrator.md` from provider commands directory
  - Deletes `.squad/` directory entirely
  - Preserves `squad.yml` so users can re-init without reconfiguring
  - Confirmation prompt by default; `--yes` / `-y` to skip
- **10 unit tests** covering hook removal, bootstrap block removal, and provider path resolution

---

## v0.7.10 — Always Update SDD Playbooks (2026-03-28)

SDD playbooks are now always overwritten on every `npx squad-station install`, ensuring users always get the latest version without needing `--force`.

### Fixed

- **SDD playbooks not updating on reinstall** — Previously, `install` skipped `.squad/sdd/*.md` files if they already existed, silently leaving users on outdated playbooks. Now they are always overwritten since SDD files are managed entirely by squad-station and not user-editable.

---

## v0.7.9 — Fix Codex Launch Flag (2026-03-28)

Fixes the Codex agent launch command from `--full-auto` to `--yolo`.

### Fixed

- **Codex launch flag** — `squad-station init` now launches Codex agents with `codex --yolo` (was incorrectly using `codex --full-auto` which is not a valid flag)

---

## v0.7.8 — OpenAI Codex Provider (2026-03-28)

Adds OpenAI Codex CLI as a first-class provider alongside `claude-code` and `gemini-cli`. Codex agents can now be orchestrated end-to-end: auto-installed hooks, model validation, launch command generation, and context injection.

### Added

- **Codex provider support** — `provider: codex` is now a valid first-class provider in `squad.yml`
- **7 Codex model slugs** — validated in config: `gpt-5.4`, `gpt-5.4-mini`, `gpt-5.3-codex`, `gpt-5.2-codex`, `gpt-5.2`, `gpt-5.1-codex-max`, `gpt-5.1-codex-mini`
- **Auto hook installation** — `squad-station init` writes `.codex/hooks.json` with `Stop` (completion signal) and `PostToolUse` (Bash tool notifications) hooks
- **Session start hook** — Optional `SessionStart` hook installs context injection for Codex agents
- **Launch command** — `codex --full-auto` (with `--model <slug>` when model is specified in config)
- **Context generation** — `squad-station context` generates `squad-orchestrator.md` at `.codex/commands/` for Codex orchestrators
- **Behavior flags** — Full `providers.rs` coverage: `Stop` completion event, no JSON stdout requirement, no alternate buffer, `/clear` as fire-and-forget
- **14 new tests** — Covering provider validation, hook structure, launch commands, session start, and context paths

### Changed

- **`providers.rs` comment** — Removed stale "v0.7.0 consideration: refactor to Provider trait" note
- **`config.rs` test** — `unknown_provider_warns_but_succeeds` now uses `"aider"` as example (not `"codex"` which is now known)

---

## v0.7.7 — Documentation Rewrite (2026-03-28)

Comprehensive rewrite of both the root README.md and npm-package README.md for the v0.7.x release.

### Changed

- **Root README.md** — Rewritten to reflect current architecture, commands, and workflow
- **npm-package README.md** — Updated installation and usage instructions for v0.7.x

---

## v0.7.6 — Orchestrator Tiered Tool Restrictions (2026-03-25)

Added tiered tool model for the orchestrator template — orchestrator acts as a PM who reads dashboards, not a developer who reads code.

### Fixed

- **Orchestrator self-research gap** — The generated `squad-orchestrator.md` forbade writing code but did not prevent the orchestrator from using Read, Bash, grep, or other tools to self-research. Result: orchestrator would read files and run git commands directly instead of delegating to agents.

### Added

- **Tiered tool restrictions section** — New "Tool Restrictions — Tiered" section in the generated orchestrator template with two tiers:
  - **ALLOWED** (no delegation): squad-station CLI, tmux capture-pane (post-signal), SDD playbooks, tracking/status files (sprint-status.yaml, epics.md, REQUIREMENTS.md, CHANGELOG), basic git status/branch for orientation
  - **MUST DELEGATE**: source code reading, deep git research (log/diff/blame), code search (grep/Glob), report generation, tests/builds, file writes, Agent subagents

### Changed

- **QA Gate step 3** — Orchestrator now answers agent technical questions from dashboard knowledge first, delegating research only when needed (previously always delegated)

---

## v0.7.5 — Remove RECONCILE & Simplify Watchdog (2026-03-25)

Removes RECONCILE logic that was prematurely completing tasks and causing signal loss. Strips the watchdog down to a pure health monitor.

### Fixed

- **RECONCILE causing signal loss** — RECONCILE was marking tasks as completed BEFORE agents actually finished. When the real stop hook fired, it found no pending task and skipped orchestrator notification. Removed all task-completion logic from both `reconcile` (when called by watchdog) and the watchdog itself.

### Changed

- **Watchdog is now health monitor only** — The watchdog's sole responsibility is checking tmux session liveness: mark agents "dead" if their session crashed, revive to "idle" if session reappears. Removed: BusyAlertState, tiered escalation (Tier 1–3), pane idle detection, pane content scanning, orchestrator notifications via send-keys, and auto-heal logic.
- **Reconcile command simplified** — The `reconcile` CLI command still performs orphan reset (busy + zero processing messages) and dead agent detection, but no longer attempts task completion based on pane idle heuristics. Removed `pane_looks_idle()`, `capture_pane_alternate()`, and `[SQUAD RECONCILE]` orchestrator notification.

### Removed

- `BusyAlertState` struct and 4 associated tests
- `pane_looks_idle()`, `capture_pane()`, `capture_pane_alternate()` from reconcile module
- 4 pane idle pattern tests from reconcile module
- All `[SQUAD RECONCILE]` and `[SQUAD WATCHDOG]` auto-heal notifications

---

## v0.7.4 — Bootstrap Path Fix & Playbook Compliance (2026-03-24)

Fixes bootstrap block writing to the wrong file path and strengthens orchestrator compliance language.

### Fixed

- **Bootstrap block file path** — `inject_bootstrap_block()` was writing to `.claude/CLAUDE.md` and `.gemini/GEMINI.md` inside subdirectories, but Claude Code reads `CLAUDE.md` from the project root and Gemini reads `GEMINI.md` from the project root. Fixed `provider_doc_paths()` to return the correct root-level paths.

### Changed

- **Bootstrap block wording** — Replaced passive "Read playbook" with explicit directive: playbook defines WORKING RULES, must not be treated as optional, do NOT invent your own approach.
- **PRE-FLIGHT SDD wording** — Reworded from "Read and fully internalize" to "These define your WORKING PRINCIPLES" with mandatory compliance language throughout the session.

---

## v0.7.3 — Orchestrator Bootstrap & Context Cleanup (2026-03-24)

Ensures the orchestrator automatically knows its role after `/clear` and context compact — no manual re-prompting needed.

### Added

- **Orchestrator bootstrap block** — `squad-station init` now injects a lightweight bootstrap section into the provider's project doc file (`.claude/CLAUDE.md` for Claude Code, `.gemini/GEMINI.md` for Gemini CLI). This file is always loaded into context, surviving `/clear` and context compact. Includes a tmux session-name guard so worker agents ignore it. Idempotent: uses HTML marker comments for replace-on-reinit.

### Changed

- **Removed Autonomous Mode section** from generated orchestrator context — decision authority, escalation rules, and driving-to-completion instructions are now the responsibility of the SDD playbook, not hardcoded in the orchestrator template.
- **Removed capture-pane polling hint** — the line "Only proactively check (capture-pane) if you suspect the agent is stuck" was encouraging orchestrators to poll agent output instead of waiting for the completion signal.

---

## v0.7.2 — npm Installer Hardening (2026-03-24)

Fixes npm installer issues that prevented clean upgrades and macOS Gatekeeper blocks on downloaded binaries.

### Fixed

- **Binary upgrade removes stale symlinks** — `npx squad-station install` now unlinks the old binary before downloading when a version mismatch is detected. Fixes upgrade failures when `~/.cargo/bin/squad-station` is a symlink from `cargo install`.
- **macOS Gatekeeper bypass** — Strips `com.apple.quarantine` and `com.apple.provenance` xattr after downloading the binary, preventing "cannot be opened because Apple cannot check it for malicious software" errors.

### Added

- **Rules scaffolding in npm installer** — `npx squad-station install` now copies `.squad/rules/` git workflow templates alongside existing sdd/ and examples/ scaffolding.

---

## v0.7.0 — SDD Git Workflow Rules (2026-03-24)

Auto-install SDD git workflow rules during squad initialization, plus three watchdog reliability fixes.

### Added

- **SDD git workflow rules auto-install** — During `squad-station init`, for each SDD entry in squad.yml, copies the matching rule template from `.squad/rules/git-workflow-<name>.md` into provider-specific rules directories (`.claude/rules/`, `.gemini/rules/`). Ships with 4 built-in rule templates: get-shit-done, bmad-method, openspec, superpowers.
- **SDD templates versioned** — `.squad/rules/`, `.squad/sdd/`, and `.squad/examples/` are now tracked in git. Runtime files (station.db, logs, PID) remain ignored via `.gitignore` whitelisting.

### Fixed

- **Orphan busy state reset** — Reconcile and watchdog now detect agents marked "busy" in DB with zero processing messages (signal completed the task but failed to reset status). Resets to idle immediately without heuristics.
- **Pane capture window 5→20 lines** — `pane_looks_idle()` captured only 5 lines, missing Claude Code's prompt behind 4-5 status bar lines. Switched from `-l 5` to `-S -20` for broader tmux version compatibility.
- **Signal completes all processing messages on stop** — When orchestrator rapid-fires N tasks, agent processes all in one turn but only one Stop hook fires. Now uses `complete_all_processing()` to prevent N-1 orphaned "processing" messages.

---

## v0.6.9 — Remove Idle Nudge (2026-03-23)

Simplifies watchdog by removing idle nudge notifications. Watchdog now focuses solely on stuck-agent detection with tiered escalation.

### Changed

- **Watchdog simplified** — Removed idle nudge (Pass 2) that sent "System idle for Xm" notifications to the orchestrator. Watchdog now only monitors for stuck agents: log-only at 10m, auto-heal at 30m, orchestrator alert at 60m.
- **`--stall-threshold` hidden** — CLI arg kept for backwards compatibility but hidden from help output (no longer functional).

### Fixed

- **Docs updated** — SYSTEM-DESIGN.md now includes `watch` and `doctor` in CLI reference, `watch.rs` in architecture modules, and updated release history. README watchdog description updated to reflect tiered stuck-agent detection.

---

## v0.6.8 — Lean SDD Playbooks (2026-03-23)

Trims SDD playbooks to agent-essential content and adds OpenSpec as a supported SDD framework.

### Changed

- **SDD playbooks trimmed 84%** — Removed installation guides, troubleshooting, mermaid diagrams, verbose prose, and external links from bmad-playbook.md, gsd-playbook.md, and superpowers-playbook.md. Each file now contains only workflow sequences, command reference tables, and critical rules.

### Added

- **OpenSpec SDD playbook** — New `openspec-playbook.md` (74 lines) supporting OpenSpec's spec-driven workflow (propose → apply → archive) with core and expanded profiles
- **OpenSpec in example configs** — Added OpenSpec as a commented-out SDD option in orchestrator-claude.yml and orchestrator-gemini.yml examples

---

## v0.6.7 — Hook Log Redirect Fix (2026-03-23)

Fixes hook command failures caused by relative shell redirects resolving against the wrong working directory.

### Fixed

- **Hook stderr redirect path** — `squad-station init` generated hook commands with `2>>.squad/log/signal.log` which fails when Claude Code's hook runner CWD differs from the project root. Replaced with `2>/dev/null` since `signal.rs` handles logging internally via `log_signal()`. Gemini hooks similarly updated from `>>.squad/log/signal.log 2>&1` to `>/dev/null 2>&1`.
- **notify.rs hook safety** — `notify` command used `anyhow::bail!` on errors (non-zero exit), which could break provider hook contracts. Now always exits 0 with best-effort logging via `log_notify()`, matching the `signal.rs` pattern.

### Changed

- Updated SYSTEM-DESIGN.md GUARD flowchart and prose to reflect `/dev/null` redirect pattern

---

## v0.6.6 — Stale Busy Fix (2026-03-23)

Fixes false positive watchdog warnings caused by orphaned processing messages and missed idle detection.

### Fixed

- **current_task overwrite in send.rs** — when a second task was sent while the first was still processing, `set_current_task` blindly overwrote the FK to the newer message. Signal then completed the wrong message, orphaning the original in `processing` forever and leaving the agent stuck in `busy` state. Now only sets `current_task` if no task is currently assigned; queued tasks are picked up by signal's remaining-processing check.
- **Idle pane detection in reconcile.rs** — `pane_looks_idle` only checked the last non-empty line for the `❯` prompt pattern. Claude Code's TUI renders a status bar below the prompt, so the last line was always status info, never the prompt. Now scans all 5 captured lines for idle patterns.

### Added

- Regression test `test_second_send_does_not_overwrite_current_task` reproducing the exact production incident

---

## v0.6.5 — Async Pattern Fixes and Batch DB Queries (2026-03-23)

Fixes blocking async patterns in tmux operations and optimizes the status command with a batch database query.

### Fixed

- **Async sleep in tmux.rs** — converted 3 instances of `std::thread::sleep()` to `tokio::time::sleep().await` in `send_keys_literal()`, `inject_single()`, and `inject_body()`. These were blocking the Tokio executor for 2–5 seconds per call, preventing other async tasks from making progress.
- **Clippy warnings** — resolved 7 clippy lints: empty doc comment line, `push_str` → `push` for single char, `match` → `matches!` macro (3×), needless borrows (2×)

### Changed

- **Batch DB query in status command** — added `count_processing_per_agent()` single `GROUP BY` aggregate query replacing N sequential `list_messages()` calls (one per agent). Scales O(1) instead of O(N) with agent count.
- Updated 9 callers across 5 command files (`send.rs`, `notify.rs`, `signal.rs`, `reconcile.rs`, `watch.rs`) to await the now-async tmux functions

### Added

- 3 new unit tests for `count_processing_per_agent()` covering empty DB, single agent, and multiple agents

---

## v0.6.4 — Smart PATH Detection for npm Installer (2026-03-23)

npm installer now picks install directories already in PATH, adds cross-platform PATH instructions, and adds Windows support.

### Added

- **Smart PATH detection** — `findBestInstallDir()` scans PATH for writable directories (`/usr/local/bin`, `~/.local/bin`, `~/bin`) before falling back to `~/.squad-station/bin`, eliminating manual PATH configuration on most systems
- **Cross-platform PATH instructions** — when the install directory is not in PATH, prints shell-specific instructions (bash/zsh/fish/PowerShell) for adding it
- **Windows support** — npm installer handles `.exe` suffix, uses PowerShell for downloads, and supports Windows PATH directories

### Changed

- **Release process codified** — added `/release` slash command (`.claude/commands/release.md`) documenting the full 7-step release checklist with lessons learned

---

## v0.6.3 — npm Installer Binary Fix (2026-03-23)

Fixes the npm installer so the downloaded binary is executable and the npm package works correctly out of the box.

### Fixed

- **chmod +x bin/run.js** — npm entry point was not executable after install
- **npm package fixes** — corrected package configuration for reliable `npx squad-station install` flow

---

## v0.6.2 — Post-Init Health Check, Autonomous Orchestrator, Doctor Command (2026-03-23)

Adds a comprehensive post-init health check that validates 9 components, a standalone `doctor` diagnostic command, autonomous orchestrator mode with clear decision authority boundaries, and fixes the watchdog self-detection race condition.

### Added

- **Post-init health check** — validates 9 components after `squad-station init`: database, log directory, signal hooks, notify hooks (per provider), orchestrator context file, tmux sessions (orchestrator + each agent), and watchdog daemon. Prints pass/fail/warn summary with actionable remediation steps.
- **`squad-station doctor` command** — standalone health check for diagnosing squad operational issues without re-running init. Exits with code 1 if any checks fail.
- **Autonomous orchestrator mode** — new "Autonomous Mode" section in generated `squad-orchestrator.md`:
  - **Decision authority** — orchestrator makes routing, implementation, testing, and technical trade-off decisions without asking the user
  - **Escalation criteria** — only escalate for ambiguous requirements, destructive actions, external dependencies, or scope conflicts
  - **Driving to completion** — orchestrator dispatches follow-up tasks on errors, answers agent questions, and verifies work before reporting done
- **11 E2E lifecycle tests** (`tests/test_e2e_lifecycle.rs`) — covers watchdog daemon lifecycle (start/stop/duplicate/stale PID/logging), init artifact creation, init idempotency, doctor exit codes, and watchdog self-detection regression

### Fixed

- **Watchdog self-detection race condition** — daemon was killing itself immediately after start because it read its own PID from the PID file and treated it as a duplicate. Now compares PID file contents against `std::process::id()` and skips the duplicate check when they match.

### Changed

- **QA Gate instructions refined** — error handling now instructs orchestrator to analyze and fix errors autonomously; technical questions answered from project context; only genuinely ambiguous requirements escalated to user

---

## v0.6.1 — Signal Hook Fix & Watchdog Self-Healing (2026-03-22)

Fixes the critical signal hook failure where `$SQUAD_AGENT_NAME` was never available in hook subprocess contexts, causing silent signal drops. Adds tiered watchdog self-healing that auto-recovers stuck agents.

**168 tests passing** (89 lib + 79 integration).

### Fixed

- **Signal hook agent name resolution** (BUG-01, CRITICAL) — switched from `$SQUAD_AGENT_NAME`/`tmux list-panes` to `tmux display-message -p '#S'`, a tmux server-side query that works reliably in all hook subprocess contexts (Claude Code Stop hooks, Gemini CLI AfterAgent). The env var approach from v0.6.0 never worked because hook subprocesses don't inherit the shell's exported variables.
- **GUARD-1 silent failure** (BUG-02) — empty agent name now logged to `.squad/log/signal.log` + stderr before exit 0, instead of silently swallowed with zero forensic evidence
- **Watchdog daemon dies silently** (BUG-06) — new `ensure_watchdog()` health check called opportunistically from `signal` and `send`; detects dead PID and respawns daemon
- **Watchdog stderr sent to /dev/null** (BUG-07) — daemon stderr now redirected to `.squad/log/watch-stderr.log` for crash diagnostics
- **Watchdog Pass 3 observe-only** (BUG-08) — prolonged busy detection upgraded from log-only to tiered escalation with corrective actions
- **Watchdog killed by terminal close** (BUG-10) — daemon now calls `setsid()` to create a new session, surviving SIGHUP from parent terminal

### Added

- **Tiered watchdog busy detection** — 4-level escalation for stuck agents:
  - 10-30min: log only (long tasks are normal)
  - 30min+: auto-heal if pane is idle (complete stuck tasks, reset to idle, notify orchestrator)
  - 60min+: alert orchestrator with WARNING (10min per-agent cooldown)
  - 120min+: escalate to URGENT prefix
- **Pane content snapshot logging** — Tier 2 idle detection logs last 5 lines of pane content to `watch.log` for diagnosing false positives
- **`complete_all_processing()` DB function** — batch-completes all processing messages for an agent; used by watchdog self-healing
- **`BusyAlertState`** — per-agent notification cooldown to prevent orchestrator notification spam
- **`spawn_watchdog_daemon()` shared helper** — extracted from `watch --daemon` and `ensure_watchdog` to eliminate duplication; configures setsid, stderr-to-log, stdin/stdout null
- **Unit tests** — `BusyAlertState` (4 tests), `complete_all_processing` (2 tests), `test_guard1_logs_empty_agent_name`

### Changed

- **Hook commands simplified** — removed intermediate `$AGENT` variable, `[ -n "$AGENT" ]` shell guard, `$SQUAD_AGENT_NAME`, `$TMUX_PANE`, and `tmux list-panes` fallback. Signal.rs GUARD-1 handles empty names with logging — no shell-level guard needed.
- **`agent_resolve_snippet()` renamed to `agent_name_subshell()`** — returns `$(tmux display-message -p '#S' 2>/dev/null)` instead of the old multi-stage resolution
- **`pane_looks_idle()` visibility** — narrowed from `fn` to `pub(crate)` for watchdog Tier 2 access
- **`capture_pane()` visibility** — narrowed from `fn` to `pub(crate)` for watchdog pane snapshot logging
- **`// SAFETY:` comments** — added to all `unsafe` blocks (`setsid`, `kill`, `signal`)

### Removed

- **Vendored GSD plugin files** — removed ~104k lines of `.claude/agents/`, `.claude/commands/gsd/`, `.claude/get-shit-done/`, `.gemini/`, `.planning/` directories
- **`$SQUAD_AGENT_NAME` env var** — never available in hook contexts; removed entirely

### Documentation

- **SYSTEM-DESIGN.md** — updated section 5.2 (guard chain) and 5.3 (hook commands) to reflect new `tmux display-message` pattern with stderr redirection
- **PLAYBOOK.md** — added section 4 (Watchdog) documenting tiered escalation, resilience features, and log files; updated troubleshooting
- **Change analysis archived** — `docs/changes/archive/bug-analysis-signal-watchdog.md` and `solution-signal-watchdog.md`

---

## v0.6.0 — Signal Reliability (2026-03-20)

Three-layer defense against lost agent completion signals. Zero-config hook setup, project-scoped logging, and a self-healing watchdog daemon.

**233 tests passing** (84 lib + 149 integration). E2E validated on kindle-ai-export with 3 running Claude Code agents.

### Added

- **`squad-station reconcile` command** — reconcile agent statuses against live tmux sessions; supports `--dry-run` and `--json` output
- **`squad-station watch` daemon** — background watchdog with 3-pass detection: individual agent reconcile, global stall detection with orchestrator nudge, and prolonged busy warnings; auto-starts on `init`
- **`clean --all` flag** — deletes logs in addition to DB and sessions
- **`providers.rs` module** — centralized provider metadata (idle patterns, hook events, settings paths, fire-and-forget prefixes, alternate buffer detection)
- **`$SQUAD_AGENT_NAME` environment variable** — set in each tmux session at launch for reliable hook identification; eliminates fragile `tmux display-message` in subprocess contexts
- **Project-scoped signal logging** — all signal events logged to `.squad/log/signal.log` with RFC3339 timestamps, level (OK/WARN/GUARD), agent name, and structured context
- **Watchdog logging** — daemon activity logged to `.squad/log/watch.log` with nudge tracking and stall detection
- **Log rotation** — signal.log auto-truncates to 500 lines when exceeding 1MB
- **Signal uses `current_task` FK** — targeted completion of the exact task being worked on, with FIFO fallback safety net when `current_task` is NULL
- **DB layer functions** — `set_current_task`, `clear_current_task`, `complete_by_id`, `last_completed_id`, `complete_message_by_id`, `count_processing_all`, `total_count`, `last_activity_timestamp`
- **Hook templates upgraded** — Claude Code: Stop + Notification + PostToolUse with `$SQUAD_AGENT_NAME` and stderr-to-log redirection; Gemini CLI: AfterAgent + Notification with JSON stdout compliance and 30s timeout

### Changed

- **Signal flow rewritten** — primary path uses `current_task` FK for targeted completion instead of FIFO queue; FIFO retained as fallback with WARN-level logging
- **Fire-and-forget commands** (`/clear`) no longer set `current_task` — prevents corruption when `/clear` overlaps an in-flight real task
- **Hook resolution** — switched from `tmux display-message -p '#S'` to `$SQUAD_AGENT_NAME` env var with `tmux list-panes` fallback
- **Init command** — now creates `.squad/log/` directory, auto-starts watchdog, installs hooks for all providers in the squad (not just orchestrator)
- **Clean command** — stops watchdog daemon before deleting DB to prevent crash loops

### Fixed

- **current_task corruption** when `/clear` overlaps in-flight task — current_task now correctly reverts to the real task (v0.5.8)
- **Signal race condition** — `/clear` followed by a real task no longer leaves the real task stuck at `processing` (v0.5.7)
- **Shell injection in session names** — `sanitize_session_name` now strips all shell metacharacters (`' ` `` ` `` `$ ; () | & <> \ /` space newline null), not just `.` `:` `"` (PR #2)
- **Unquoted model value in launch commands** — model values validated against `[a-zA-Z0-9._-:]` before shell interpolation (PR #2)
- **Clean command misses sessions** — `compute_session_names` now calls `sanitize_session_name` to match init naming (PR #2)
- **Signal exit-0 violation** — `get_agent` error no longer propagates as non-zero exit; uses soft guard matching the rest of the function (PR #2)
- **Antigravity agents marked dead** — `reconcile_agent_statuses` now skips `tool="antigravity"` agents that never have tmux sessions (PR #2)
- **inject_body corrupts task text** — `&&` splitting now only triggers when ALL parts are slash commands; plain text like "check if A && B" is sent as-is (PR #2)
- **Orphan WAL/SHM files** — `delete_db_file` now removes `station.db-wal` and `station.db-shm` alongside the main DB (PR #2)

### Security

- **sanitize_session_name hardened** — prevents shell injection via crafted session names in `sh -c` view commands
- **Model value validation** — blocks injection via malicious `model` field in `squad.yml`

### Removed

- Raw SQL queries outside `src/db/` — all moved to db layer functions

## v0.5.8 - 2026-03-20

### 🐛 Bug Fixes

- Fixed `current_task` corruption when `/clear` is sent while another task is already processing — `current_task` now reverts to the real task instead of staying pointed at the completed `/clear` message

### 🧪 Tests

- Added `test_fire_and_forget_clear_while_task_processing` integration test — verifies `current_task` and agent status are correct when fire-and-forget overlaps with an in-flight task

## v0.5.7 - 2026-03-20

### 🐛 Bug Fixes

- Fixed signal race condition: `/clear` followed by a real task no longer leaves the real task stuck at `processing` forever
- Root cause: `/clear` in Claude Code produces no response turn, so the Stop hook never fires — its DB message blocked the FIFO signal queue, causing subsequent signals to complete the wrong task
- Fire-and-forget commands (e.g. `/clear`, `/clear hard`) are now auto-completed at send time in `send.rs`
- Agent status correctly resets to idle after fire-and-forget if no other tasks are queued

### 🧪 Tests

- Added `is_fire_and_forget` unit tests (positive + negative cases)
- Added `test_fire_and_forget_clear_auto_completed` integration test — reproduces the exact race condition and verifies signal targets the correct task

## v0.5.6 - 2026-03-20

### 🌟 Highlights

- `/clear` context management upgraded from vague guidance to **hard rules** — weaker models (Haiku) no longer ignore `/clear` decisions

### 🎁 Features

- Mandatory `/clear` triggers: topic shift, 3-task threshold, agent hint detection
- Pre-send checklist added to orchestrator playbook — run before every `squad-station send`
- Explicit `How to /clear` section with code example
- QA Gate step 5 now says "Run the `/clear` checklist" instead of "Decide if `/clear` is needed"

### 🔧 Maintenance

- Version bumped to 0.5.6 across Cargo.toml, npm-package/package.json, and bin/run.js binary download
- npm binary download version aligned to 0.5.6 (was stuck at 0.5.3)

## v0.5.5 - 2026-03-19

### 🌟 Highlights

- Orchestrator context can now be **auto-injected** on session start, resume, or compact — no more forgetting to run `/squad-orchestrator`
- CLI simplified: `close` removed, `clean` now does everything (kill sessions + delete DB)
- New orchestrator guidance for managing agent context with `/clear`

### 🎁 Features

- `squad-station context --inject` outputs orchestrator content to stdout for SessionStart hook consumption
- Orchestrator-only guard: detects tmux session name and silently skips injection for worker agents
- Provider-aware output format: raw markdown for Claude Code, JSON `hookSpecificOutput.additionalContext` for Gemini CLI
- Opt-in SessionStart hook during `squad-station init` with interactive prompt (default: No)
- New "Context Management — /clear" section in orchestrator playbook
- QA Gate now includes step 5: "Decide if `/clear` is needed before the next task"

### 💥 Breaking Changes

- `squad-station close` command removed — use `squad-station clean` instead
- `squad-station clean` now kills all tmux sessions AND deletes the database (previously only deleted the database)

### 🔧 Maintenance

- Version aligned to 0.5.5 across Cargo.toml and npm-package/package.json
- Updated SDD playbooks in npm-package
- 171 tests passing

## v0.5.3 - 2026-03-16

### 🌟 Highlights

- New PostToolUse hook catches agent questions (AskUserQuestion) and forwards them to the orchestrator
- Elicitation dialog support for permission-like prompts

### 🎁 Features

- PostToolUse hook: `AskUserQuestion` matcher notifies orchestrator when an agent asks a question
- Notification hook: added `elicitation_dialog` matcher alongside `permission_prompt`
- Orchestrator resolution fix for multi-agent squads

### 📚 Documentation

- Added README to npm-package

### 🔧 Maintenance

- `cargo fmt` formatting pass across source and tests
- 164 tests passing

## v0.5.1 - 2026-03-16

### 🌟 Highlights

- First public release as an npm package (`npx squad-station install`)
- Provider-agnostic hook system with auto-installation
- Colored, informative init output

### 🎁 Features

- `npx squad-station install` — npm package with postinstall binary download for macOS and Linux
- Colored init output with squad setup summary, hook status, and get-started instructions
- Gemini CLI hooks: AfterAgent (signal) and Notification (notify) auto-installed to `.gemini/settings.json`
- Claude Code hooks: Stop (signal) and Notification (permission_prompt) auto-installed to `.claude/settings.json`
- Gemini CLI slash command generated in TOML format (`.gemini/commands/squad-orchestrator.toml`)
- Provider-specific orchestrator context file paths resolved dynamically
- Freeze/unfreeze commands to block or allow orchestrator task dispatch
- Monitor session: tiled tmux view of all agent panes created during init
- Context command: generates unified `squad-orchestrator.md` with agent roster, routing rules, and playbook references
- Signal command: auto-detects agent from tmux pane ID, idempotent completion handling
- Full messaging pipeline: send, peek, list, signal with priority ordering (urgent > high > normal)
- SQLite WAL mode with single-writer pool and 5s busy timeout
- Literal-mode `send-keys` to prevent shell injection via tmux
- Antigravity provider support (DB-only orchestrator, no tmux session)
- SDD workflow orchestration: playbook-driven task delegation to agents
- Interactive TUI dashboard (ratatui) for monitoring agent status and messages

### 🔧 Maintenance

- Rust CLI with clap argument parsing, async tokio runtime, sqlx migrations
- 160+ tests (unit + integration)
- CI workflow for tests, clippy, and fmt
- curl-pipe-sh installer script
- MIT license
