# Squad Station Playbook

A step-by-step guide to orchestrating AI agent squads with squad-station.

---

## Prerequisites

- **tmux** installed and available in PATH
- **squad-station** binary built and available in PATH (`cargo build --release`, installed at `~/.squad/bin/squad-station`)
- At least one AI coding tool: Claude Code (`claude`) or Gemini CLI (`gemini`)

---

## 1. Define Your Squad

Create a `squad.yml` in your project root.

### Standard CLI Orchestrator

```yaml
project: my-app

sdd:
  - name: get-shit-done
    playbook: "/path/to/GSD/Playbook.md"

orchestrator:
  provider: claude-code
  role: orchestrator
  model: opus
  description: "Lead orchestrator. Delegates tasks, synthesizes results."

agents:
  - name: frontend
    provider: claude-code
    role: worker
    model: sonnet
    description: "Frontend specialist"
  - name: backend
    provider: claude-code
    role: worker
    model: sonnet
    description: "Backend specialist"
```

### IDE Orchestrator (Antigravity)

```yaml
project: my-app

orchestrator:
  provider: antigravity
  role: orchestrator
  description: >
    Orchestrator running inside Antigravity IDE.
    Uses Manager View to poll and monitor tmux worker agents.

agents:
  - name: implement
    provider: claude-code
    role: worker
    model: sonnet
    description: "Implements features and fixes bugs"
```

**Agent naming convention:** The `name` field acts as a role suffix. The full registered agent name is `<project>-<name>`. For example: project `my-app`, name `frontend` → registered as `my-app-frontend`.

**Fields:**

| Field | Required | Description |
|-------|----------|-------------|
| `project` | Yes | Project identifier. Used as prefix in agent names. |
| `sdd` | No | Array of SDD workflow configs (name + playbook path). Injected into orchestrator context. |
| `orchestrator` | Yes | Exactly one orchestrator per squad |
| `agents` | Yes | Array of worker agents |
| `*.name` | Yes | Role suffix; full agent name is `<project>-<name>` (e.g., `my-app-frontend`) |
| `*.provider` | Yes | Known: `claude-code`, `gemini-cli`, `antigravity`. Unknown providers warn but proceed. |
| `*.role` | Yes | `orchestrator` or `worker` |
| `*.model` | No | Model identifier (e.g., `sonnet`, `opus`) — shown in context output |
| `*.description` | No | Human-readable description — shown in context output |

> **Note:** The DB is stored locally at `.squad/station.db` inside the project directory. Override with `SQUAD_STATION_DB` env var.

---

## 2. Launch the Squad

```bash
squad-station init
```

This will:
1. Create the SQLite database at `.squad/station.db`
2. Register all agents (names auto-prefixed as `<project>-<name>`)
3. Launch each agent in its own tmux session
4. Auto-install completion hooks (or print manual instructions)
5. Ask whether to enable context auto-inject (SessionStart hook)
6. Generate orchestrator slash command (`.claude/commands/squad-orchestrator.md` or `.gemini/commands/squad-orchestrator.toml`)
7. Create `<project>-monitor` tmux session with interactive tiled panes for all agents

**Check the result:**

```bash
squad-station init --json
# {
#   "launched": 3,
#   "skipped": 0,
#   "failed": [],
#   "db_path": ".squad/station.db"
# }
```

Re-running `init` is safe — already-running agents are skipped.

**Antigravity note:** When orchestrator provider is `antigravity`, `init` registers the orchestrator in the DB only (no tmux session is created for it). Worker agents still get tmux sessions normally.

---

## 3. Hooks — Completion & Notification

Hooks let squad-station know when an agent finishes work or needs input. Without hooks, you must signal manually.

### Automatic Setup

`squad-station init` automatically installs all hooks:
- If a `settings.json` already exists, init merges hook entries and creates a `.bak` backup
- If no `settings.json` exists, init prints the hook configuration to stdout for manual setup

All hooks use the same inline command pattern — no external shell scripts needed:
- **Signal:** `squad-station signal $(tmux display-message -p '#S')`
- **Notify:** `squad-station notify --body 'Agent needs input' --agent $(tmux display-message -p '#S')`

### Claude Code Hooks (4 events + optional SessionStart)

Add to `.claude/settings.json` (project-level) or `~/.claude/settings.json` (global):

| Event | Matcher | Fires When |
|-------|---------|------------|
| `Stop` | `*` | Agent finishes turn → `signal` marks task completed |
| `Notification` | `permission_prompt` | Agent blocked by permission dialog → `notify` orchestrator |
| `Notification` | `elicitation_dialog` | Agent blocked by MCP input form → `notify` orchestrator |
| `PostToolUse` | `AskUserQuestion` | Agent asks a clarifying question → `notify` orchestrator |
| `SessionStart` | `*` | Session start/compact → `context --inject` (opt-in) |

