# Squad Station

Message routing and orchestration for AI agent squads — coordinate multiple AI agents through a single orchestrator using tmux sessions and SQLite messaging.

Squad Station is a stateless Rust CLI that routes tasks between an AI orchestrator and N worker agents. It is provider-agnostic: works with Claude Code, Gemini CLI, or any AI tool that runs in a terminal. Each project stores its state in a local SQLite database at `.squad/station.db`.

## Features

- **Provider-agnostic** — Mix and match Claude Code, Gemini CLI, or any terminal-based AI tool in a single squad
- **SDD workflow orchestration** — Plug in structured development methodologies (Get Shit Done, BMad Method, OpenSpec, Superpowers) as playbooks that define how the orchestrator delegates work
- **Automatic signal hooks** — Agent completion hooks auto-installed for each provider; the orchestrator is notified without polling
- **Orchestrator bootstrap** — Survives `/clear` and context compaction; the orchestrator always knows its role
- **Tiered tool restrictions** — Orchestrator acts as a PM: reads dashboards and status files, delegates all code work to agents
- **Health monitor watchdog** — Background daemon checks tmux session liveness, marks dead agents, revives recovered sessions
- **Interactive TUI** — Real-time dashboard showing agent status and message flow
- **SDD git workflow rules** — Auto-installed rule templates that guide agents on branching, commits, and PR conventions

## Installation

### npm (recommended)

```bash
npx squad-station@latest install
```

This downloads the native binary for your platform and scaffolds starter files:
- `.squad/sdd/` — SDD playbook templates (GSD, BMad, OpenSpec, Superpowers)
- `.squad/rules/` — Git workflow rule templates per SDD methodology
- `.squad/examples/` — Example `squad.yml` configs for Claude and Gemini setups

Requires Node.js 14+. Handles macOS quarantine (xattr removal) and upgrades over existing cargo-installed symlinks automatically.

### curl

```bash
curl -fsSL https://raw.githubusercontent.com/thientranhung/squad-station/master/install.sh | sh
```

Installs to `/usr/local/bin` (falls back to `~/.local/bin`). Supports macOS and Linux.

### Build from source

```bash
git clone https://github.com/thientranhung/squad-station.git
cd squad-station
cargo build --release
# Binary: target/release/squad-station
```

