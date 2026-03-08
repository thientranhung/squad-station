# Squad Station

## What This Is

Squad Station là một stateless CLI binary (Rust + embedded SQLite) hoạt động như trạm trung chuyển messages giữa AI Orchestrator và N agents chạy trong tmux sessions. Provider-agnostic — hỗ trợ bất kỳ AI coding tool nào (Claude Code, Gemini CLI, Codex, Aider...). Người dùng chỉ tương tác với Orchestrator, Station lo việc routing messages, tracking trạng thái agent, và cung cấp fleet monitoring qua TUI dashboard và tmux views.

## Core Value

Routing messages đáng tin cậy giữa Orchestrator và agents — gửi task đúng agent, nhận signal khi hoàn thành, notify Orchestrator — tất cả qua stateless CLI commands không cần daemon.

## Requirements

### Validated

- ✓ Orchestrator gửi task đến agent qua `squad-station send` — v1.0
- ✓ Hook-driven signal khi agent hoàn thành (`squad-station signal`) — v1.0
- ✓ Agent registry từ `squad.yml` config (`squad-station init`) — v1.0
- ✓ Dynamic agent registration at runtime (`squad-station register`) — v1.0
- ✓ Multi-project isolation (DB riêng per project) — v1.0
- ✓ Orchestrator skip trong hook (chống infinite loop) — v1.0
- ✓ Agent lifecycle detection (idle/busy/dead) — v1.0
- ✓ Auto-generate orchestrator context file — v1.0
- ✓ TUI dashboard (`squad-station ui`) — v1.0
- ✓ Split tmux view (`squad-station view`) — v1.0
- ✓ Idempotent send/signal (duplicate hook fires safe) — v1.0
- ✓ Message priority levels (normal, high, urgent) — v1.0
- ✓ Peek for pending tasks (`squad-station peek`) — v1.0
- ✓ SQLite WAL mode with busy_timeout (concurrent-safe) — v1.0
- ✓ tmux send-keys literal mode (injection-safe) — v1.0
- ✓ Shell readiness check before prompt injection — v1.0
- ✓ SIGPIPE handler at binary startup — v1.0
- ✓ 4-layer guard on signal command — v1.0
- ✓ Text status overview (`squad-station status`) — v1.0
- ✓ Agent list with status (`squad-station agents`) — v1.0
- ✓ Provider hook scripts (Claude Code + Gemini CLI) — v1.0
- ✓ Message list with filters (`squad-station list`) — v1.0
- ✓ squad.yml config: `project` string, `model`/`description`, removed `command`, `provider`→`tool` — v1.1
- ✓ Messages DB schema: `from_agent`/`to_agent`, `type`, `processing` status, `completed_at` — v1.1
- ✓ Agents DB schema: `model`, `description`, `current_task` FK, `tool` field — v1.1
- ✓ Notification hooks: `claude-code-notify.sh` + `gemini-cli-notify.sh` — v1.1
- ✓ CLI `send --body` flag (positional arg removed) — v1.1
- ✓ Agent naming auto-prefix `<project>-<tool>-<role>` on init — v1.1
- ✓ `context` output includes `model` + `description` per agent — v1.1
- ✓ Signal format standardized to `"<agent> completed <msg-id>"` — v1.1
- ✓ ARCHITECTURE.md updated to reflect actual sqlx + flat module structure — v1.1
- ✓ PLAYBOOK.md rewritten with correct CLI syntax and config format — v1.1
- ✓ GitHub Actions CI/CD cross-compiles Rust binary for 4 targets (darwin-arm64, darwin-x86_64, linux-arm64, linux-x86_64) and creates GitHub Release — v1.2
- ✓ npm package detects platform and downloads correct binary on postinstall — v1.2
- ✓ curl | sh install script as npm-free alternative to install binary — v1.2
- ✓ README.md documents all installation methods with usage quickstart — v1.2

### Active

<!-- v1.3 or next milestone — add requirements here -->

(None yet — define with /gsd:new-milestone)

### Out of Scope

- Task management / workflow logic — đó là việc của Orchestrator AI
- Orchestration decisions / reasoning — đó là việc của AI model
- File sync / code sharing giữa agents — agents work on same codebase via git
- Web UI / browser dashboard — TUI sufficient, complexity not justified
- Git conflict resolution giữa agents — orchestrator should sequence work to avoid
- Agent-to-agent direct messaging — all communication routes through orchestrator
- Offline mode — stateless CLI always needs tmux + DB

## Context