### Gemini CLI Hooks (2 events + optional SessionStart)

Add to `.gemini/settings.json`:

| Event | Matcher | Fires When |
|-------|---------|------------|
| `AfterAgent` | `*` | Agent finishes turn → `signal` marks task completed |
| `Notification` | `*` | Any notification (permissions, alerts) → `notify` orchestrator |
| `SessionStart` | `*` | Session start/resume/clear → `context --inject` (opt-in) |

### SessionStart — Context Auto-Inject

The `SessionStart` hook is **opt-in**. During `squad-station init`, you are asked:

```
Enable auto-inject? [y/N]
```

When enabled, the orchestrator automatically receives its role, agent roster, and playbook context whenever the AI starts a new session, resumes, or compacts context. When disabled, you must manually run `/squad-orchestrator` each time.

The `--inject` command guards on the orchestrator session name — workers sharing the same settings file receive no injection.

### Notes

- The hook command auto-detects the agent from the tmux session name
- The command always exits 0 and never blocks the AI tool, even on errors
- `signal` has a 4-layer guard: not in tmux → exit, not registered → exit, is orchestrator → exit, no pending task → exit
- Legacy shell scripts in `hooks/` are kept for reference but are no longer required

---

## 4. Watchdog — Self-Healing & Monitoring

The watchdog daemon monitors agent health and automatically recovers from stuck states.

### Starting the watchdog

```bash
# Start as background daemon (default: 30s interval, 5min stall threshold)
squad-station watch --daemon

# Custom settings
squad-station watch --daemon --interval 60 --stall-threshold 10

# Stop the daemon
squad-station watch --stop
```

The watchdog is also started automatically by `squad-station init`.

### Tiered busy detection

When an agent stays in "busy" status, the watchdog escalates through 4 tiers:

| Duration | Tier | Action |
|----------|------|--------|
| 0–10 min | — | Normal operation, no action |
| 10–30 min | **Tier 1: Log only** | Logs warning to `watch.log`. Long tasks (builds, refactors) are normal. |
| 30+ min | **Tier 2: Auto-heal** | Checks if the agent's tmux pane is actually idle (prompt visible). If idle → signal was lost, so watchdog completes stuck tasks, resets agent to idle, and notifies the orchestrator. Logs pane content snapshot for false-positive diagnosis. |
| 60+ min | **Tier 3: Alert** | Pane is active but agent has been busy too long. Sends `[SQUAD WATCHDOG] WARNING` to orchestrator (10 min cooldown per agent). |
| 120+ min | **Tier 3: Urgent** | Escalates to `[SQUAD WATCHDOG] URGENT` prefix. Same cooldown. |

### Watchdog resilience

- **SIGHUP immunity:** Daemon runs in its own session (`setsid`) — closing the terminal that ran `init` won't kill it
- **Stderr capture:** Daemon stderr goes to `.squad/log/watch-stderr.log` (not `/dev/null`)
- **Self-healing:** `signal` and `send` commands opportunistically check if the watchdog is alive and respawn it if dead
- **Global stall detection:** If all agents are idle with no pending tasks for longer than `--stall-threshold`, the watchdog nudges the orchestrator (up to 3 times with 10 min cooldown)

### Log files

| File | Contents |
|------|----------|
| `.squad/log/watch.log` | Watchdog tick events, reconciliation, heal actions, alerts |
| `.squad/log/watch-stderr.log` | Daemon panics, DB errors, startup failures |
| `.squad/log/signal.log` | Signal hook events, guard exits, completion records |

---

## 5. Send Tasks to Agents

```bash
# Basic task
squad-station send my-app-frontend --body "Build the login page with email/password fields"

# With priority
squad-station send my-app-backend --body "Fix the auth endpoint" --priority urgent

# With thread grouping (links related messages)
squad-station send my-app-frontend --body "Now add form validation" --thread <thread-id>

# JSON output (for scripting)
squad-station send my-app-frontend --body "Add form validation" --priority high --json
```

**Priority levels:** `normal` (default), `high`, `urgent`

What happens behind the scenes:
1. Task is stored in the database (status: processing)
2. Agent is marked as "busy"
3. Task text is injected into the agent's tmux session via safe `load-buffer`/`paste-buffer`

---

## 6. Mid-Task Notifications

When an agent needs input from the orchestrator without completing its task:

```bash
# Auto-detect agent from tmux session
squad-station notify --body "Need confirmation: use JWT or sessions?"

# Explicit agent name
squad-station notify --body "Need design review" --agent my-app-frontend
```

The orchestrator receives: `[SQUAD INPUT NEEDED] Agent 'my-app-frontend': Need confirmation: use JWT or sessions?`

