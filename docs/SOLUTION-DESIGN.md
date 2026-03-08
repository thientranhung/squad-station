# Squad Station — Solution Design

> Source of truth. Based on Obsidian `02. Solution Design - Squad Station.md`.
> Confirmed decisions: **Rust** (not Go), **sqlx** (not rusqlite).
> Go-specific sections replaced by `TECH-STACK.md`.

---

## 1. Core Concept

User only communicates with **Orchestrator**. Orchestrator reasons, makes decisions, sends tasks to **Station**. Station routes to the correct agent via tmux. Agent completes work, **hook automatically** reports to Station. Orchestrator uses **tmux capture-pane** to read actual output.

```
                    ┌───────────────┐
                    │     USER      │
                    └───────┬───────┘
                            │ conversation
                            ▼
                    ┌───────────────┐
                    │ ORCHESTRATOR  │  ← any AI tool
                    │ (tmux session)│
                    └───────┬───────┘
                            │
              ┌─────────────▼─────────────┐
              │       SQUAD STATION        │
              │  (Rust binary + SQLite)    │
              │                            │
              │  • Receive task from Orch  │
              │  • Route to agent (tmux)   │
              │  • Receive signal from hook│
              │  • Notify Orch             │
              │  • Track state             │
              └──────┬──────┬──────┬───────┘
                     │      │      │
                     ▼      ▼      ▼
              ┌──────┐ ┌──────┐ ┌──────┐
              │Agent │ │Agent │ │Agent │
              │  A   │ │  B   │ │  C   │
              │(tmux)│ │(tmux)│ │(tmux)│
              └──────┘ └──────┘ └──────┘
```

**Key points:**
- Agent is **completely passive** — does not know about Station, does not send results
- **Hook** is an external layer on the agent, auto-detects Stop/AfterAgent events → reports to Station
- Orchestrator **captures output** from agent session (`tmux capture-pane -t <agent> -p`)
- Station is a **stateless CLI** — exits after completion, no background process

### Two-directional communication

```
  OUTBOUND (delegate task):
    Orchestrator ──► Station ──► tmux send-keys ──► Agent
                     (save DB)   (inject prompt)

  INBOUND (report completion):
    Agent done ──► Hook auto-fires ──► Station ──► notify Orchestrator
                   (agent is unaware)  (save DB)   (tmux send-keys)
                                                        │
                                          Orchestrator ──► tmux capture-pane
                                          (reads raw output from agent session)
```

## 2. Config File — `squad.yml`

```yaml
project: myapp

orchestrator:
  provider: claude
  model: opus
  description: >
    Main orchestrator. Receives requests from user, reasons,
    delegates tasks to agents, reads results, synthesizes.

agents:
  - name: implement
    provider: claude
    model: sonnet
    description: >
      Developer agent. Writes code, fixes bugs, runs tests.
  - name: brainstorm
    provider: gemini
    model: gemini-2.5-pro
    description: >
      Architect & reviewer. Designs architecture, reviews code.
  - name: docs
    provider: claude
    model: haiku
    description: >
      Technical writer. Writes documentation.
  - name: test
    provider: claude
    model: sonnet
    description: >
      QA agent. Writes test cases, runs test suites.
```

### Required fields

| Field | Type | Description |
|-------|------|-------------|
| `project` | string | Project name, used for DB path + agent name prefix |
| `orchestrator` | object | Exactly 1 orchestrator per squad |
| `orchestrator.provider` | string | AI tool label |
| `orchestrator.model` | string | AI model name |
| `orchestrator.description` | string | Role description, used for context generation |
| `agents` | array | List of worker agents |
| `agents[].name` | string | Role name (will be prefixed with `<project>-<provider>-`) |
| `agents[].provider` | string | AI tool label |
| `agents[].model` | string | AI model name |
| `agents[].description` | string | Capability description |

### Initialization from config

```
  $ squad-station init

  Read squad.yml
      │
      ├── Create ~/.agentic-squad/myapp/station.db
      │
      ├── Register agents in DB (role = orchestrator | worker):
      │   myapp-claude-orchestrator    role=orchestrator  ← hook will SKIP
      │   myapp-claude-implement       role=worker
      │   myapp-gemini-brainstorm      role=worker
      │   myapp-claude-docs            role=worker
      │   myapp-claude-test            role=worker
      │
      ├── Create tmux sessions + launch AI tools
      │
      └── Generate orchestrator context file
```

### Orchestrator context — auto-generated

Station creates a context file so orchestrator knows its available agents:

```
  # Squad Agents

  You are the orchestrator for project myapp.
  You have the following agents to delegate tasks to:

  ## myapp-claude-implement (Claude Sonnet)
  Developer agent. Writes code, fixes bugs, runs tests.
  → squad-station send myapp-claude-implement --body "..."

  ## myapp-gemini-brainstorm (Gemini 2.5 Pro)
  Architect & reviewer. Designs architecture, reviews code.
  → squad-station send myapp-gemini-brainstorm --body "..."

  ## How to delegate
  1. Reasoning → select appropriate agent
  2. squad-station send <agent> --body "<task>"
  3. Wait for notification when agent completes
  4. tmux capture-pane -t <agent> -p to read result
  5. Reasoning → continue or report to user
```

