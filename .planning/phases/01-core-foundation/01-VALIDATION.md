---
phase: 1
slug: core-foundation
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-06
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | tokio-test 0.4 (already in dev-dependencies) |
| **Config file** | none — Wave 0 creates test structure |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test -- --include-ignored` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo test -- --include-ignored`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 0 | SESS-01 | integration | `cargo test test_init_creates_db` | ❌ W0 | ⬜ pending |
| 01-01-02 | 01 | 0 | SESS-01 | integration | `cargo test test_init_idempotent` | ❌ W0 | ⬜ pending |
| 01-01-03 | 01 | 0 | SESS-02 | unit | `cargo test test_register_agent` | ❌ W0 | ⬜ pending |
| 01-01-04 | 01 | 0 | MSG-01 | unit | `cargo test test_send_creates_message` | ❌ W0 | ⬜ pending |
| 01-01-05 | 01 | 0 | MSG-02 | unit | `cargo test test_signal_updates_status` | ❌ W0 | ⬜ pending |
| 01-01-06 | 01 | 0 | MSG-03 | unit | `cargo test test_signal_idempotent` | ❌ W0 | ⬜ pending |
| 01-01-07 | 01 | 0 | MSG-04 | unit | `cargo test test_list_filters` | ❌ W0 | ⬜ pending |
| 01-01-08 | 01 | 0 | MSG-05 | unit | `cargo test test_priority_ordering` | ❌ W0 | ⬜ pending |
| 01-01-09 | 01 | 0 | MSG-06 | unit | `cargo test test_peek_returns_pending` | ❌ W0 | ⬜ pending |
| 01-01-10 | 01 | 0 | SAFE-01 | integration | `cargo test test_concurrent_signals` | ❌ W0 | ⬜ pending |
| 01-01-11 | 01 | 0 | SAFE-02 | unit | `cargo test test_tmux_args_literal` | ❌ W0 | ⬜ pending |
| 01-01-12 | 01 | 0 | SAFE-03 | unit | `cargo test test_launch_uses_new_session` | ❌ W0 | ⬜ pending |
| 01-01-13 | 01 | 0 | SAFE-04 | unit | `cargo test test_sigpipe_installed` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/` directory — create as Rust integration test directory
- [ ] `tests/helpers.rs` — shared test DB setup (in-memory SQLite pool)
- [ ] `tests/test_messages.rs` — covers MSG-01 through MSG-06
- [ ] `tests/test_concurrent.rs` — covers SAFE-01
- [ ] `src/tmux.rs` test module — unit tests for Command args (SAFE-02, SAFE-03)
- [ ] Framework install: none needed — tokio-test already in dev-dependencies

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| tmux session actually launches agent | SESS-01 | Requires real tmux | Run `squad-station init` with test squad.yml, verify `tmux ls` shows sessions |
| send-keys delivers prompt to tmux pane | MSG-01 | Requires real tmux | Run `squad-station send <agent> "test"`, verify text appears in pane |
| Signal notification appears in orchestrator pane | MSG-02 | Requires real tmux | Run `squad-station signal <agent>`, verify notification in orchestrator pane |

*All core logic paths have automated verification. Manual tests only for tmux integration.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
