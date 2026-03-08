# Squad Station — Documentation Index

> Source of truth for the Squad Station project.
> Read documents in the order listed below.

---

## Reading Order

| # | File | Purpose | Priority |
|---|------|---------|----------|
| 1 | [VISION.md](./VISION.md) | Vision, architecture overview, scope, problems to solve | **Start here** |
| 2 | [SOLUTION-DESIGN.md](./SOLUTION-DESIGN.md) | Detailed design: config format, data model, hooks, CLI, scenarios, naming conventions | Core reference |
| 3 | [TECH-STACK.md](./TECH-STACK.md) | Rust stack decisions, module structure, safety checklist, roadmap | Implementation guide |
| 4 | [GAP-ANALYSIS.md](./GAP-ANALYSIS.md) | 10 gaps between design specs and current codebase | **Action items** |
| 5 | [PLAYBOOK.md](./PLAYBOOK.md) | ⚠️ **STALE** — User-facing guide, needs rewrite after gaps are fixed | Do not trust |

## Document Hierarchy

```
VISION.md                    ← WHY: Vision, scope, problems
    └── SOLUTION-DESIGN.md   ← WHAT: Design specifications (source of truth)
        └── TECH-STACK.md    ← HOW: Implementation decisions
            └── GAP-ANALYSIS.md  ← DELTA: What needs to change
                └── PLAYBOOK.md  ← GUIDE: User manual (rewrite last)
```

## Source Origin

All documents are derived from the original Obsidian design notes:

| Doc | Obsidian Source |
|-----|-----------------|
| VISION.md | `01. Vision & Scope.md` |
| SOLUTION-DESIGN.md | `02. Solution Design - Squad Station.md` |
| TECH-STACK.md | `03. Tech Stack Decision - Squad Station.md` |
| GAP-ANALYSIS.md | Cross-reference analysis (Obsidian vs codebase) |

## Key Decisions (Quick Reference)

| Decision | Value | Source |
|----------|-------|--------|
| Language | **Rust** | TECH-STACK.md |
| DB Library | **sqlx** (async, compile-time SQL) | TECH-STACK.md |
| Architecture | **Stateless CLI** — no daemon | VISION.md |
| DB | **SQLite WAL** — 1 file per project | SOLUTION-DESIGN.md |
| Config | `squad.yml` with `project` (string), `model`, `description` fields | SOLUTION-DESIGN.md |
| Agent Naming | `<project>-<provider>-<role>` | SOLUTION-DESIGN.md |
| Hooks | **2 per provider**: Stop + Notification | SOLUTION-DESIGN.md |
| Messages Schema | **2-directional**: `from_agent` → `to_agent`, status: processing/completed/failed | SOLUTION-DESIGN.md |

---
*Last updated: 2026-03-08*