**Key difference from `signal`:**
- `notify` — task in progress, agent needs input, no status change
- `signal` — task done, marks completed, resets agent to idle

---

## 7. Monitor Your Squad

### Quick status overview

```bash
squad-station status
# Project: my-app
# DB: .squad/station.db
# Agents: 3 -- 2 idle, 1 busy, 0 dead
#
#   my-app-orchestrator: idle 5m   |  0 pending
#   my-app-frontend: busy 2m      |  1 pending
#   my-app-backend: idle 10m      |  0 pending
```

### Agent list with live tmux reconciliation

```bash
squad-station agents
# NAME              ROLE          STATUS            TOOL
# my-app-orch       orchestrator  idle 5m           claude-code
# my-app-frontend   worker        busy 2m           claude-code
# my-app-backend    worker        idle 10m          claude-code
```

The `agents` command checks tmux to detect crashed sessions:
- Session gone → agent marked **dead**
- Session reappears → agent revived to **idle**

### Message log

```bash
# All messages
squad-station list

# Filter by agent
squad-station list --agent my-app-frontend

# Filter by status
squad-station list --status processing

# Limit results
squad-station list --limit 5

# JSON output
squad-station list --agent my-app-backend --status completed --json
```

### Status overview

```bash
squad-station status           # Project + agent summary
squad-station agents           # Roster with live status (idle/busy/dead)
squad-station list --agent my-app-backend --limit 10   # Recent messages
```

### Attach directly to an agent session

```bash
tmux attach -t my-app-implement
```

Switch between sessions with `Ctrl-b s` (tmux session switcher).

---

## 8. Check Pending Work

```bash
# What's the next task for an agent?
squad-station peek my-app-frontend
# [processing] (priority=high) Add form validation
# id: a1b2c3d4-...

# No pending work
squad-station peek my-app-backend
# No pending tasks for my-app-backend
```

Peek returns the highest-priority task first (urgent > high > normal), with oldest-first tie-breaking.

---

## 9. Signal Completion

If hooks are set up, this happens automatically. For manual signaling:

```bash
squad-station signal my-app-frontend
# ✓ Signaled completion for my-app-frontend (task_id=a1b2c3d4-...)
```

What happens:
1. Most recent processing message is marked "completed"
2. Agent status resets to "idle"
3. Orchestrator receives a structured notification:

```
[SQUAD SIGNAL] Agent 'my-app-frontend' completed task a1b2c3d4-.... Read output: tmux capture-pane -t my-app-frontend -p | Next: squad-station status
```

Duplicate signals are safe — they silently succeed.

---

## 10. Cleanup Commands

### Clean (kill sessions + delete DB)

```bash
# With confirmation prompt
squad-station clean

# Skip confirmation
squad-station clean -y
```

Kills all squad tmux sessions (agents + monitor) and deletes the database. Shows killed/skipped counts and DB deletion status.

### Uninstall (remove hooks, files, sessions)

```bash
squad-station uninstall        # Prompts for confirmation
squad-station uninstall -y     # Skip confirmation
```

Removes hook entries from `settings.json`, deletes `.squad/`, kills tmux sessions, and removes the generated context files.

---

## 11. Antigravity IDE Orchestrator Mode

### When to use

Use `provider: antigravity` when you want to run the orchestrator inside an IDE (Antigravity, Cursor, VS Code agent, etc.) rather than as a CLI tmux session.

### What changes with Antigravity

- `init` registers the orchestrator in the DB only — no tmux session is created for it
- `signal` updates the message status in the DB but does NOT inject a tmux notification
- The IDE polls completion by calling `squad-station status` or `squad-station list --status completed`
- Worker agents still run as tmux sessions and receive tasks via the normal send path

### IDE workflow

1. Run `squad-station init` — registers orchestrator in DB, launches worker tmux sessions
2. Run `squad-station context` — generates `.claude/commands/squad-orchestrator.md` (or provider-specific file)
3. IDE orchestrator reads the generated playbook
4. IDE orchestrator calls `squad-station send <agent> --body "..."` to dispatch tasks
5. IDE orchestrator polls `squad-station status` to detect task completion

---

## 12. Generate Orchestrator Context

```bash
# Generate slash command file (normal mode)
squad-station context

# Output to stdout for SessionStart hook (inject mode)
squad-station context --inject
```

Generates the orchestrator slash command with a provider-specific format:

| Provider | Generated path |
|----------|----------------|
| `claude-code` | `.claude/commands/squad-orchestrator.md` |
| `gemini-cli` | `.gemini/commands/squad-orchestrator.toml` |
| Other | `.claude/commands/squad-orchestrator.md` (fallback) |

