# Assistant Personas

This folder contains **assistant personas** that transform any LLM chat interface into a project-aware assistant for Squad Station.

## Concept

An assistant persona is a prompt file that, when loaded into an LLM chat session, turns that session into a specialized project assistant. The assistant does not work on code directly — instead, it acts as a **bridge** between the user and the Agent Orchestrator running in tmux.

### How It Works

```
User (natural language, any language)
  │
  ▼
Assistant Persona (LLM chat session)
  │  ── translates intent into structured tasks
  ▼
Agent Orchestrator (tmux)
  │  ── coordinates brainstorm + implement agents
  ▼
Results flow back up
  │
  ▼
Assistant translates and advises the user
```

1. **User** opens their preferred LLM chat interface (Claude Desktop Code, Antigravity, Cursor, etc.)
2. **User** invokes the persona via slash command (e.g., `/assistant-squad-station`)
3. The persona prompt loads, transforming the LLM into a project assistant
4. The assistant **delegates all technical work** to the orchestrator and its agent team
5. Results are translated back to the user with analysis and recommendations

### Why This Matters

- **Language barrier removal** — Users speak naturally in their preferred language; the assistant handles all English communication with the orchestrator
- **Decision support** — The assistant evaluates agent outputs, highlights risks and trade-offs, and provides actionable recommendations
- **No context switching** — Users stay in their familiar chat interface while a full agent team works in the background
- **Provider-agnostic** — Personas can be adapted for any LLM chat interface that supports prompt loading

## Directory Structure

```
assistant/
├── README.md                  # This file
└── claude-code/               # Personas for Claude Code compatible interfaces
    └── assistant-squad-station.md
```

### `claude-code/`

Contains personas designed for **Claude Code** and compatible interfaces. These personas are loaded via Claude Code's slash command system (`/project:` or custom commands).

## Creating New Personas

A persona file should define:

- **Role** — What the assistant does (advisor, translator, reviewer, etc.)
- **Constraints** — What the assistant must NOT do (e.g., never analyze code directly)
- **Workflow** — Step-by-step flow for handling user requests
- **Communication protocol** — How to interact with the orchestrator via tmux
- **Advisory framework** — How to evaluate and present agent results

Place new personas in the subfolder matching the target LLM interface. If no subfolder exists yet, create one following the pattern: `assistant/<interface-name>/`.
