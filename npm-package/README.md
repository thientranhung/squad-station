# Squad Station

Message routing and orchestration for AI agent squads — coordinate multiple AI agents through a single orchestrator using tmux sessions and SQLite messaging.

Provider-agnostic: works with Claude Code, Gemini CLI, or any terminal-based AI tool.

## Install

```bash
npx squad-station@latest install
```

This downloads the `squad-station` binary for your platform and scaffolds project files:

```
.squad/
├── sdd/                              # SDD methodology playbooks
│   ├── gsd-playbook.md
│   ├── bmad-playbook.md
│   ├── openspec-playbook.md
│   └── superpowers-playbook.md
├── rules/                            # Git workflow rule templates
│   ├── git-workflow-get-shit-done.md
│   ├── git-workflow-bmad-method.md
│   ├── git-workflow-openspec.md
│   └── git-workflow-superpowers.md
└── examples/                         # Example squad.yml configs
    ├── orchestrator-claude.yml
    └── orchestrator-gemini.yml
```

Handles macOS quarantine (xattr removal) and upgrades over existing cargo-installed symlinks automatically.

## Quick Start

```bash
# 1. Copy an example config
cp .squad/examples/orchestrator-claude.yml squad.yml

# 2. Edit — set project name, providers, models
vi squad.yml

# 3. Launch the squad
squad-station init
```

## Example `squad.yml`

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

## Commands

| Command | Description |
|---------|-------------|
| `squad-station init` | Launch squad from `squad.yml` — creates DB, sessions, hooks |
| `squad-station send <agent> --body "<task>"` | Send a task to an agent |
| `squad-station signal [agent]` | Signal agent task completion |
| `squad-station list` | List messages (`--agent`, `--status`, `--limit`) |
| `squad-station agents` | List agents with live status |
| `squad-station status` | Project and agent summary |
| `squad-station ui` | Interactive TUI dashboard |
| `squad-station view` | Tiled tmux view of all agent sessions |
| `squad-station doctor` | Health check for diagnosing issues |
| `squad-station watch --daemon` | Start watchdog health monitor |
| `squad-station reset` | Kill sessions, delete DB, relaunch |
| `squad-station clean` | Kill sessions and delete DB (`--all` includes logs) |

All commands support `--json` for machine-readable output.

## Requirements

- macOS or Linux
- tmux
- Node.js 14+ (for `npx install` only)

## Alternative Install

```bash
# curl
curl -fsSL https://raw.githubusercontent.com/thientranhung/squad-station/master/install.sh | sh

# From source
git clone https://github.com/thientranhung/squad-station.git
cd squad-station && cargo build --release
```

## License

MIT