The generated content includes:
- PRE-FLIGHT checklist (read playbooks, verify agents)
- Completion notification protocol (signal-based wait, anti-polling)
- Context management guidance (when to /clear agents)
- Session routing rules (which agent for which task type)
- SDD orchestration (if configured in squad.yml)
- Sending task templates
- Full context transfer instructions
- Workflow completion discipline
- QA Gate (6-step checklist)
- Agent roster table

The `--inject` flag outputs the content to stdout for hook consumption instead of writing to a file. It guards on the orchestrator session name — workers receive no output.

---

## 13. Update Squad at Runtime

Add or change agents without a full teardown:

```bash
squad-station update           # Re-apply squad.yml: launch new agents, restart changed, skip running
```

This is the safe way to add a new worker or change a model after `init` — running agents are not interrupted.

---

## Workflow Summary

```
┌─────────────────────────────────────────────────────────────┐
│                        Orchestrator                          │
│                                                             │
│  1. Reads context (squad-orchestrator.md)                   │
│  2. Sends tasks (squad-station send agent --body "task")    │
│  3. Receives signals:                                       │
│     [SQUAD SIGNAL] Agent 'X' completed task <id>...         │
│  4. Reads output (tmux capture-pane -t agent -p)            │
│  5. Sends next task or reports to user                      │
└──────────┬──────────────────────────┬───────────────────────┘
           │                          │
     ┌─────▼─────┐            ┌──────▼──────┐
     │  Worker A  │            │  Worker B   │
     │            │            │             │
     │ Receives   │            │ Receives    │
     │ task via   │            │ task via    │
     │ tmux       │            │ tmux        │
     │            │            │             │
     │ Completes  │            │ Completes   │
     │ work       │            │ work        │
     │            │            │             │
     │ Hook fires │            │ Hook fires  │
     │ signal cmd │            │ signal cmd  │
     └────────────┘            └─────────────┘
```

---

## Command Reference

| Command | Purpose | Key Flags |
|---------|---------|-----------|
| `init [config]` | Launch squad from config | `--json` |
| `send <agent> --body <task>` | Send task to agent | `--body`, `--priority`, `--thread`, `--json` |
| `signal [agent]` | Signal task completion | `--json` |
| `notify --body <msg>` | Mid-task notification to orchestrator | `--body`, `--agent`, `--json` |
| `peek <agent>` | View next pending task | `--json` |
| `list` | List messages | `--agent`, `--status`, `--limit`, `--json` |
| `agents` | List agents with status | `--json` |
| `status` | Project overview | `--json` |
| `context` | Generate orchestrator context | `--inject` |
| `reconcile [config]` | Fix stuck agents | `--dry-run`, `--json` |
| `update [config]` | Re-apply squad.yml at runtime | `--json` |
| `watch` | Watchdog health monitor | `--daemon`, `--stop`, `--interval` |
| `doctor` | Health check (6 checks) | `--json` |
| `uninstall [config]` | Remove hooks, files, sessions | `-y`/`--yes` |
| `clean [config]` | Kill all sessions + delete DB | `-y`/`--yes`, `--all` |
| `freeze` / `unfreeze` | Block or allow orchestrator dispatch | `--json` |

All commands support `--json` for machine-readable output and `--help` for usage details.

---

## Troubleshooting

**Agent shows "dead" status**
The tmux session crashed or was closed. Re-run `squad-station init` to relaunch, or manually start a new tmux session with the agent's name.

**"Agent not found" when sending**
The agent name doesn't match any registered agent. Check `squad-station agents` for the exact names. Remember: full agent names are `<project>-<name>`.

**"tmux session not running" when sending**
The agent is registered but its tmux session is down. Re-run `squad-station init` or launch the session manually.

**Hook not firing**
Verify `squad-station` is in PATH: `which squad-station`. Verify the agent is running inside a tmux session (hooks use `tmux display-message -p '#S'` to resolve the agent name). Check that `settings.json` has the correct hook configuration. Check `.squad/log/signal.log` for GUARD entries that show why the signal was skipped.

**Database locked errors**
Squad-station uses single-writer SQLite with WAL mode. If you see lock errors, ensure only one write operation runs at a time. The 5-second busy timeout handles most concurrent cases.

**Antigravity: orchestrator not receiving completion signals**
This is expected behavior. With `provider: antigravity`, the orchestrator has no tmux session, so `signal` does not inject a notification. Use `squad-station status` or `squad-station list --status completed` to poll for task completion instead.

**Agent stuck in "busy" for hours**
The watchdog should auto-heal this. Check if the watchdog is running: look for `.squad/watch.pid`. If the PID file is missing or stale, the next `signal` or `send` command will respawn it. You can also manually run `squad-station watch --daemon`. Check `.squad/log/watch.log` for HEAL entries.

**Full reset when things go wrong**
Run `squad-station clean -y` to kill all sessions and delete the database, then `squad-station init` to start fresh.