## 3. Distinguishing Squad Agent vs Independent Agent

```
  AGENT IN SQUAD                     INDEPENDENT AGENT

  Tmux session:                      Tmux session:
  "myapp-claude-implement"           "my-personal-claude"
  (registered in DB)                 (NOT in DB)

  Agent stops → Hook fires           Agent stops → Hook fires
  → squad-station signal             → squad-station signal
    "myapp-claude-implement"            "my-personal-claude"
  → FOUND in DB ✓                    → NOT FOUND → SKIP ✓
  → Update completed + notify orch   → Exit 0, silent
```

**4-layer guard in hook:**
1. Not in tmux → exit (user uses AI tool from regular terminal)
2. Agent not registered in Station → exit (instance outside squad)
3. Agent has role = `orchestrator` → exit (prevent loop)
4. No task processing → exit (agent in squad but chatting freely)

## 4. Real-world Scenarios

### 4.1 Happy path — delegate task, receive result

```
  Orchestrator:
  > squad-station send myapp-claude-implement \
    --body "Implement JWT auth module..."

  ✓ Message msg-a1b2c3 created
  ✓ Injected into myapp-claude-implement

  ... (wait) ...

  "myapp-claude-implement completed msg-a1b2c3"

  > tmux capture-pane -t myapp-claude-implement -p
  > 12 tests passed, 3 files created ✓
```

### 4.2 Multi-agent — implement then review

```
  Orchestrator ──► send implement "Implement auth"
                   ... wait ...
               ◄── "implement done" → capture output
               ──► send brainstorm "Review auth code"
                   ... wait ...
               ◄── "brainstorm done" → capture output
               → Synthesize for user
```

### 4.3 Parallel — send to N agents simultaneously

```
  Orchestrator ──► send implement  (write code)
               ──► send test       (write tests)
               ──► send docs       (write docs)
               ... wait for all ...
               ◄── "docs done"
               ◄── "test done"
               ◄── "implement done"
               → Synthesize
```

### 4.4 Agent dies mid-work

```
  Orchestrator ──► send implement
                   ... wait too long ...
               ──► squad-station status
               ◄── implement: DEAD (tmux session does not exist)
               → Reasoning: Restart or reassign
```

### 4.5 Multiple projects simultaneously

```
  Project myapp:     DB: ~/.agentic-squad/myapp/station.db
  Project api-svc:   DB: ~/.agentic-squad/api-svc/station.db
  → Fully isolated, separate DBs, different agent name prefixes
```

## 5. Workflow Diagrams

### 5.1 Delegate task

```
  Orchestrator                   Station                       Agent
      │                             │                             │
      │  ① squad-station send       │                             │
      │    <agent-name>              │                             │
      │    --body "task prompt"      │                             │
      │  ──────────────────────►     │                             │
      │                             │  INSERT message             │
      │                             │  (status=processing)        │
      │                             │  UPDATE agent → busy        │
      │  ◄── message_id            │  + current_task             │
      │                             │                             │
      │                             │  tmux send-keys ───────────►│
      │                             │  (inject task prompt)        │
      │                             │                             │
      │                             │                             │  ② Works
```

### 5.2 Completion

```
  Orchestrator                   Station                       Agent
      │                             │                             │
      │                             │                             │  ③ Agent stops
      │                             │                             │  ④ HOOK fires
      │                             │  ◄─────────────────────────│  ⑤ squad-station signal
      │                             │                             │
      │                             │  SELECT FROM messages       │
      │                             │  WHERE to_agent=<agent>     │
      │                             │  AND status='processing'    │
      │                             │                             │
      │                             │  UPDATE msg → completed     │
      │                             │  UPDATE agent → idle        │
      │                             │                             │
      │  ⑥ tmux send-keys ◄─────  │                             │
      │  "<agent> completed         │                             │
      │   <message_id>"             │                             │
      │                             │                             │
      │  ⑦ tmux capture-pane      │                             │
      │    (read raw output)       │                             │
      │  ⑧ Reasoning/summarize    │                             │
```

## 6. Hook System

### 6.1 Each provider needs 2 hooks

| Event | Claude Code | Gemini CLI | Purpose |
|-------|-------------|------------|---------|
| **Stop / AfterAgent** | `Stop` | `AfterAgent` | Task completed → signal Station |
| **Notification** | `Notification` (matcher: `permission_prompt`) | `Notification` | Agent needs approval → forward to Orchestrator |

### 6.2 Hook auto-detects agent

`.claude/settings.json` or `.gemini/settings.json` declares **1 hook command** per event. Hook auto-detects tmux session name → that is the agent name.

