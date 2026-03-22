# Solution Design: Signal Hook Reliability & Watchdog Self-Healing

**Date:** 2026-03-22
**Status:** Implemented
**Fixes:** BUG-01, BUG-02, BUG-06, BUG-07, BUG-08, BUG-10

---

## Problem 1: Hook Agent Name Resolution

### Current State (before fix)

The hook command used a two-stage resolution:
```bash
AGENT=${SQUAD_AGENT_NAME:-$(tmux list-panes -t "$TMUX_PANE" -F '#S' 2>/dev/null | head -1)}
```

**Failure chain we observed:**
1. `SQUAD_AGENT_NAME` — set via `export` in tmux shell (`tmux.rs:372`), but Claude Code hooks run in **child subprocesses** that do NOT inherit the shell's exported variables. Always empty in hook context.
2. `TMUX_PANE` — available in the tmux session shell, but NOT guaranteed in hook subprocesses (provider-dependent). Works intermittently.
3. Result: both fail → agent name is empty → GUARD-1 (`signal.rs:52`) returns `Ok(())` silently → no log, no signal, task stuck forever.

**What we verified works:**
- `tmux display-message -p '#S'` — works reliably in BOTH contexts (Claude Code hooks and `tmux run-shell`). This is a tmux **server** command that doesn't depend on inherited environment variables. It resolves the session name from the tmux client context, which IS available in hook subprocesses.

### Design Decision

**Switch to `tmux display-message -p '#S'` as the SOLE resolution method.**

Rationale:
- It's a tmux server-side query, not an env var lookup
- Works in every context where there's a tmux client connection (which is true for all hook scenarios — hooks run inside tmux sessions)
- The previous concern about `display-message` being "fragile" (documented in `hooks/gemini-cli.sh:1-6`) was about running OUTSIDE tmux. Inside tmux hooks, it's actually the most reliable method.

**`$SQUAD_AGENT_NAME` override removed** — it was never available in hook context (the one place it matters), and keeping it added complexity for a case (CI, testing) that doesn't exist in practice. The team confirmed this simplification.

### Implemented Hook Command

```bash
# Claude Code Stop hook (actual — simplified, no shell guards)
squad-station signal "$(tmux display-message -p '#S' 2>/dev/null)" 2>>.squad/log/signal.log

# Gemini CLI AfterAgent hook (actual — with JSON stdout wrapper)
squad-station signal "$(tmux display-message -p '#S' 2>/dev/null)" >>.squad/log/signal.log 2>&1; printf '{}'
```

Changes from previous:
1. `tmux list-panes -t "$TMUX_PANE" -F '#S'` → `tmux display-message -p '#S'`
2. Removes dependency on `$TMUX_PANE` and `$SQUAD_AGENT_NAME` environment variables entirely
3. Removes intermediate `$AGENT` variable and `[ -n "$AGENT" ]` shell guard — signal.rs GUARD-1 handles empty names with logging
4. Much simpler command — easier to understand and debug

### Code Changes (Implemented)

#### File: `src/commands/init.rs`

**Change 1:** Replaced `agent_resolve_snippet()` — now returns just the tmux subshell:
```rust
fn agent_resolve_snippet() -> &'static str {
    r#"$(tmux display-message -p '#S' 2>/dev/null)"#
}
```

**Change 2:** Claude hook commands simplified to direct invocation (no shell guards).

**Change 3:** Gemini hook commands simplified similarly.

**Change 4:** `print_hook_instructions()` updated to show the new pattern.

#### File: `src/commands/signal.rs`

**Change 5:** GUARD-1 now catches both `None` and empty string, logs to `.squad/log/signal.log`, and prints to stderr. Uses CWD-relative `.squad` path because this guard fires before config/DB resolution (provider hooks always set CWD to project root):
```rust
let agent: String = match agent {
    Some(name) if !name.is_empty() => name,
    _ => {
        let squad_dir = std::path::Path::new(".squad");
        if squad_dir.exists() {
            log_signal(
                std::path::Path::new("."),
                "GUARD",
                "(empty)",
                "reason=no_agent_name hook_env_resolution_failed",
            );
        }
        eprintln!("squad-station: signal: no agent name (tmux display-message failed or outside tmux)");
        return Ok(());
    }
};
```

**Why still `Ok(())`:** We must never fail the provider hook (exit code must be 0). Claude Code's hook contract expects exit 0. A non-zero exit code causes Claude Code to print a warning to the user's session, creating noise. Gemini CLI treats non-zero as hook failure and may retry or abort. The fix is observability (logging + stderr), not exit code changes.

#### File: `src/commands/init.rs` (tests)

