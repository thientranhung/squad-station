# Squad Station Playbook

A step-by-step guide to orchestrating AI agent squads with squad-station.

---

## Prerequisites

- **Rust toolchain** (for building from source)
- **tmux** installed and available in PATH
- At least one AI coding tool: Claude Code (`claude`) or Gemini CLI (`gemini`)

### Build & Install

```bash
cargo build --release
# Binary at: target/release/squad-station
# Add to PATH or use absolute path
```

---

## 1. Define Your Squad

Create a `squad.yml` in your project root:

```yaml
project: my-app

orchestrator:
  tool: claude-code
  role: orchestrator
  model: claude-opus-4-5           # optional
  description: "Lead orchestrator" # optional

agents:
  - name: frontend                  # role suffix; full name = my-app-claude-code-frontend
    tool: claude-code
    role: worker
    model: claude-sonnet-4-5        # optional
    description: "Frontend specialist"
  - name: backend
    tool: gemini
    role: worker
```

**Agent naming convention:** The `name` field acts as a role suffix. The full registered agent name is automatically prefixed as `<project>-<tool>-<role_suffix>`. For example: project `my-app`, tool `claude-code`, name `frontend` → registered as `my-app-claude-code-frontend`.

**Fields:**

| Field | Required | Description |
|-------|----------|-------------|
| `project` | Yes | Project identifier (plain string). Used in DB path and as prefix in agent names. |
| `orchestrator` | Yes | Exactly one orchestrator per squad |
| `agents` | Yes | Array of worker agents (can be empty) |
| `*.name` | Yes | Acts as role suffix; full agent name is auto-prefixed as `<project>-<tool>-<role_suffix>` (e.g., `my-app-claude-code-frontend`) |
| `*.tool` | Yes | Label: `claude-code`, `gemini`, or any string |
| `*.role` | Yes | `orchestrator` or `worker` |
| `*.model` | No | Model identifier (e.g., `claude-sonnet-4-5`) — shown in context output |
| `*.description` | No | Human-readable description — shown in context output |

> **Note:** The DB path is controlled by the `SQUAD_STATION_DB` environment variable only — it is not set in `squad.yml`.

---

## 2. Launch the Squad

```bash
squad-station init
```

This will:
1. Create the SQLite database
2. Register all agents (names auto-prefixed as `<project>-<tool>-<role_suffix>`)
3. Launch each agent in its own tmux session

**Check the result:**

```bash
squad-station init --json
# {
#   "launched": 3,
#   "skipped": 0,
#   "failed": [],
#   "db_path": "/Users/you/.agentic-squad/my-app/station.db"
# }
```

Re-running `init` is safe — already-running agents are skipped.

---

## 3. Set Up Completion Hooks

Hooks let squad-station know when an agent finishes its work. Without hooks, you must signal manually.

### Claude Code

Add to your `.claude/settings.json` (project-level) or `~/.claude/settings.json` (global):

```json
{
  "hooks": {
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/your-project/hooks/claude-code-notify.sh"
          }
        ]
      }
    ]
  }
}
```

### Gemini CLI

Add to your `.gemini/settings.json`:

```json
{
  "hooks": {
    "AfterAgent": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/your-project/hooks/gemini-cli-notify.sh"
          }
        ]
      }
    ]
  }
}
```

Both hooks always exit 0 — they never break the tool, even on errors.

---

## 4. Send Tasks to Agents

Task body is a required named flag (`--body`), not a positional argument.

```bash
# Basic task
squad-station send my-app-claude-code-frontend --body "Build the login page with email/password fields"

# With priority
squad-station send my-app-gemini-backend --body "Fix the auth endpoint" --priority urgent

# JSON output (for scripting)
squad-station send my-app-claude-code-frontend --body "Add form validation" --priority high --json
# {
#   "sent": true,
#   "message_id": "8c2e9e2f-...",
#   "agent": "my-app-claude-code-frontend",
#   "priority": "high"
# }
```

**Priority levels:** `normal` (default), `high`, `urgent`

What happens behind the scenes:
1. Task is stored in the database (status: pending)
2. Agent is marked as "busy"
3. Task text is injected into the agent's tmux session

---

## 5. Monitor Your Squad

### Quick status overview

```bash
squad-station status
# Project: my-app
# DB: /Users/you/.agentic-squad/my-app/station.db
# Agents: 3 -- 2 idle, 1 busy, 0 dead
#
#   my-app-claude-code-orchestrator: idle 5m   |  0 pending
#   my-app-claude-code-frontend: busy 2m       |  1 pending
#   my-app-gemini-backend: idle 10m            |  0 pending
```

### Agent list with live tmux reconciliation

```bash
squad-station agents
# NAME                              ROLE          STATUS            TOOL
# my-app-claude-code-orchestrator   orchestrator  idle 5m           claude-code
# my-app-claude-code-frontend       worker        busy 2m           claude-code
# my-app-gemini-backend             worker        idle 10m          gemini
```

The `agents` command checks tmux to detect crashed sessions:
- Session gone → agent marked **dead**
- Session reappears → agent revived to **idle**

