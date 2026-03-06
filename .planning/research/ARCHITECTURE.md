# Architecture Research

**Project:** Squad Station
**Domain:** Stateless Rust CLI — tmux message router with embedded SQLite
**Researched:** 2026-03-06
**Overall Confidence:** HIGH (patterns verified via official docs + active crates)

---

## Component Overview

Squad Station is a **stateless CLI binary** — every invocation starts fresh, reads from SQLite, executes one action, and exits. There is no daemon, no shared memory, no long-running process. SQLite is the only persistent state store.

### Top-Level System Map

```
User / Orchestrator AI
        |
        | shell commands
        v
┌──────────────────────────────────────────────────┐
│                 squad-station (binary)           │
│                                                  │
│  ┌──────────┐   ┌──────────┐   ┌─────────────┐  │
│  │   CLI    │──>│ Commands │──>│  Core Logic │  │
│  │  Layer   │   │ Handlers │   │   Services  │  │
│  └──────────┘   └──────────┘   └──────┬──────┘  │
│                                        │         │
│              ┌─────────────────────────┤         │
│              │                         │         │
│      ┌───────v──────┐    ┌─────────────v──────┐  │
│      │   DB Layer   │    │   Tmux Adapter     │  │
│      │  (rusqlite)  │    │ (tmux_interface)   │  │
│      └───────┬──────┘    └────────────────────┘  │
│              │                                   │
│     ~/.agentic-squad/<project>/station.db        │
└──────────────────────────────────────────────────┘
              |                          |
              v                          v
         SQLite file               tmux sessions
                                  (agent processes)
```

The TUI (`squad-station ui`) is a separate invocation path — same binary, different subcommand — that enters a blocking event loop instead of exiting immediately.

---

## Module Boundaries

### Module 1: `cli` — Argument Parsing Entry Point

**Responsibility:** Parse argv, resolve project context, dispatch to command handlers.

**Contains:**
- `Cli` struct (clap derive macro, top-level)
- `Commands` enum (subcommands: `init`, `send`, `signal`, `register`, `status`, `view`, `ui`, `context`)
- `Context` struct — normalized runtime context passed to all handlers

**Depends on:** Nothing internal. Uses `clap` and `std`.

**Boundary rule:** `cli` module never touches SQLite or tmux directly. It only parses arguments and builds a `Context`, then hands off.

```rust
// src/cli/mod.rs
pub struct Context {
    pub project: String,          // resolved project name
    pub db_path: PathBuf,         // ~/.agentic-squad/<project>/station.db
    pub config_path: PathBuf,     // squad.yml location
}

pub enum Commands {
    Init { config: Option<PathBuf> },
    Send { agent: String, message: String },
    Signal { status: SignalStatus },
    Register { name: String, session: String, provider: String },
    Status,
    View,
    Ui,
    Context,
}
```

---

### Module 2: `commands` — Command Handlers

**Responsibility:** Orchestrate one user-visible command. Each subcommand gets its own file. Handlers own the "what happens when user runs X" logic.

**Contains:** One handler function per subcommand.

```
src/commands/
  init.rs      — load squad.yml, upsert agents into DB
  send.rs      — look up agent, call tmux::send_keys, update DB state
  signal.rs    — detect current tmux session, update agent status in DB, notify orchestrator
  register.rs  — insert/update agent record in DB at runtime
  status.rs    — query DB, print agent table
  view.rs      — call tmux split-window to build split view
  context.rs   — query DB, write orchestrator context file
  ui.rs        — enter TUI event loop
```

**Depends on:** `db`, `tmux`, `config`, `tui` (ui.rs only).

**Boundary rule:** Handlers call into service abstractions (`db::AgentRepository`, `tmux::TmuxAdapter`). They do not construct SQL strings or shell command strings directly.

---

### Module 3: `db` — Database Layer

**Responsibility:** All SQLite access. Schema definition, migrations, typed queries.