**Change 6:** Updated test assertions:
- `test_install_claude_hooks_uses_tmux_display_message` — asserts `display-message`, asserts NO `SQUAD_AGENT_NAME`, NO `list-panes`, NO `TMUX_PANE`
- `test_install_gemini_hooks_uses_tmux_display_message` — same
- `tests/test_integration.rs:test_init_hook_prints_instructions` — updated to match new command format

### Alternatives Considered

| Alternative | Pros | Cons | Verdict |
|-------------|------|------|---------|
| **Hardcode agent name in hook** | Zero runtime resolution | settings.json is project-level, not per-agent | **Rejected** |
| **`$SQUAD_AGENT_NAME` with `display-message` fallback** | Supports CI | SQUAD_AGENT_NAME never available in hook context; adds complexity | **Rejected** (team feedback) |
| **`tmux display-message -p '#S'`** (chosen) | Works in all tmux contexts, simplest command | Fails outside tmux — but hooks only run inside tmux | **Chosen** |

---

## Problem 2: Watchdog Self-Healing

### Current State (before fix)

Four gaps:
1. **Daemon death** — spawned once at init (`init.rs:303`), stdout/stderr to `/dev/null` (`watch.rs:118-120`). If it crashes, nobody knows.
2. **Pass 3 observe-only** — detects busy >30min, logs warning, does nothing (`watch.rs:292-311`).
3. **No health check** — PID file may point to dead process for days.
4. **No session isolation** — daemon inherits parent's process group; closing the terminal that ran `init` sends SIGHUP to the watchdog, killing it.

### Implemented: Tiered Escalation in Pass 3

The key insight: reconcile already knows how to detect idle panes and complete stuck tasks (`reconcile.rs:100-145`). Pass 3 just needs to invoke it with escalating urgency.

**Thresholds (as implemented):**

| Duration | Action | Rationale |
|----------|--------|-----------|
| 0-10min | Skip | Normal operation. Agent is working. |
| 10-30min | Log only (Tier 1) | Long tasks (builds, large refactors) are normal. |
| 30min+ | **Reconcile check** (Tier 2) | Call `pane_looks_idle()` for this agent. If idle → log pane content snapshot (last 5 lines for diagnosing false positives), then auto-heal (complete tasks + notify orchestrator). If active → log and continue waiting. |
| 60min+ | **Notify orchestrator** (Tier 3) | Send `[SQUAD WATCHDOG] WARNING — Agent 'X' busy for 60m, may be stuck.` with 10min cooldown per agent. |
| 120min+ | **Urgent notify** (Tier 3) | Send `[SQUAD WATCHDOG] URGENT —` prefix instead of WARNING. Same 10min cooldown. |

**Why NOT auto force-complete:**
- A long `cargo build --release` can take 30+ minutes
- A complex refactoring task with many file changes can take 60+ minutes
- Force-completing would mark the task done when the agent is still mid-work
- The orchestrator would then send a NEW task to a "busy" agent, causing task collision
- Instead: reconcile check (is pane idle?) is safe — it only completes if the agent is ACTUALLY done

**Why NOT restart the agent:**
- Agent may have in-flight context (Claude Code conversation history, Gemini CLI session state)
- Restarting destroys this context
- The orchestrator is the right entity to decide whether to restart an agent

### Code Changes (Implemented)

#### File: `src/commands/watch.rs`

**Change 7:** New `BusyAlertState` struct with per-agent cooldown tracking (10min cooldown, `HashMap<String, DateTime>`).

**Change 8:** Pass 3 rewritten with 3-tier escalation. Tier 2 calls `reconcile::pane_looks_idle()` and auto-heals if pane is idle. Tier 3 sends notifications with cooldown throttle.

**Change 9:** Daemon stderr redirected to `.squad/log/watch-stderr.log` instead of `/dev/null`. Startup panics and DB errors are now captured.

**Change 10:** `setsid()` via `pre_exec` in daemon fork — creates a new session so SIGHUP from closing the terminal that ran `squad-station init` doesn't propagate to the watchdog daemon. This was included based on team feedback (previously deferred as BUG-10).

```rust
unsafe {
    cmd.pre_exec(|| {
        libc::setsid();
        Ok(())
    });
}
```

