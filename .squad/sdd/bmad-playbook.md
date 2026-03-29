# BMad Method — Agent Playbook

## Workflow Selection

Classify every incoming task BEFORE delegating. The agent — not bmad-help — decides the workflow.

| Task type | Workflow | Agent code |
|---|---|---|
| Bug fix, refactor, quick change, one-off task | **QD** (Quick Dev) | Barry 🚀 |
| Bug/task that needs a written spec first | **QS** → **QD** (Quick Spec then Quick Dev) | Barry 🚀 |
| Planned story from sprint backlog | **CS → DS → CR** (Create Story → Dev Story → Code Review) | Bob 🏃 → Amelia 💻 |
| New feature, no plan exists yet | Start from **Phase 2** or earlier — see Phase Sequence below | John 📋 |
| Scope changed, existing plan is now wrong | **CC** (Correct Course) → update epics → resume story cycle | Bob 🏃 |
| Need to understand project position | **BH** (bmad-help) — GPS only, shows where you are | — |
| Documentation needed | **WD / VD / MG / EC** (Tech Writer) | Paige 📚 |
| Review code or artifacts | **CR / AR / ECH** (Code Review / Adversarial / Edge Case) | Amelia 💻 |

### Decision shortcut

```
Does this task change existing PRD, Architecture, or Epics?
  YES → CC (Correct Course) first, then resume story cycle
  NO  → Is there a story in the sprint backlog for this?
          YES → CS → DS → CR (story cycle)
          NO  → QD (Quick Dev) — or QS → QD if spec needed
```

## Phase Sequence (for planned work)

When building from scratch or adding major features, follow phases in order.
Each workflow runs in a **fresh chat**. Never chain workflows.

**Phase 1 — Analysis** (optional):
`BP` Brainstorm → `MR/DR/TR` Research → `CB` Product Brief

**Phase 2 — Planning** (required):
`CP` Create PRD → `CU` Create UX (if UI needed)

**Phase 3 — Solutioning** (required for complex projects):
`CA` Create Architecture → `CE` Create Epics & Stories → `IR` Implementation Readiness check

**Phase 4 — Implementation**:
`SP` Sprint Planning (once) → **story cycle**: `CS → DS → CR` (repeat) → `ER` Retrospective (per epic)

## Document Discipline

A task is NOT done until its artifacts are updated:

- **After story**: sprint-status.yaml status → `done`
- **After Quick Dev**: log change in CHANGELOG.md or docs/ (Quick Dev does NOT update sprint tracking)
- **After Correct Course**: PRD/Architecture/Epics are now updated — verify consistency
- **After epic complete**: run Retrospective (`ER`)
- **After any architecture change**: update relevant docs

## Critical Rules

1. **Fresh chat per workflow** — never chain workflows in one session.
2. **Code review every story** — CR after every DS, no exceptions.
3. **Quick Dev lives outside sprint** — it does not create stories or update sprint-status.yaml. Bridge manually if tracking matters.
4. **bmad-help is GPS, not autopilot** — use it to check project position, not to select workflows. YOU select the workflow.
5. **Verify agent completion after SQUAD SIGNAL** — SQUAD SIGNAL means the agent processed your message, not that the workflow is complete. Check if the agent is still waiting (questions, menus, confirmations) before marking done.