### Message log

```bash
# All messages
squad-station list

# Filter by agent
squad-station list --agent my-app-claude-code-frontend

# Filter by status
squad-station list --status pending

# Limit results
squad-station list --limit 5

# JSON output
squad-station list --agent my-app-gemini-backend --status completed --json
```

### Interactive TUI dashboard

```bash
squad-station ui
```

Controls:
- `j`/`k` or arrow keys — navigate agents
- `Tab` — switch between agent and message panels
- `q` or `Esc` — quit

The dashboard auto-refreshes every 3 seconds.

### tmux tiled view

```bash
squad-station view
```

Opens a tiled tmux layout showing all live agent sessions side by side.

---

## 6. Check Pending Work

```bash
# What's the next task for an agent?
squad-station peek my-app-claude-code-frontend
# [pending] (priority=high) Add form validation
# id: a1b2c3d4-...

# JSON mode
squad-station peek my-app-claude-code-frontend --json
# {
#   "id": "a1b2c3d4-...",
#   "task": "Add form validation",
#   "priority": "high",
#   "status": "pending"
# }

# No pending work
squad-station peek my-app-gemini-backend
# No pending tasks for my-app-gemini-backend
```

Peek returns the highest-priority task first (urgent > high > normal), with oldest-first tie-breaking.

---

## 7. Signal Completion

If hooks are set up, this happens automatically. For manual signaling:

```bash
squad-station signal my-app-claude-code-frontend
# ✓ Signaled completion for my-app-claude-code-frontend (task_id=a1b2c3d4-...)
```

What happens:
1. Most recent pending message is marked "completed"
2. Orchestrator receives a notification in its tmux session:
   `my-app-claude-code-frontend completed a1b2c3d4-...`
3. Agent status resets to "idle"

**Signal format:** The notification injected into the orchestrator's tmux session is a plain string:

```
<agent> completed <msg-id>
```

Example: `my-app-claude-code-frontend completed 8c2e9e2f-1234-...`

Duplicate signals are safe — they silently succeed.

---

## 8. Register Agents at Runtime

Add agents without restarting the squad:

```bash
squad-station register reviewer \
  --role reviewer \
  --tool claude-code
```

This registers the agent in the database but does **not** launch a tmux session. Use this for agents managed externally.

If no `squad.yml` is available, you can point to the database directly:

```bash
SQUAD_STATION_DB=/path/to/station.db squad-station register my-agent --tool claude-code
```

---

## 9. Generate Orchestrator Context

```bash
squad-station context
```

Outputs a Markdown document with the agent roster and usage examples. Feed this to your orchestrator so it knows which agents are available and how to dispatch tasks.

```
# Squad Station -- Agent Roster

## Available Agents

## my-app-claude-code-frontend (claude-sonnet-4-5)

Frontend specialist

Role: worker | Status: idle

→ squad-station send my-app-claude-code-frontend --body "..."

---

## my-app-gemini-backend

Role: worker | Status: busy

→ squad-station send my-app-gemini-backend --body "..."

---

## Usage

Send a task to an agent:
```
squad-station send <agent> --body "<task description>"
```
```

---

## Workflow Summary

```
┌─────────────────────────────────────────────────────────────┐
│                        Orchestrator                          │
│                                                             │
│  1. Reads context (squad-station context)                   │
│  2. Sends tasks (squad-station send agent --body "task")    │
│  3. Receives signals via tmux notification                  │
│     <agent> completed <msg-id>                              │
│  4. Sends next task or coordinates results                  │
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
| `send <agent> --body <task>` | Send task to agent | `--body`, `--priority`, `--json` |
| `signal <agent>` | Signal task completion | `--json` |
| `peek <agent>` | View next pending task | `--json` |
| `list` | List messages | `--agent`, `--status`, `--limit`, `--json` |
| `register <name>` | Register agent at runtime | `--role`, `--tool`, `--json` |
| `agents` | List agents with status | `--json` |
| `status` | Project overview | `--json` |
| `context` | Generate orchestrator context | — |
| `ui` | Interactive TUI dashboard | — |
| `view` | tmux tiled view | `--json` |

All commands support `--json` for machine-readable output and `--help` for usage details.

---

## Troubleshooting

**Agent shows "dead" status**
The tmux session crashed or was closed. Re-run `squad-station init` to relaunch, or manually start a new tmux session with the agent's name.

**"Agent not found" when sending**
The agent name doesn't match any registered agent. Check `squad-station agents` for the exact names. Remember: full agent names are prefixed as `<project>-<tool>-<role_suffix>`.

**"tmux session not running" when sending**
The agent is registered but its tmux session is down. Re-run `squad-station init` or launch the session manually.

**Hook not firing**
Verify the hook path is absolute and the script is executable (`chmod +x hooks/claude-code-notify.sh`). Check that the agent is running inside a tmux session (hooks check `TMUX_PANE`).

**Database locked errors**
Squad-station uses single-writer SQLite. If you see lock errors, ensure only one write operation runs at a time. The 5-second busy timeout handles most concurrent cases.