**Contains:**

```
src/db/
  mod.rs          — Connection factory (open_db), run migrations on open
  migrations.rs   — Embedded SQL migrations via rusqlite_migration
  models.rs       — Domain types: Agent, Message, Project
  agent_repo.rs   — CRUD for agents table
  message_repo.rs — Insert/query for message_log table (audit trail)
```

**Schema (initial):**

```sql
CREATE TABLE agents (
    id        INTEGER PRIMARY KEY,
    name      TEXT NOT NULL UNIQUE,        -- naming convention: <project>-<provider>-<role>
    session   TEXT NOT NULL,               -- tmux session name (same as name)
    provider  TEXT NOT NULL,               -- claude|gemini|codex|aider|...
    role      TEXT NOT NULL,
    status    TEXT NOT NULL DEFAULT 'idle',-- idle|busy|dead
    registered_at  TEXT NOT NULL,
    last_signal_at TEXT
);

CREATE TABLE message_log (
    id         INTEGER PRIMARY KEY,
    agent_name TEXT NOT NULL,
    direction  TEXT NOT NULL,              -- sent|received
    content    TEXT NOT NULL,
    timestamp  TEXT NOT NULL
);
```

**Depends on:** `rusqlite`, `rusqlite_migration`. Nothing internal.

**Boundary rule:** All SQL lives here. No SQL strings escape this module. Returns domain types (`Agent`, etc.), not raw `Row` values.

**Migration strategy:** `rusqlite_migration` — migrations embedded as `M::up(sql)` constants in `migrations.rs`, applied automatically on `open_db()`. Uses SQLite `user_version` pragma (no migration table overhead).

---

### Module 4: `tmux` — Tmux Adapter

**Responsibility:** Thin wrapper over `tmux_interface` crate. Encapsulates all tmux shell-out operations.

**Contains:**

```
src/tmux/
  mod.rs        — TmuxAdapter struct, public interface
  send.rs       — send_message(session, message) → send-keys
  capture.rs    — capture_output(session) → String
  session.rs    — list_sessions(), session_exists(name), current_session_name()
  view.rs       — build_split_view(sessions: &[&str])
```

**Key operations:**

```rust
pub struct TmuxAdapter;

impl TmuxAdapter {
    // Inject prompt into agent tmux session
    pub fn send_message(&self, session: &str, message: &str) -> Result<()>;

    // Read raw pane output (for orchestrator capture)
    pub fn capture_pane(&self, session: &str) -> Result<String>;

    // Detect which tmux session we are currently running inside
    pub fn current_session(&self) -> Result<String>;

    // Check if a named session exists (lifecycle detection)
    pub fn session_exists(&self, session: &str) -> bool;

    // Build split-pane view of multiple sessions
    pub fn build_split_view(&self, sessions: &[&str]) -> Result<()>;
}
```

**Depends on:** `tmux_interface` crate. Nothing internal.

**Boundary rule:** No business logic here. No SQLite access. Pure tmux I/O. Command handlers call this; it does not call them.

---

### Module 5: `config` — YAML Config Parser

**Responsibility:** Parse `squad.yml` into typed Rust structs.

**Contains:**

```
src/config/
  mod.rs     — load_config(path) → SquadConfig
  types.rs   — SquadConfig, AgentConfig structs
```

**squad.yml structure:**

```yaml
project: my-project
agents:
  - name: my-project-claude-backend
    provider: claude
    role: backend
    session: my-project-claude-backend
  - name: my-project-gemini-frontend
    provider: gemini
    role: frontend
    session: my-project-gemini-frontend
```

**Depends on:** `serde`, `serde_yaml`. Nothing internal.

**Boundary rule:** Config parsing only. No DB writes here — `commands::init` reads config then calls `db::agent_repo` to persist.

---

### Module 6: `tui` — Terminal UI (Optional Invocation)

