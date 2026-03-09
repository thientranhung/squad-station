# Squad Station — Vision & Scope

> Source of truth. Based on Obsidian `01. Vision & Scope.md` + confirmed decisions.
> Updated: incorporates `04. Upgrade Design — Antigravity & Hooks Optimization`.

---

## 1. Vision

Build a **team of AI agents** working collaboratively on the same codebase, where each agent can be any AI coding tool (Claude Code, Gemini CLI, Codex, etc.). Agents are connected and coordinated through **tmux sessions** and a **central messaging system**.

The user only interacts with **a single orchestrator**. The orchestrator reasons autonomously, delegates tasks to agents, receives results, and decides next steps — forming an automated loop until work is complete.

## 2. Architecture Overview

### 2.1 Orchestrator — Agents Model

**Provider-agnostic:** Both Orchestrator and Agents can be **any AI tool** — Claude Code, Gemini CLI, Codex, Aider, Antigravity IDE, etc. No hard-coding of Gemini = orchestrator or Claude = agent.

#### Two Orchestrator Modes

| Mode | Runtime | Communication | Examples |
|------|---------|---------------|----------|
| **CLI-based** | Orchestrator lives inside a tmux session | Event-driven: Station notifies via `tmux send-keys` | Gemini CLI, Claude Code |
| **IDE-based** | Orchestrator runs natively inside an IDE | Polling: IDE monitors tmux worker panes directly | Antigravity (Manager View) |

When the orchestrator is **IDE-based** (e.g. Antigravity):
- The IDE has a built-in **Manager View / Mission Control** that can spawn, poll, and monitor multiple agents.
- **No tmux session is created for the Orchestrator itself** — only for worker agents.
- **No notification back to Orchestrator** — the IDE actively polls tmux worker panes and detects completion autonomously.
- Squad Station's role is simplified to a **Bootstrapper + State Registry** (DB tracking only).

```
┌─────────────────────────────────────────────────────────────────┐
│                           USER                                  │
│                   Only talks to Orchestrator                    │
└──────────────────────────┬──────────────────────────────────────┘
                           │ conversation
                           ▼
┌─────────────────────────────────────────────────────────────────┐
│              TMUX SESSION: Orchestrator (Master)                │
│                                                                 │
│  Tool: ANY — Gemini CLI / Claude Code / Codex / ...            │
│  Role: PM, Tech Lead — reasoning, decisions, delegation        │
│                                                                 │
│  ⚠ Hook MUST SKIP orchestrator session to prevent loop         │
│    (orchestrator stops → hook fires → notifies itself → loop)  │
│                                                                 │
│  Loop:                                                          │
│    1. Receive request from user                                 │
│    2. Reasoning → select appropriate agent                      │
│    3. Send task to agent via messaging                           │
│    4. Wait for result (event-driven, no polling)                │
│    5. Read result → continue reasoning → repeat or stop         │
└────────┬───────────────────────────────┬────────────────────────┘
         │ delegate task                 │ delegate task
         ▼                              ▼
┌────────────────────────┐  ┌────────────────────────────────────┐
│  TMUX SESSION: Agent 1 │  │  TMUX SESSION: Agent 2             │
│  (implement)           │  │  (brainstorm)                      │
│                        │  │                                    │
│  Tool: Claude Code     │  │  Tool: Gemini CLI                  │
│  Model: Sonnet         │  │  Model: gemini-2.5-pro             │
│  Role: Developer       │  │  Role: Architect / QA              │
│  - Write code          │  │  - Design architecture             │
│  - Fix bugs            │  │  - Review code                     │
│  - Run tests           │  │  - Solve complex problems          │
└────────────────────────┘  └────────────────────────────────────┘

  Agents can be ANY provider:
  Claude Code, Gemini CLI, Codex, Aider, ...
```

### 2.2 Communication Flow