Note: `setsid()` return value is intentionally ignored — failure is benign (only fails if already a session leader, which can't happen for a freshly-forked child).

#### File: `src/commands/reconcile.rs`

**Change 11:** `pane_looks_idle()` changed from `fn` to `pub fn` so `watch.rs` can call it for Tier 2 reconcile checks.

#### File: `src/commands/helpers.rs`

**Change 12:** New `ensure_watchdog()` function — opportunistic PID health check. If PID file exists but process is dead, removes stale PID file and respawns the daemon with stderr redirected to log file. Logs respawn to `watch.log`. **Note:** Respawn uses hardcoded defaults (interval=30s, stall_threshold=5min) — custom values from original launch are not preserved. This is acceptable as best-effort recovery.

#### File: `src/commands/signal.rs`

**Change 13:** Calls `helpers::ensure_watchdog()` after every successful signal completion.

#### File: `src/commands/send.rs`

**Change 14:** Calls `helpers::ensure_watchdog()` after every successful message send.

---

## Summary of All Changes (Implemented)

| # | File | Change | Fixes |
|---|------|--------|-------|
| 1 | `init.rs:359-362` | `agent_resolve_snippet()` → `$(tmux display-message -p '#S' 2>/dev/null)` | BUG-01 |
| 2 | `init.rs:368-376` | Claude hook commands simplified (no shell guards, no env vars) | BUG-01 |
| 3 | `init.rs:424-431` | Gemini hook commands simplified similarly | BUG-01 |
| 4 | `init.rs:888-897` | Manual hook instructions updated | BUG-01 |
| 5 | `signal.rs:46-62` | GUARD-1: catches empty string, logs to signal.log + stderr | BUG-02 |
| 6 | `init.rs` (tests) | `test_install_claude_hooks_uses_tmux_display_message` (renamed + updated) | BUG-01 |
| 7 | `init.rs` (tests) | `test_install_gemini_hooks_uses_tmux_display_message` (renamed + updated) | BUG-01 |
| 8 | `test_integration.rs` | `test_init_hook_prints_instructions` updated assertion | BUG-01 |
| 9 | `watch.rs:5-43` | New `BusyAlertState` struct with per-agent cooldown | BUG-08 |
| 10 | `watch.rs:362-468` | Pass 3 rewritten with tiered escalation (10/30/60/120min) | BUG-08 |
| 11 | `watch.rs:118-145` | Daemon stderr → `watch-stderr.log` | BUG-07 |
| 12 | `watch.rs:173-179` | `setsid()` via `pre_exec` in daemon fork | BUG-10 |
| 13 | `reconcile.rs:153` | `pane_looks_idle()` made `pub` | (support for #10) |
| 14 | `helpers.rs:70-138` | New `ensure_watchdog()` health check + respawn | BUG-06 |
| 15 | `signal.rs:262` | Calls `helpers::ensure_watchdog()` | BUG-06 |
| 16 | `send.rs:92-96` | Calls `helpers::ensure_watchdog()` | BUG-06 |

### Tests (Implemented)

| Test | File | Status |
|------|------|--------|
| `test_install_claude_hooks_uses_tmux_display_message` | `init.rs` | Updated (renamed from `_uses_squad_agent_name`) |
| `test_install_gemini_hooks_uses_tmux_display_message` | `init.rs` | Updated (renamed from `_uses_squad_agent_name`) |
| `test_init_hook_prints_instructions` | `test_integration.rs` | Updated assertion |
| `test_guard1_logs_empty_agent_name` | `signal.rs` | New |
| `test_busy_alert_state_first_alert` | `watch.rs` | New |
| `test_busy_alert_state_respects_cooldown` | `watch.rs` | New |
| `test_busy_alert_state_per_agent` | `watch.rs` | New |
| `test_busy_alert_state_clear` | `watch.rs` | New |

**Total: 168 tests passing, 0 new clippy warnings.**

### Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `tmux display-message` fails outside tmux | GUARD-1 now logs the failure; hooks only run inside tmux |
| Pass 3 reconcile races with normal signal | Reconcile uses same `update_status()` with `WHERE status='processing'` — idempotent, no double-complete |
| Watchdog respawn creates infinite respawn loop if DB is corrupt | `ensure_watchdog()` only runs during successful signal/send — if DB is broken, signal/send fail first, respawn never triggered |
| `pane_looks_idle()` false positive (agent paused but not done) | Only triggers at 30min+. Pane content snapshot (last 5 lines) logged to `watch.log` for post-mortem diagnosis. False positive = task completed early, orchestrator gets notified. Low cost. |
| Tier 3 notification spam to orchestrator | `BusyAlertState` with 10min per-agent cooldown prevents repeated notifications |

### Not Addressed (Future Work)

- **BUG-04/05/17 (race conditions):** Transaction atomicity in send.rs/signal.rs. Separate effort, orthogonal to this fix.
- **BUG-09 (PID file race):** Low severity, rare in practice (two concurrent inits).
- **`launchd`/`systemd` integration:** Considered but rejected for now — adds platform-specific complexity. Opportunistic health check in signal/send is simpler and cross-platform.