**Responsibility:** `squad-station ui` — live dashboard showing agent states. Blocking event loop. Reads DB every N ms. Does NOT write to DB.

**Contains:**

```
src/tui/
  mod.rs       — App struct, run() event loop
  state.rs     — TuiState (agents list, selected index, refresh interval)
  components/
    agent_table.rs  — Agent status table widget
    header.rs       — Title bar
    footer.rs       — Key bindings help
  events.rs    — Keyboard/tick event handling
```

**Architecture:** Component pattern (ratatui docs recommendation). Each component implements:
- `render(frame, area)` — draws itself into ratatui Frame
- owns its own display state (not application state)

**Event loop:**

```
Tick (every 500ms) → query DB → update TuiState → re-render all components
KeyEvent          → handle navigation / quit
```

**Depends on:** `ratatui`, `crossterm`, `db`. Does NOT depend on `tmux` or `commands`.

**Boundary rule:** TUI is read-only with respect to data. It calls `db::agent_repo::list_agents()` to refresh. It never writes to SQLite or invokes tmux. This keeps the dashboard safely isolated from mutation.

---

### Module 7: `orchestrator` — Context File Generator

**Responsibility:** Write the orchestrator context file that the AI Orchestrator reads to know about available agents.

**Contains:**

```
src/orchestrator/
  mod.rs         — generate_context(agents: &[Agent]) → String
  template.rs    — Markdown/text template for context file
```

**Output format example:**

```markdown
# Squad Station Context
## Available Agents
- **my-project-claude-backend** (claude) — status: idle
  Session: my-project-claude-backend
- **my-project-gemini-frontend** (gemini) — status: busy
  Session: my-project-gemini-frontend

## Commands
Send task: squad-station send --agent <name> --message "<task>"
Check status: squad-station status
```

**Depends on:** `db::models` (Agent type). Nothing else.

---

## Data Flow

### Flow 1: `squad-station send --agent worker-1 --message "fix the bug"`

```
argv
  └─> cli::parse()                    # clap parses args, builds Context
        └─> commands::send::run()     # handler
              ├─> db::agent_repo::find_by_name("worker-1")
              │     └─> rusqlite SELECT → Agent { session: "worker-1", status: "idle", ... }
              ├─> tmux::TmuxAdapter::send_message("worker-1", "fix the bug")
              │     └─> tmux_interface::SendKeys → shell: `tmux send-keys -t worker-1 "..." Enter`
              ├─> db::agent_repo::update_status("worker-1", "busy")
              │     └─> rusqlite UPDATE
              └─> db::message_log::insert(...)
                    └─> rusqlite INSERT
```

Exit code 0. Total duration: ~50ms (DB + tmux shell-out).

---

### Flow 2: `squad-station signal` (called from AI tool hook)

```
argv (hook environment — inside tmux session "worker-1")
  └─> cli::parse()                           # parse signal subcommand
        └─> commands::signal::run()
              ├─> tmux::TmuxAdapter::current_session()
              │     └─> tmux_interface: `tmux display-message -p '#S'` → "worker-1"
              ├─> db::agent_repo::find_by_name("worker-1")
              │     └─> rusqlite SELECT → Agent
              ├─> db::agent_repo::update_status("worker-1", "idle")
              │     └─> rusqlite UPDATE
              └─> [optional] orchestrator::generate_context() → write file
                    └─> db::agent_repo::list_agents() → all agents
```

Exit code 0. Orchestrator reads the context file on its next iteration.

---

### Flow 3: `squad-station ui`

```
argv
  └─> cli::parse()
        └─> commands::ui::run()
              └─> tui::run(ctx)                    # BLOCKING — event loop starts here
                    │
                    ├── Tick event (every 500ms):
                    │     └─> db::agent_repo::list_agents() → Vec<Agent>
                    │           └─> tui::state::update(agents)
                    │                 └─> ratatui::render(all components)
                    │
                    └── KeyEvent:
                          ├─ 'q' → break loop, exit
                          ├─ arrows → update selected row in TuiState
                          └─ (future) 'r' → force refresh
```

