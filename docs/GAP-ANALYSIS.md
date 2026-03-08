# Squad Station — Gap Analysis

> Comparison of Source of Truth docs (from Obsidian) vs current codebase.
> Goal: identify what needs to change in the codebase to comply with the original design.

---

## Summary

| Severity | Count | Description |
|----------|-------|-------------|
| 🔴 CRITICAL | 3 | Fix immediately — affects core design |
| 🟡 HIGH | 4 | Missing feature vs design |
| 🟢 MEDIUM | 3 | Docs/naming need update |

---

## 🔴 CRITICAL — Must fix for design compliance

### GAP-01: Config `squad.yml` — wrong structure

**Design (Obsidian):**
```yaml
project: myapp                     # ← string

orchestrator:
  provider: claude
  model: opus                      # ← REQUIRED
  description: >                   # ← REQUIRED
    Main orchestrator...

agents:
  - name: implement
    provider: claude
    model: sonnet                   # ← REQUIRED
    description: >                  # ← REQUIRED
      Developer agent...
```

**Current codebase (`config.rs`):**
```yaml
project:
  name: squad-station              # ← object instead of string
  db_path: ...

orchestrator:
  name: squad-orchestrator
  provider: claude-code
  role: orchestrator
  command: "claude"                # ← NO model, description
```

**Required changes:**
- [ ] `ProjectConfig` → change from `{name, db_path}` to string (or add backward compat)
- [ ] `AgentConfig` → add fields: `model`, `description`
- [ ] Field `command` → decide: keep (per code) or remove (per Obsidian)
- [ ] Update sample `squad.yml`

### GAP-02: DB Schema `messages` — missing 2-directional and processing status

**Design (Obsidian):**
```sql
messages:
  id          TEXT PK
  from_agent  TEXT           -- sender (orch → agent)
  to_agent    TEXT           -- recipient
  type        TEXT           -- task_request | task_completed | notify
  priority    TEXT
  status      TEXT           -- processing | completed | failed | aborted
  body        TEXT
  created_at  DATETIME
  completed_at DATETIME
```

**Current codebase:**
```sql
messages:
  id          TEXT PK
  agent_name  TEXT           -- single direction only
  task        TEXT
  status      TEXT           -- pending | completed (missing processing)
  priority    TEXT
  created_at  TEXT
  updated_at  TEXT
```

**Required changes:**
- [ ] Add `from_agent`, `to_agent` instead of single `agent_name`
- [ ] Add `type` column (task_request, task_completed, notify)
- [ ] Change status `pending` → `processing` (per Obsidian lifecycle)
- [ ] Add `completed_at` instead of just `updated_at`
- [ ] New migration file

### GAP-03: DB Schema `agents` — missing critical fields

**Design (Obsidian):**
```sql
agents:
  name            TEXT PK
  role            TEXT
  tool            TEXT           -- claude-code, gemini-cli
  model           TEXT           -- sonnet, opus, gemini-2.5-pro
  description     TEXT           -- from squad.yml
  status          TEXT           -- idle | busy | dead
  current_task    TEXT FK        -- → messages.id
  last_heartbeat  DATETIME
```

**Current codebase:**
```sql
agents:
  id              TEXT PK
  name            TEXT UNIQUE
  provider        TEXT           -- ≈ tool
  role            TEXT
  command         TEXT           -- not in Obsidian design
  created_at      TEXT
  status          TEXT
  status_updated_at TEXT
```

**Required changes:**
- [ ] Add `model` column
- [ ] Add `description` column
- [ ] Add `current_task` FK → messages.id
- [ ] Rename `provider` → `tool` (or keep `provider` but document decision)
- [ ] New migration file

---

## 🟡 HIGH — Missing features

### GAP-04: Notification hook not implemented

**Design:** 2 hooks per provider (Stop + Notification)
**Codebase:** Only Stop hook exists

**Required:**
- [ ] Create `hooks/claude-code-notify.sh` for Notification event
- [ ] Create `hooks/gemini-cli-notify.sh`
- [ ] Document how to register notification hook

### GAP-05: CLI `send` — uses positional arg instead of `--body` flag

**Design:**
```bash
squad-station send <agent> --body "task..."
```

**Codebase:**
```bash
squad-station send <agent> "task..."    # positional arg
```

**Decision needed:** Keep positional (current) or change to --body (per Obsidian)?

### GAP-06: Agent naming convention not enforced

**Design:** `<project>-<provider>-<role>` (auto-prefix from project name)
**Codebase:** User sets agent name freely

**Required:**
- [ ] `init` command auto-prefixes agent name with `<project>-<provider>-`
- [ ] Or validate naming convention in config

### GAP-07: `context` command lacks `description` and `model`

**Design:** Context file displays model + description for each agent
**Codebase:** Context only shows name, role, status (DB lacks model + description)

**Required:** Fix after GAP-03 is complete (add model + description to DB)

---

## 🟢 MEDIUM — Docs / Naming

### GAP-08: `.planning/research/ARCHITECTURE.md` stale

- Still references `rusqlite` instead of `sqlx`
- CLI syntax wrong (`--agent --message` instead of positional)
- Module structure wrong (nested folders instead of flat files)

**Required:** Update ARCHITECTURE.md to match reality

### GAP-09: `PLAYBOOK.md` has many incorrect details

**Required:** Rewrite PLAYBOOK.md after fixing above GAPs.

### GAP-10: Signal notification format

**Design:** `"<agent> completed <msg-id>"`
**Codebase:** `"[SIGNAL] agent=X status=completed task_id=Y"`

**Decision needed:** Which format to use officially?

---

## Implementation Priority

```
Phase 1 (DB + Config refactor):
  GAP-01 → Config format
  GAP-02 → Messages schema
  GAP-03 → Agents schema

Phase 2 (Feature completion):
  GAP-04 → Notification hooks
  GAP-05 → CLI send syntax
  GAP-06 → Naming convention
  GAP-07 → Context with model/description

Phase 3 (Documentation):
  GAP-08 → Update .planning/
  GAP-09 → Rewrite PLAYBOOK.md
  GAP-10 → Finalize signal format
```

---

## Owner Decisions Needed

| # | Question | Option A (per Obsidian) | Option B (per current code) |
|---|---------|--------------------------|-------------------------------|
| 1 | `project` config | string `project: myapp` | object `project: {name, db_path}` |
| 2 | Field `command` | Not present (provider infers) | Present (explicit launch command) |
| 3 | CLI `send` syntax | `--body "..."` flag | positional arg `"..."` |
| 4 | Signal format | `"<agent> completed <id>"` | `"[SIGNAL] agent=X..."` |
| 5 | `provider` vs `tool` | `tool` | `provider` |

---
*Generated: 2026-03-08*
*Based on: docs/VISION.md, docs/SOLUTION-DESIGN.md, docs/TECH-STACK.md vs codebase*