Requires Rust toolchain. See [Cargo docs](https://doc.rust-lang.org/cargo/getting-started/installation.html).

## Quick Start

### 1. Create `squad.yml`

```yaml
project: my-app

sdd:
  - name: get-shit-done
    playbook: ".squad/sdd/gsd-playbook.md"

orchestrator:
  provider: claude-code
  role: orchestrator
  model: haiku
  description: Team leader, coordinates tasks for agents

agents:
  - name: implement
    provider: claude-code
    role: worker
    model: sonnet
    description: Senior coder, coding, fixing bugs

  - name: brainstorm
    provider: claude-code
    role: worker
    model: opus
    description: Technical lead, planning, analysis, code reviews
```

### 2. Initialize

```bash
squad-station init
```

This:
- Registers all agents in the SQLite database
- Launches tmux sessions for the orchestrator and each worker
- Installs provider-specific signal and notification hooks (Claude Code `.claude/settings.json`, Gemini CLI `.gemini/settings.json`)
- Generates the orchestrator playbook at `.claude/commands/squad-orchestrator.md` (or `.gemini/commands/squad-orchestrator.toml`)
- Injects a bootstrap block into project-root `CLAUDE.md` / `GEMINI.md` so the orchestrator survives `/clear` and context compaction
- Copies SDD git workflow rules into `.claude/rules/` or `.gemini/rules/`
- Starts the watchdog health monitor daemon

### 3. Start the orchestrator

Switch to the orchestrator's tmux session and invoke the playbook:

```bash
# Claude Code:
/squad-orchestrator

# Gemini CLI:
@squad-orchestrator
```

The orchestrator reads its playbook and begins coordinating agents via `squad-station send`.

### 4. Monitor

```bash
squad-station status           # Agent overview: statuses, pending tasks
squad-station list             # List recent messages
squad-station agents           # Agent roster with live status
squad-station ui               # Interactive TUI dashboard
squad-station view             # Tiled tmux view of all agent panes
```

## CLI Reference

| Command | Description |
|---------|-------------|
| `init [config]` | Initialize squad from `squad.yml` (default), launch sessions, install hooks |
| `send <agent> --body "<task>"` | Send a task to an agent (supports `--priority urgent\|high\|normal`) |
| `signal [agent]` | Signal agent task completion (auto-detects agent from tmux session) |
| `notify --body "<msg>"` | Mid-task notification from agent to orchestrator |
| `list` | List messages (`--agent`, `--status`, `--limit` filters) |
| `peek <agent>` | Show an agent's next pending task |
| `agents` | List agents with reconciled status |
| `status` | Project and agent status summary |
| `context` | Regenerate orchestrator playbook file |
| `ui` | Interactive TUI dashboard |
| `view` | Tiled tmux view of all live sessions |
| `reconcile` | Detect and fix stuck agents (`--dry-run` supported) |
| `watch` | Watchdog health monitor (`--daemon`, `--stop`, `--interval`) |
| `doctor` | Health check to diagnose operational issues |
| `reset [config]` | Kill sessions, delete database, relaunch |
| `clean [config]` | Kill sessions and delete database (`--all` includes logs) |
| `freeze` / `unfreeze` | Block or allow orchestrator task dispatch |

All commands support `--json` for machine-readable output.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  User                                                   │
│    └─→ Orchestrator (tmux session)                      │
│          ├─→ squad-station send agent-A --body "..."    │
│          ├─→ squad-station send agent-B --body "..."    │
│          └─← [SQUAD SIGNAL] Agent 'A' completed task 7 │
│                                                         │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐              │
│  │ Agent A  │  │ Agent B  │  │ Agent C  │  (tmux)      │
│  │ Claude   │  │ Gemini   │  │ Claude   │              │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘              │
│       └──────────────┴─────────────┘                    │
│                      │                                  │
│              ┌───────▼────────┐                         │
│              │  SQLite (WAL)  │                         │
│              │ .squad/station │                         │
│              │    .db         │                         │
│              └────────────────┘                         │
└─────────────────────────────────────────────────────────┘
```

- **Stateless CLI** — Every command opens the database, reads or writes, and exits. No background daemon required (watchdog is optional).
- **SQLite WAL mode** — Single-writer pool with 5s busy timeout. Two tables: `agents` (name, provider, role, status, model) and `messages` (agent, task, status, priority, timestamps).
- **tmux sessions** — Each agent runs in its own named session. `send-keys -l` (literal mode) prevents shell injection. `$SQUAD_AGENT_NAME` env var set at launch.
- **Provider hooks** — Auto-installed by `init`. Claude Code uses Stop + Notification + PostToolUse hooks. Gemini CLI uses AfterAgent + Notification hooks. Hooks call `squad-station signal` on task completion to notify the orchestrator.
- **Watchdog** — Pure health monitor. Checks tmux session liveness on a 30s interval: marks agents "dead" if their session crashed, revives to "idle" if the session reappears. No task completion logic.
- **Signal logging** — Structured logs at `.squad/log/signal.log` for debugging signal flow.

## Configuration

### `squad.yml`

```yaml
project: my-app                    # Project name (used as tmux session prefix)

sdd:                               # Optional: SDD methodology playbooks
  - name: get-shit-done
    playbook: ".squad/sdd/gsd-playbook.md"

orchestrator:
  provider: claude-code            # claude-code | gemini-cli
  role: orchestrator
  model: haiku                     # Model for the orchestrator session
  name: lead                       # Optional: custom session name suffix
  description: Team coordinator

agents:
  - name: backend                  # Agent name (session: my-app-claude-code-backend)
    provider: claude-code
    role: worker
    model: sonnet
    description: Backend implementation

  - name: frontend
    provider: gemini-cli
    role: worker
    model: gemini-2.5-pro
    description: Frontend implementation
```

### Supported SDD Playbooks

| Playbook | File | Description |
|----------|------|-------------|
| Get Shit Done | `.squad/sdd/gsd-playbook.md` | Fast iteration, minimal ceremony |
| BMad Method | `.squad/sdd/bmad-playbook.md` | Structured agile with defined roles |
| OpenSpec | `.squad/sdd/openspec-playbook.md` | Spec-driven: propose, apply, archive |
| Superpowers | `.squad/sdd/superpowers-playbook.md` | Full-stack autonomous development |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `SQUAD_STATION_DB` | Override database path (default: `.squad/station.db` in project root) |
| `SQUAD_AGENT_NAME` | Set automatically in each agent's tmux session |

## Requirements

- **tmux** — Required for agent session management
- **macOS or Linux** — Windows is not supported (tmux unavailable)
- **Node.js 14+** — For npm installation method only

## License

MIT License