---

### Flow 4: `squad-station init` (from squad.yml)

```
argv
  └─> cli::parse()
        └─> commands::init::run()
              ├─> config::load_config("squad.yml") → SquadConfig
              ├─> db::open_db(ctx.db_path)           # creates DB + runs migrations if new
              └─> for each agent in SquadConfig:
                    db::agent_repo::upsert(agent)     # INSERT OR REPLACE
```

---

## Suggested Build Order

Components are ordered by dependency depth — build what has no internal dependencies first.

### Phase 1: Foundation (no internal deps)

1. **`config` module** — parse squad.yml. Pure serde, zero internal deps. Validates the config shape early.
2. **`db` module** — schema + migrations + typed repos. Depends only on rusqlite. This is the core state store; everything else needs it.

**Milestone:** `cargo test` passes for config parsing and DB open/migrate/CRUD.

---

### Phase 2: External Adapters

3. **`tmux` module** — TmuxAdapter wrapping tmux_interface. Depends only on the external crate. Unit-testable by mocking or integration-testing against real tmux.
4. **`orchestrator` module** — context file generator. Depends only on `db::models`. Pure text formatting.

**Milestone:** Can send a message to a real tmux session. Can generate a context file from mock agents.

---

### Phase 3: CLI + Commands

5. **`cli` module** — clap struct, Context resolution, project path logic.
6. **`commands` module** — one file per subcommand, wiring together db + tmux + orchestrator.

**Milestone:** `squad-station init`, `send`, `signal`, `register`, `status` work end-to-end.

---

### Phase 4: Views + TUI