```bash
#!/bin/bash
# squad-on-stop.sh
AGENT_NAME=$(tmux display-message -p '#S' 2>/dev/null) || exit 0
squad-station signal "$AGENT_NAME" 2>/dev/null || exit 0
```

### 6.3 Orchestrator skip guard

```
  Station checks DB:
    SELECT role FROM agents WHERE name = $AGENT_NAME
    → role = 'orchestrator' → SKIP, exit 0
    → role = 'worker'       → UPDATE completed, notify orch
```

## 7. Station is a Stateless CLI

Every command exits after completion. No daemon, no background process.

```
  $ squad-station send ...       ← runs ~50ms, writes DB + tmux inject, exits
  $ squad-station signal ...     ← runs ~50ms, updates DB + notify, exits
  $ squad-station status         ← runs ~10ms, queries DB + prints, exits
```

## 8. Agent Naming Convention

Agent name = Tmux session name. Format: `<project>-<provider>-<role>`

```
  Pattern:  <project>-<provider>-<role>

  project   = project name (from squad.yml)
  provider  = gemini, claude, aider, codex, ...
  role      = orchestrator, implement, brainstorm, test, docs, ...

  Agent name = tmux session name
  → Hook detects agent via: tmux display-message -p '#S'
  → Station looks up tmux session via: agent name
```

## 9. Data Model

```
station.db  (1 file per project: ~/.agentic-squad/<project>/)

┌──────────────────────────────────────────────────┐
│ messages                                          │
│                                                   │
│  id              TEXT PK          (UUID)          │
│  from_agent      TEXT             (sender)        │
│  to_agent        TEXT             (recipient)     │
│  type            TEXT             (task / signal)  │
│  priority        TEXT             (normal/high/    │
│                                    urgent)        │
│  status          TEXT             (processing /    │
│                                    completed /    │
│                                    failed)        │
│  body            TEXT             (task prompt)    │
│  created_at      DATETIME                         │
│  completed_at    DATETIME                         │
└──────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────┐
│ agents                                            │
│                                                   │
│  name            TEXT PK          (<proj>-<prov>  │
│                                    -<role>)       │
│  role            TEXT             (orchestrator /  │
│                                    worker)        │
│  tool            TEXT             (claude-code,    │
│                                    gemini-cli)    │
│  model           TEXT             (sonnet, opus)   │
│  description     TEXT             (from squad.yml) │
│  status          TEXT             (idle/busy/dead) │
│  current_task    TEXT FK          (→ messages.id)  │
│  last_heartbeat  DATETIME                         │
└──────────────────────────────────────────────────┘

Relationships:
  agents.current_task ──► messages.id
  Hook only needs signal <agent-name>, does NOT need to know task ID
```

## 10. Message Lifecycle

```
                    Station send             Hook signal
                    + tmux inject            (agent done)
                      to agent
  send ───────► PROCESSING ──────────────────────► COMPLETED
                     │
                     │ abort / fail / timeout
                     ▼
                  FAILED / ABORTED
```

## 11. CLI Interface

```
squad-station — Message routing for AI agent squads

  Setup:
    init                              Read squad.yml, create station
    register <agent> [--tool] [--model] [--role] [--description]

  Info:
    status                            Quick summary
    agents                            List agents
    list [--agent] [--status] [--limit]

  Messaging:
    send <to> --body "..."            Send task + inject tmux
         [--type] [--priority]

  Signal:
    signal <agent>                    Hook reports completed/failed
           [--status completed|failed]
    peek <agent>                      Any pending task?

  Display:
    ui                                Dashboard TUI
    view                              Split tmux view

  Flags:
    --json                            Machine-readable output (global)
```

## 12. Dashboard

### `squad-station ui`

```
  ┌─────────────────────────────────────────────────────────────┐
  │  SQUAD STATION — myapp                                      │
  │                                                              │
  │  AGENTS                          STATUS    CURRENT TASK      │
  │  myapp-gemini-orchestrator       ● idle    —                 │
  │  myapp-claude-implement          ● busy    msg-a1b2 (auth)   │
  │  myapp-claude-brainstorm         ● idle    —                 │
  │                                                              │
  │  RECENT MESSAGES                                             │
  │  msg-a1b2  orch → implement    processing  2m ago            │
  │  msg-c3d4  orch → brainstorm   completed   5m ago            │
  │                                                              │
  │  [1-5] attach  [r] refresh  [q] quit                         │
  └─────────────────────────────────────────────────────────────┘
```

### `squad-station view`

Split tmux layout showing all live agent sessions side by side.

## 13. Methodology Integration (v2 Roadmap)

```yaml
# squad.yml — v2 will add methodology field
project: myapp
methodology: bmad          # bmad | superpower | speckit | openspec | none
```

> **v1:** User sets up methodology manually. Station only handles agent coordination.
> **v2:** Integrate methodology into squad.yml + auto-install on init.

---
*Source: Obsidian/1-Projects/Agentic-Coding-Squad/02. Solution Design - Squad Station.md*
*Updated: Rust confirmed (supersedes Go references in original)*
