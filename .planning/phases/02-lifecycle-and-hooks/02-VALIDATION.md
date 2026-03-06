---
phase: 2
slug: lifecycle-and-hooks
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-06
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust test (`cargo test`) + tokio-test 0.4 |
| **Config file** | Cargo.toml `[dev-dependencies]` — no separate config |
| **Quick run command** | `cargo test --test test_db` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --test test_db`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | SESS-03 | unit | `cargo test test_update_agent_status` | ❌ W0 | ⬜ pending |
| 02-01-02 | 01 | 1 | SESS-03 | unit | `cargo test test_format_status_with_duration` | ❌ W0 | ⬜ pending |
| 02-01-03 | 01 | 1 | SESS-04 | unit | `cargo test test_reconcile_dead_agent` | ❌ W0 | ⬜ pending |
| 02-01-04 | 01 | 1 | SESS-04 | unit | `cargo test test_reconcile_revived_agent` | ❌ W0 | ⬜ pending |
| 02-02-01 | 02 | 1 | SESS-05 | unit | `cargo test test_context_output_contains_agents` | ❌ W0 | ⬜ pending |
| 02-02-02 | 02 | 1 | SESS-05 | unit | `cargo test test_context_output_has_usage` | ❌ W0 | ⬜ pending |
| 02-03-01 | 03 | 2 | HOOK-01 | unit | `cargo test test_signal_skips_orchestrator` | ❌ W0 | ⬜ pending |
| 02-03-02 | 03 | 2 | HOOK-03 | unit | `cargo test test_signal_guard_no_tmux` | ❌ W0 | ⬜ pending |
| 02-03-03 | 03 | 2 | HOOK-03 | unit | `cargo test test_signal_guard_unregistered` | ❌ W0 | ⬜ pending |
| 02-03-04 | 03 | 2 | HOOK-03 | unit | `cargo test test_signal_guard_db_error` | ❌ W0 | ⬜ pending |
| 02-04-01 | 04 | 2 | HOOK-02 | shell/manual | Manual — shell script testing | N/A | ⬜ pending |
| 02-04-02 | 04 | 2 | HOOK-02 | shell/manual | Manual — shell script testing | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/test_db.rs` — add `test_update_agent_status`, `test_format_status_with_duration`
- [ ] `tests/test_lifecycle.rs` — new file: reconcile tests, signal guard tests, context output tests
- [ ] HOOK-02 shell scripts: manual verification only

*Existing infrastructure (`setup_test_db()`, `#[tokio::test]`, `tempfile`) covers all framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| claude-code.sh exits 0 in all cases | HOOK-02 | Shell script with tmux dependency | Run script outside tmux, verify exit 0; run inside tmux with unregistered agent, verify exit 0 |
| gemini-cli.sh exits 0 in all cases | HOOK-02 | Shell script with tmux dependency | Same as above for gemini script |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
