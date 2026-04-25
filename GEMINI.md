# Squad Station - Project Context

## Project Overview
**Squad Station** is a provider-agnostic, stateless Rust CLI application designed for message routing and orchestration among multiple AI agents (e.g., Claude Code, Gemini CLI). It coordinates a "squad" of AI agents using a central orchestrator, local `tmux` sessions for execution, and a local SQLite database (`.squad/station.db`) for tracking state, tasks, and messages.

It allows users to plug in structured development methodologies (SDD) like "Get Shit Done", "BMad Method", or "Superpowers" as playbooks. The tool automatically hooks into agent completion signals and manages session lifecycles using a watchdog monitor. 

The project contains a core Rust application (`src/`) and is also distributed via an npm wrapper (`package.json`, `bin/run.js`) for easy installation.

## Key Technologies
- **Rust**: Core CLI logic, using `tokio` for async operations, `sqlx` for SQLite database interactions, and `clap` for command-line argument parsing.
- **SQLite**: Stores project state locally in `.squad/station.db` using WAL mode.
- **tmux**: Manages individual terminal sessions for the orchestrator and each worker agent.
- **Node.js/npm**: Used for distribution and bootstrapping the native binary installation (`npx squad-station`).

## Building and Running

### Rust Core
To build the Rust binary from source:
```bash
cargo build --release
```
The compiled binary will be located at `target/release/squad-station`.

To run the application directly using Cargo:
```bash
cargo run -- <command>
# e.g., cargo run -- status
```

### Installation (NPM Wrapper)
To install using the npm wrapper locally (useful for testing the distribution):
```bash
npm install -g .
```

### Testing
The project uses standard Rust testing practices along with shell scripts for end-to-end testing.
To run the test suite:
```bash
cargo test
```
End-to-end tests are located in the `tests/` directory (e.g., `tests/e2e_cli.sh`, `tests/e2e_workflow.sh`).

## Development Conventions
- **Stateless Design**: The CLI itself is stateless; every invocation connects to the SQLite database, performs its read/write operation, and exits. There is no long-running daemon (except for the optional watchdog).
- **Database Access**: Uses `sqlx` for SQLite interactions. Keep database queries efficient as they are executed on every command.
- **Playbooks & Config**: Agent orchestration is driven by a `squad.yml` configuration file and Markdown/YAML playbooks located in the `.squad/` directory. When making structural changes, ensure compatibility with these configuration formats.
- **Signal Handling**: The system relies on provider-specific hooks (like post-tool use in Claude or Gemini CLI) that call `squad-station signal` to indicate task completion. Any changes to command interfaces must consider backward compatibility with these hooks.
- **Error Handling**: Use `anyhow` for flexible and consistent error propagation throughout the codebase.
