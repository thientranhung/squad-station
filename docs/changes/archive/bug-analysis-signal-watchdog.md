# Bug Analysis: Signal Hook Failure & Watchdog Design Gaps

**Date:** 2026-03-22
**Triggered by:** CompetitorIQ deployment — implement agent completed Phase 4 but never signaled back (message 47805060 stuck in `processing` for 13+ hours)

---

## Table of Contents

1. [Root Cause Analysis: Signal Hook Failure](#1-root-cause-analysis-signal-hook-failure)
2. [Watchdog Design Gaps](#2-watchdog-design-gaps)
3. [Newly Discovered Bugs](#3-newly-discovered-bugs)
4. [Summary Table](#4-summary-table)
5. [Recommended Fix Order](#5-recommended-fix-order)

---

## 1. Root Cause Analysis: Signal Hook Failure

### BUG-01: Hook Environment Variable Resolution Fragility
**Severity:** CRITICAL
**File:** `src/commands/init.rs:359-376`, `src/commands/signal.rs:46-53`

**Root Cause:** The Stop hook relies on a two-stage agent name resolution:
```bash
AGENT=${SQUAD_AGENT_NAME:-$(tmux list-panes -t "$TMUX_PANE" -F '#S' 2>/dev/null | head -1)}
```

If **both** `SQUAD_AGENT_NAME` and `TMUX_PANE` are unavailable in the hook's execution context (which varies by provider CLI version and invocation method), the agent name resolves to empty string. Signal.rs GUARD-1 (`signal.rs:46-53`) then exits silently with code 0 — no error, no log entry, task stays `processing` forever.

**Why Phase 1-3 worked but Phase 4 didn't:** Intermittent — depends on provider CLI's internal state when the Stop event fires. The hook environment is not guaranteed identical across invocations. A provider CLI restart, memory pressure, or internal timeout can change whether `TMUX_PANE` is inherited.

**Evidence:** `.squad/log/signal.log` has NO entry for task 47805060, confirming the hook either didn't fire or fired with empty agent name (GUARD-1 exits before logging).

**Actual Fix (implemented):**
1. Switched hook to use `tmux display-message -p '#S'` — a tmux server-side query that works reliably in all hook contexts (no env vars needed)
2. Simplified hook command: `squad-station signal "$(tmux display-message -p '#S' 2>/dev/null)" 2>>.squad/log/signal.log` — removed `$SQUAD_AGENT_NAME`, `$TMUX_PANE`, intermediate variables, and shell guards
3. GUARD-1 now logs empty agent name to `.squad/log/signal.log` + stderr before exiting

---

### BUG-02: Signal Exits 0 on All Failure Paths (Silent Failures)
**Severity:** CRITICAL
**File:** `src/commands/signal.rs:46-53, 80-87, 92-111`

**Root Cause:** All guard clauses in signal.rs return `Ok(())` (exit code 0), making it impossible for the calling hook or provider to detect failure:

| Guard | Location | Condition | Exit |
|-------|----------|-----------|------|
| GUARD-1 | `signal.rs:46-53` | Empty agent name | `Ok(())` — silent |
| GUARD-2 | `signal.rs:80-87` | DB connection failed | `Ok(())` — silent |
| GUARD-3 | `signal.rs:92-111` | Agent not found in DB | `Ok(())` — silent |
| GUARD-4 | `signal.rs:113-123` | Agent is orchestrator | `Ok(())` — by design |

GUARD-1 and GUARD-2 are the most dangerous because they exit **before** writing to signal.log, leaving zero forensic evidence.

**Actual Fix (implemented):**
1. GUARD-1 now logs to signal.log AND prints to stderr before exiting (kept exit 0 — provider hook contract requires it)
2. Exit code remains 0 for all guards — Claude Code and Gemini CLI treat non-zero as errors causing noise/retry. Observability fix (logging + stderr), not exit code changes.
3. GUARD-2 and GUARD-3 already logged before this fix; GUARD-1 was the gap

---

### BUG-03: DB Connection Timeout Causes Silent Signal Drop
**Severity:** HIGH
**File:** `src/commands/signal.rs:80-87`, `src/db/mod.rs:12-27`

**Root Cause:** Signal creates a new SQLite pool on every invocation. If the TUI (`ui.rs`) or another writer holds the single-writer lock for >5 seconds (the `busy_timeout`), signal's `db::connect()` times out. GUARD-2 catches this and exits silently.

**Proposed Fix:**
1. Increase `busy_timeout` to 15s for signal operations (they're hook-driven and must succeed)
2. Add retry with backoff (3 attempts, 2s/4s/8s) before giving up
3. Ensure TUI drops pool after each fetch cycle (documented design but needs enforcement)

---

### BUG-04: send.rs Lacks Transaction Atomicity (current_task Race)
**Severity:** HIGH
**File:** `src/commands/send.rs:56-86`

**Root Cause:** The send command performs three sequential SQL operations without wrapping them in a transaction:
1. `insert_message()` (line 56)
2. Agent status update (line 72-79)
3. `set_current_task()` (line 84)

If signal fires between steps 1 and 3, `current_task` is still NULL. Signal falls through to FIFO fallback (`signal.rs:153-177`), which completes the **oldest** processing message by priority — potentially the wrong task.

**Proposed Fix:** Wrap the three operations in a single SQLite transaction.

---

### BUG-05: current_task/Signal Race — Concurrent Send Overwrites Pointer
**Severity:** HIGH
**File:** `src/commands/signal.rs:247-251`, `src/commands/send.rs:84`

**Root Cause:** After signal completes a task, it fetches the next pending task and calls `set_current_task()` (signal.rs:250). Concurrently, a new `send` command may also call `set_current_task()` (send.rs:84). Last writer wins — one task becomes invisible to the signal mechanism and can only be completed via FIFO fallback.

**Proposed Fix:** Use `UPDATE agents SET current_task = ? WHERE current_task IS NULL` (conditional set) to prevent overwrites, or wrap in transaction with conflict detection.

---

## 2. Watchdog Design Gaps

### BUG-06: Watchdog Daemon Dies Silently — No Respawn
**Severity:** CRITICAL
**File:** `src/commands/watch.rs:104-131`, `src/commands/init.rs:303`

**Root Cause:** The watchdog is spawned once during `init` via `Command::new(exe).spawn()`. If it crashes (OOM, panic, DB corruption), it stays dead permanently. No health check, no respawn, no supervisor. The PID file (`.squad/watch.pid`) points to a dead process.

**Evidence:** PID 79346 no longer exists. `.squad/log/watch.log` does NOT exist (daemon died before first tick or log directory was unwritable).

**Actual Fix (implemented):**
1. New `ensure_watchdog()` helper in `helpers.rs` — checks PID liveness, respawns dead daemon with stderr to log file
2. Called opportunistically from `signal.rs` and `send.rs` after every successful operation
3. Logs respawn events to `watch.log`
4. **Limitation:** Respawn uses hardcoded defaults (interval=30s, stall_threshold=5min); custom launch values are not preserved. Acceptable as best-effort recovery.

---

### BUG-07: Watchdog stdout/stderr Redirected to /dev/null
**Severity:** HIGH
**File:** `src/commands/watch.rs:118-120`

**Root Cause:**
```rust
cmd.stdin(std::process::Stdio::null())
   .stdout(std::process::Stdio::null())
   .stderr(std::process::Stdio::null());
```

All output is lost. If the daemon panics on startup (e.g., DB file locked, migration failure), the error message vanishes. Combined with BUG-06 (no respawn), this creates a completely silent failure mode.

**Evidence:** No `.squad/log/watch.log` exists — the daemon likely crashed before its first tick could create the log file.

**Actual Fix (implemented):**
1. Daemon stderr redirected to `.squad/log/watch-stderr.log` instead of `/dev/null`
2. `ensure_watchdog()` respawner also redirects stderr to the same log file
3. Startup confirmation log already existed at line 142 (`log_watch("INFO", "watchdog started ...")`)

---

### BUG-08: Pass 3 (Prolonged Busy Detection) Is Observe-Only
**Severity:** HIGH
**File:** `src/commands/watch.rs:292-311`

**Root Cause:** Pass 3 detects agents stuck in `busy` status for >30 minutes but only logs a warning. It takes **no corrective action** — doesn't reconcile, doesn't signal, doesn't notify the orchestrator, doesn't update agent status.

This is exactly the scenario that occurred: implement agent was busy for 13+ hours and the watchdog (if alive) would have only logged a warning.

**Actual Fix (implemented):** Tiered escalation with 4 levels:
1. 10-30min: Log only (Tier 1)
2. 30min+: Reconcile check — if `pane_looks_idle()`, log pane content snapshot (last 5 lines) for false-positive diagnosis, then auto-heal (complete tasks + notify orchestrator). Breadcrumb comment injected into pane (safe because idle detection confirmed shell prompt). (Tier 2)
3. 60min+: Notify orchestrator with WARNING (10min cooldown per agent via `BusyAlertState`) (Tier 3)
4. 120min+: Notify orchestrator with URGENT prefix (same cooldown) (Tier 3)
- No auto force-complete — too dangerous (agent might be doing a long build)

---

### BUG-09: PID File Race Condition (Double Daemon)
**Severity:** LOW
**File:** `src/commands/watch.rs:84-102`

**Root Cause:** Check-then-act pattern on PID file without atomic locking. Two concurrent `init` calls can both pass the "PID file exists?" check and spawn duplicate watchdog daemons. Second PID overwrites first in the file; `--stop` only kills one.

**Proposed Fix:** Use `flock()` or `O_EXCL` for atomic PID file creation.

---

### BUG-10: No Daemonization Session Isolation (setsid)
**Severity:** MEDIUM
**File:** `src/commands/watch.rs:108-121`

**Root Cause:** The watchdog child process doesn't call `setsid()` to create a new session. It inherits the parent's process group, meaning:
- If the parent shell receives SIGHUP (terminal closes), the watchdog may receive it too
- File descriptor inheritance from parent could cause unexpected behavior

**Actual Fix (implemented):** Added `pre_exec` with `libc::setsid()` in daemon fork. Watchdog now survives terminal closure. `setsid()` return value intentionally ignored — failure is benign (only fails if already a session leader, which can't happen for a freshly-forked child).

---

## 3. Newly Discovered Bugs

### BUG-11: Agent Name Collision After Sanitization
**Severity:** MEDIUM
**File:** `src/commands/init.rs:24, 70`

Two agents with names that sanitize to the same string (e.g., `project:worker` and `project-worker` both become `project-worker`) will silently share the same tmux session. Second launch overwrites first. No validation or error.

**Proposed Fix:** Validate uniqueness of sanitized names during init, before launching sessions.

---

### BUG-12: Fire-and-Forget Detection Over-Matches
**Severity:** LOW
**File:** `src/commands/send.rs:121-125`

`is_fire_and_forget()` uses `starts_with("/clear")` after lowercasing. This matches unintended commands like `/clearContext` or `/clearHistory`, auto-completing them without sending to the agent.

**Proposed Fix:** Match exact `/clear` or `/clear ` (with trailing space/end-of-string).

---

### BUG-13: Reconcile Sends Duplicate Nudges on Crash-Recovery
**Severity:** LOW
**File:** `src/commands/watch.rs:265`, `src/commands/reconcile.rs:123`

Pass 2 nudges (tmux send-keys to orchestrator) are not idempotent. If the watchdog crashes mid-tick and restarts, the same nudge may be sent twice. The orchestrator receives duplicate notifications.

**Proposed Fix:** Track last-nudged timestamp per agent in NudgeState; skip if within cooldown.

---

### BUG-14: Tmux inject_body Partial Delivery (Text Without Enter)
**Severity:** MEDIUM
**File:** `src/tmux.rs:128-147`

`send_keys_literal()` sends text and Enter as two separate tmux commands with a 2-second gap. If the session dies between them, text is pasted but never executed. No retry, no detection.

**Proposed Fix:** Verify pane content after Enter to confirm execution, or use a single atomic tmux command.

---

### BUG-15: Temp File Leak on Injection Failure
**Severity:** LOW
**File:** `src/tmux.rs:163, 169`

`inject_single()` writes to `/tmp/squad-station-msg-*` and uses `let _ = std::fs::remove_file()` for cleanup. If removal fails (permissions, disk full), temp files accumulate silently.

**Proposed Fix:** Log cleanup failures; add periodic temp file cleanup in watchdog.

---

### BUG-16: No Priority Validation on Message Insert
**Severity:** LOW
**File:** `src/db/messages.rs`

No validation that priority is one of `{urgent, high, normal}`. Unknown priority values silently default to lowest priority (ELSE 3 in SQL CASE). A typo like `--priority hgh` creates a message that's always processed last.

**Proposed Fix:** Validate priority enum at insert time in `insert_message()`.

---

### BUG-17: Agent Status Can Diverge from Message State
**Severity:** MEDIUM
**File:** `src/commands/signal.rs:244-258`

After completing a task, signal counts remaining messages (line 245) then updates agent status (line 256). A concurrent `send` between these two lines can insert a new task, but signal still marks agent as `idle` based on the stale count. Agent is idle in DB but has a processing message.

**Proposed Fix:** Wrap the count + status update in a single transaction, or re-check after status update.

---

## 4. Summary Table

| ID | Bug | Severity | Component | Type | Status |
|----|-----|----------|-----------|------|--------|
| BUG-01 | Hook env var resolution fragility | **CRITICAL** | signal hook | Silent failure | **FIXED** — switched to `tmux display-message -p '#S'` |
| BUG-02 | Signal exits 0 on all failure paths | **CRITICAL** | signal.rs | Observability | **FIXED** — GUARD-1 now logs empty agent name to signal.log + stderr |
| BUG-03 | DB connection timeout drops signal | **HIGH** | signal.rs / db | Silent failure | Open |
| BUG-04 | send.rs lacks transaction atomicity | **HIGH** | send.rs | Race condition | Open |
| BUG-05 | current_task race between send/signal | **HIGH** | signal.rs / send.rs | Race condition | Open |
| BUG-06 | Watchdog dies silently, no respawn | **CRITICAL** | watch.rs / init.rs | Reliability | **FIXED** — `ensure_watchdog()` in signal.rs + send.rs |
| BUG-07 | Watchdog stderr sent to /dev/null | **HIGH** | watch.rs | Observability | **FIXED** — stderr → `watch-stderr.log` |
| BUG-08 | Pass 3 observe-only, no corrective action | **HIGH** | watch.rs | Design gap | **FIXED** — tiered escalation (10/30/60/120min) |
| BUG-09 | PID file race condition | **LOW** | watch.rs | Race condition | Open |
| BUG-10 | No setsid() in daemon fork | **MEDIUM** | watch.rs | Process mgmt | **FIXED** — `pre_exec` + `setsid()` |
| BUG-11 | Agent name collision after sanitization | **MEDIUM** | init.rs | Validation | Open |
| BUG-12 | Fire-and-forget over-matches /clear* | **LOW** | send.rs | Edge case | Open |
| BUG-13 | Duplicate nudges on crash-recovery | **LOW** | watch.rs / reconcile | Idempotency | Open |
| BUG-14 | Partial tmux delivery (text without Enter) | **MEDIUM** | tmux.rs | Reliability | Open |
| BUG-15 | Temp file leak on injection failure | **LOW** | tmux.rs | Resource leak | Open |
| BUG-16 | No priority validation on insert | **LOW** | messages.rs | Validation | Open |
| BUG-17 | Agent status diverges from message state | **MEDIUM** | signal.rs | Race condition | Open |

**Totals:** 3 CRITICAL, 4 HIGH, 4 MEDIUM, 6 LOW
**Fixed:** 6 (BUG-01, BUG-02, BUG-06, BUG-07, BUG-08, BUG-10) — **Open:** 11

---

## 5. Recommended Fix Order

### Wave 1 — Stop the Bleeding (DONE)
1. **BUG-01** — ~~Hardcode agent names in hooks at init time~~ Switched to `tmux display-message` **FIXED**
2. **BUG-02** — ~~Make signal.rs log ALL guard exits and return non-zero on failure~~ GUARD-1 now logs + stderr (kept exit 0) **FIXED**
3. **BUG-06** — Add watchdog health check + opportunistic respawn in send/signal **FIXED**

### Wave 2 — Prevent Recurrence (DONE)
4. **BUG-08** — Add corrective actions to Pass 3 (reconcile → notify escalation, 10/30/60min thresholds) **FIXED**
5. **BUG-07** — Redirect watchdog stderr to log file **FIXED**
6. **BUG-10** — Add setsid() to daemon fork **FIXED** (pulled forward from Wave 4)
7. **BUG-03** — Add retry with backoff for DB connection in signal.rs — **Open**

### Wave 3 — Eliminate Race Conditions (Open)
8. **BUG-04** — Wrap send.rs operations in transaction
9. **BUG-05** — Use conditional set for current_task updates
10. **BUG-17** — Wrap signal's count + status update in transaction

### Wave 4 — Hardening (Open)
11. **BUG-11** — Validate sanitized name uniqueness in init
12. **BUG-14** — Add delivery verification for tmux inject
13. Remaining LOW severity bugs (BUG-09, BUG-12, BUG-13, BUG-15, BUG-16)