7. **`tmux::view`** — split-pane view builder (part of tmux module, but last since it's purely cosmetic).
8. **`tui` module** — ratatui event loop, read-only dashboard.

**Milestone:** `squad-station view` and `squad-station ui` work.

---

### Phase 5: Distribution

9. **npm wrapper** — separate package, downloads platform binary. Not part of Rust codebase.
10. **CI cross-compile** — `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`.

---

## Key Patterns

### Pattern 1: Context Struct (not global state)

Every command handler receives a `Context` struct built by `cli::parse()`. This struct contains `db_path`, `project`, and `config_path`. No global mutable state, no environment variable reads scattered through the codebase.

```rust
pub fn run(ctx: &Context, agent: &str, message: &str) -> Result<()> {
    let conn = db::open_db(&ctx.db_path)?;
    // ...
}
```

**Why:** Stateless commands need deterministic inputs. Context struct makes every handler independently testable without environment setup.

---

### Pattern 2: Repository Trait Separation

`db::agent_repo` is a module of functions that take `&Connection` — not a struct with a connection stored inside. This means the command handler owns the connection lifetime.

```rust
// handler controls the transaction boundary
let mut conn = db::open_db(&ctx.db_path)?;
let tx = conn.transaction()?;
agent_repo::update_status(&tx, agent_name, "busy")?;
message_log::insert(&tx, agent_name, Direction::Sent, message)?;
tx.commit()?;
```

**Why:** Allows atomic multi-table writes without threading or async complexity. Stateless CLI fits synchronous SQLite perfectly.

---

### Pattern 3: Thin Tmux Adapter (no business logic)

`TmuxAdapter` methods are one-to-one with tmux commands. The adapter does not decide which session to send to — the command handler decides. The adapter only executes.

**Why:** Keeps tmux operations mockable for tests. Keeps business logic in `commands/`, not scattered in the adapter.

---

### Pattern 4: Migration-on-Open

`db::open_db()` always calls `migrations.to_latest(&mut conn)` before returning. Callers never worry about schema version.

```rust
pub fn open_db(path: &Path) -> Result<Connection> {
    let mut conn = Connection::open(path)?;
    migrations::run(&mut conn)?;
    Ok(conn)
}
```

**Why:** Stateless CLI — there is no startup phase to run migrations separately. Every invocation might be the first on a new DB or an upgrade.

---

### Pattern 5: Error Handling Strategy

- `thiserror` for typed errors within `db` and `tmux` modules (callers can match on specific failure kinds).
- `anyhow` at the `commands` layer and in `main.rs` — errors propagate up and are displayed to users.
- Exit code non-zero on any error (standard CLI contract).

```rust
// db/agent_repo.rs
#[derive(thiserror::Error, Debug)]
pub enum AgentError {
    #[error("Agent '{0}' not found")]
    NotFound(String),
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
}

// commands/send.rs
pub fn run(ctx: &Context, agent: &str, message: &str) -> anyhow::Result<()> {
    let conn = db::open_db(&ctx.db_path)?;  // anyhow ? propagation
    let found = agent_repo::find_by_name(&conn, agent)?;
    // ...
}
```

---

### Pattern 6: Orchestrator Skip Guard

`commands/signal.rs` checks whether the current tmux session is the orchestrator session before updating state. If the agent name matches the orchestrator naming pattern, it exits silently. This prevents the Orchestrator's own AI tool hooks from triggering signal loops.

```rust
if tmux_adapter.current_session()?.starts_with("orchestrator") {
    // skip — this is the orchestrator, not an agent
    return Ok(());
}
```

**Why:** Orchestrator runs the same AI tool hooks (Claude Code / Gemini CLI) as agents. Without this guard, the Orchestrator's hook fires `signal` and corrupts its own state.

---

### Anti-Pattern: Avoid Daemon/Background Process

Do not introduce a background watcher, file-system watcher, or `tokio` async runtime. Every feature must be expressible as a stateless command + periodic polling (TUI) or hook-driven signal. Adding a daemon would break the "stateless single binary" constraint and introduce process management complexity.

**If a feature seems to require a daemon:** redesign as a hook-triggered command or as a TUI refresh poll.

---

## Scalability Considerations

| Concern | With 5 agents | With 50 agents | Notes |
|---------|---------------|----------------|-------|
| DB file size | <1MB | <10MB | SQLite easily handles this; no concern |
| Signal latency | ~50ms | ~50ms | Stateless per-invocation, does not grow with agent count |
| TUI refresh | Instant | Instant | One SELECT * FROM agents — O(N) rows, trivial |
| Tmux session limit | Fine | Fine | tmux handles hundreds of sessions; naming convention keeps them organized |
| Cross-project isolation | 1 DB per project | 1 DB per project | DB path includes project name; no cross-contamination |

Squad Station is not a distributed system. It is a personal developer tool. Scalability concerns are about developer comfort (many agents in the TUI) not throughput.

---

## Sources

- [ratatui Component Architecture](https://ratatui.rs/concepts/application-patterns/component-architecture/) — HIGH confidence
- [ratatui Elm Architecture](https://ratatui.rs/concepts/application-patterns/the-elm-architecture/) — HIGH confidence
- [tmux_interface crate docs](https://docs.rs/tmux_interface/latest/tmux_interface/) — HIGH confidence
- [rusqlite_migration crate](https://crates.io/crates/rusqlite_migration) — HIGH confidence
- [rusqlite docs.rs](https://docs.rs/rusqlite/) — HIGH confidence
- [CLI Structure in Rust — Kevin K's Blog](https://kbknapp.dev/cli-structure-01/) — MEDIUM confidence (blog, well-regarded author)
- [anyhow vs thiserror patterns](https://www.shakacode.com/blog/thiserror-anyhow-or-how-i-handle-errors-in-rust-apps/) — MEDIUM confidence (consistent with official docs guidance)
- [Rust CLI Patterns 2026](https://dasroot.net/posts/2026/02/rust-cli-patterns-clap-cargo-configuration/) — MEDIUM confidence (recent, 2026)