Shipped v1.2 Distribution with 4,367 LOC Rust, 24 files changed (+2,955 lines in v1.2).
Tech stack: Rust, SQLite (sqlx 0.8), clap 4, ratatui 0.26, serde-saphyr, owo-colors 3.
Distribution: npm package + curl | sh installer, both download pre-built binaries from GitHub Releases.
CI/CD: GitHub Actions matrix workflow produces 4 musl/darwin binaries on v* tag push.
Binary format: fully static musl binaries for Linux (no glibc), native darwin binaries.
npm postinstall: zero-dependency JS downloader with platform/arch detection and 301/302 redirect following.
Install methods: `npm install -g squad-station` or `curl -fsSL <url> | sh` or build from source.

## Constraints

- **Language**: Rust — single binary, zero runtime dependency, cross-compile cho darwin/linux
- **Database**: SQLite embedded — 1 DB file per project tại `~/.agentic-squad/<project>/station.db`
- **Architecture**: Stateless CLI — mỗi command chạy xong exit, không daemon, không background process
- **Communication**: tmux send-keys để inject prompt vào agent, tmux capture-pane để đọc output
- **Distribution**: npm package wrapper — download pre-built binary phù hợp platform
- **Repo**: Dedicated repo riêng cho Rust binary (repo hiện tại: squad-station)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Rust thay vì Go | Binary nhỏ hơn, performance tốt hơn, user preference | ✓ Good — 2,994 LOC, fast compile, single binary |
| Stateless CLI, không daemon | Đơn giản, dễ debug, event-driven qua hook chain | ✓ Good — no process management complexity |
| SQLite embedded per project | Isolation giữa projects, không cần external DB | ✓ Good — WAL mode handles concurrent writes |
| Agent name = tmux session name | Đơn giản hóa lookup, hook tự detect qua TMUX_PANE | ✓ Good — zero-config agent identity |
| npm wrapper distribution | Target audience là developers đã có Node.js | ✓ Good — npm + curl | sh both shipped v1.2 |
| Provider-agnostic design | Không lock-in vào Claude Code hay Gemini CLI | ✓ Good — hooks work for both providers |
| Hook-driven completion | Agent passive, không cần modify agent behavior | ✓ Good — clean separation of concerns |
| sqlx over rusqlite | Already in Cargo.toml, async-native, compile-time SQL checks | ✓ Good — migration system worked well |
| max_connections(1) write pool | Prevents async write-contention deadlock in SQLite | ✓ Good — no busy errors in testing |
| INSERT OR IGNORE for agents | Idempotent registration, safe for duplicate hook fires | ✓ Good — MSG-03 satisfied cleanly |
| connect-per-refresh in TUI | Prevents WAL checkpoint starvation during long TUI sessions | ✓ Good — WAL doesn't grow unbounded |
| Reconciliation loop duplication | Each command file independent, ~10 lines not worth abstraction | ✓ Good — simple, no coupling |
| `--body` flag for `send` | Named flags more discoverable and shell-safe than positional args | ✓ Good — cleaner UX, pattern-matchable |
| Auto-prefix agent naming | Enforces `<project>-<tool>-<role>` convention without manual coordination | ✓ Good — avoids name collisions across projects |
| `provider`→`tool` rename | Matches solution design terminology; aligns with squad.yml and DB | ✓ Good — consistent naming across all layers |
| Notification hooks separate from Stop hooks | Notification fires on permission prompts, not task completion — distinct behavior | ✓ Good — both hook types needed |
| Signal format `"<agent> completed <msg-id>"` | Pattern-matchable string, no JSON parsing needed in orchestrator | ✓ Good — simple, grep-friendly |
| SQUAD_STATION_DB env var in resolve_db_path | Single injection point benefits all commands without per-command changes | ✓ Good — cleaner test isolation |
| musl over gnu for Linux targets | Produces fully static binaries, no glibc dependency — required for install script portability | ✓ Good — runs on any Linux distro |
| cross tool only for linux-arm64 | aarch64-musl requires cross-compilation; native cargo sufficient for darwin and linux-x86_64 | ✓ Good — minimal Docker overhead |
| softprops/action-gh-release@v2 | Idempotent — creates release if absent, appends assets if present; safe for 4 parallel matrix uploads | ✓ Good — race-condition-free releases |
| curl | sh as npm alternative | Targets users without Node.js; POSIX sh for max portability | ✓ Good — covers non-Node environments |
| Binary naming `squad-station-{os}-{arch}` | Consistent convention consumed by npm postinstall and install script | ✓ Good — both distribution paths aligned |

---
*Last updated: 2026-03-09 after v1.2 milestone*