```
  Orchestrator             Messaging System                Agent
      │                          │                           │
      │  1. send(task_request)   │                           │
      │  ───────────────────►    │                           │
      │                          │  2. notify agent          │
      │                          │  ─────────────────────►   │
      │                          │                           │
      │                          │                           │  3. Agent works
      │                          │                           │     (write code, review...)
      │                          │                           │
      │                          │  4. send(task_completed)  │
      │                          │  ◄─────────────────────   │
      │  5. notify orchestrator  │                           │
      │  ◄───────────────────    │                           │
      │                          │                           │
      │  6. read result          │                           │
      │  ───────────────────►    │                           │
      │  ◄───────────────────    │                           │
      │  7. Reasoning...         │                           │
      │     → delegate next      │                           │
      │     → or report to user  │                           │
```

**Key points:**
- **CLI Orchestrator** does not poll agent screens — it receives notifications via the messaging system (`tmux send-keys`).
- **IDE Orchestrator** (Antigravity) actively polls worker tmux panes — Station only updates DB state, no notification sent.
- Agent **does not send tmux keystrokes** directly to orchestrator. Agent writes message to the messaging system, the system handles notification.
- Each exchange needs a way to **link request ↔ response** to distinguish work streams.

### 2.3 Hook Requirements — Provider-Agnostic

Each provider must declare **2 hook events**:

| Event | Claude Code | Gemini CLI | Purpose |
|-------|-------------|------------|---------|
| **Stop/AfterAgent** | `Stop` | `AfterAgent` | Agent completes task → signal Station |
| **Notification** | `Notification` (matcher: `permission_prompt`) | `Notification` | Agent needs user approval → forward to Orchestrator |

**Guard: Hook MUST skip orchestrator session**

```
  Agent stops → Hook fires → Check: "Am I the orchestrator?"
                                    │
                          ┌─────────┴─────────┐
                          ▼                   ▼
                     YES (orchestrator)    NO (agent)
                     → exit 0, silent      → signal Station
                     → PREVENT LOOP              │
                                        ┌────────┴────────┐
                                        ▼                 ▼
                                   CLI Orch           IDE Orch
                                   → tmux send-keys   → DB update only
                                     (notify)           (no notify)
```

## 3. Problems to Solve

### 3.1 Hook cannot distinguish context

```
Case 1 — Orchestrated (correct):
  Orchestrator sends task ──► Agent completes ──► Hook fires ──► Sends to Orchestrator ✓

Case 2 — Independent (wrong):
  User chats directly with Agent ──► Agent responds ──► Hook fires ──► Sends to Orchestrator ✗
```

**Solution:** Station checks if there's a task processing for the agent. If none → skip.

### 3.2 Cannot identify message source

When multiple agents send messages back to orchestrator, need `from_agent`/`to_agent` and thread/conversation tracking.

### 3.3 Multi-project concurrency

On the same machine, user may run 2-3 projects simultaneously. Messaging system must fully isolate between projects (1 DB file per project).

### 3.4 Git conflicts with concurrent agents

Out of scope — orchestrator should sequence work to avoid conflicts.

### 3.5 Context window decay

Prompt instructions weaken over long conversations. This is a fundamental LLM limitation, cannot be fully solved at infrastructure layer.

## 4. Scope

**In scope:**
- Central messaging system between tmux sessions
- Session registry and lifecycle management
- Multi-project concurrency on same machine
- Distinguish orchestrated workflow vs independent usage
- **IDE Orchestrator support** (Antigravity) — conditional notification skip, DB-only state tracking
- **Centralized hook handling** — single `squad-station signal` command replaces per-project shell scripts

**Out of scope:**
- Task management / workflow logic (orchestrator's responsibility)
- Orchestration decisions (Gemini/Claude/Antigravity's responsibility)
- File sync, code sharing between agents
- Web UI / browser dashboard (note: Antigravity Manager View is an IDE feature, not Station's)
- Git conflict resolution
- Agent-to-agent direct messaging

---
*Source: Obsidian/1-Projects/Agentic-Coding-Squad/01. Vision & Scope.md*
*Confirmed decisions from: 03. Tech Stack Decision*
*Updated with: 04. Upgrade Design — Antigravity & Hooks Optimization*
