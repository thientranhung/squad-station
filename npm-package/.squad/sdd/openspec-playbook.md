# OpenSpec — Agent Playbook

## How It Works

OpenSpec is **spec-driven** — every feature goes through a structured change workflow: propose → apply → archive. Changes live in `openspec/changes/<name>/` with planning artifacts (proposal, specs, design, tasks).

**Two profiles:**
- **Core** (default): `propose` → `apply` → `archive` (fast, 3 commands)
- **Expanded** (custom): Full granular control with explore, new, continue, ff, verify

## Workflow Sequence

**Quick path (core profile):**
1. `/opsx:propose <feature-name>` — Create change + ALL planning artifacts (proposal, specs, design, tasks)
2. `/opsx:apply` — Implement tasks from the plan
3. `/opsx:archive` — Archive completed change, merge specs

**Complex feature (expanded profile):**
1. `/opsx:explore [topic]` — Explore problem space (no artifacts created)
2. `/opsx:new <feature-name>` — Create change scaffold
3. `/opsx:continue [name]` — Create next artifact, review each step (repeat)
4. `/opsx:apply [name]` — Implement tasks
5. `/opsx:verify [name]` — Validate implementation vs specs
6. `/opsx:archive [name]` — Archive when done

**Fast-forward variant:** Replace step 3 with `/opsx:ff [name]` to generate ALL artifacts at once.

**Parallel changes:** Start a new change anytime with `/opsx:new <name>`. Resume previous with `/opsx:apply <name>`.

## Command Reference

### Slash Commands

| Command | Profile | Description |
|---|---|---|
| `/opsx:propose <name>` | core | Create change + ALL planning artifacts |
| `/opsx:explore [topic]` | core | Explore ideas, no artifacts created |
| `/opsx:apply [name]` | core | Implement tasks from plan |
| `/opsx:archive [name]` | core | Archive change, merge specs |
| `/opsx:new <name>` | expanded | Create change scaffold only |
| `/opsx:continue [name]` | expanded | Create next artifact, review each step |
| `/opsx:ff [name]` | expanded | Fast-forward: create ALL remaining artifacts |
| `/opsx:verify [name]` | expanded | Validate implementation vs specs |
| `/opsx:sync [name]` | expanded | Merge delta specs (usually automatic) |
| `/opsx:bulk-archive` | expanded | Archive multiple changes at once |
| `/opsx:onboard` | expanded | Guided tutorial |

### CLI Commands

| Command | Description |
|---|---|
| `openspec list` | List active changes |
| `openspec view <change>` | Show change details |
| `openspec show <path>` | Show file content |
| `openspec validate` | Validate artifacts |
| `openspec archive` | Archive a change |
| `openspec status --change <name>` | Query change state |

### Command Syntax per AI Tool

| Tool | Format | Example |
|---|---|---|
| Claude Code | `/opsx:command` | `/opsx:propose add-auth` |
| Cursor | `/opsx-command` | `/opsx-propose add-auth` |
| Windsurf | `/opsx-command` | `/opsx-propose add-auth` |
| Copilot (IDE) | `/opsx-command` | `/opsx-propose add-auth` |

## Critical Rules

1. **One change = one unit of work** — Never mix unrelated features in a single change.
2. **Name changes clearly** — Use descriptive kebab-case: `add-dark-mode`, `fix-login-redirect`. Never `update` or `wip`.
3. **Explore before committing** — Use `/opsx:explore` when requirements are unclear.
4. **Verify before archiving** — `/opsx:verify` catches spec-implementation mismatches early.
5. **Specs describe behavior, not implementation** — No class names or framework choices in specs.
