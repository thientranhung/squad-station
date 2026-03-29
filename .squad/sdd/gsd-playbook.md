# GSD (Get Shit Done) — Agent Playbook

## How GSD Works

GSD is **phase-based** — work flows through sequential phases within milestones. Each phase: discuss → plan → execute → verify. GSD has its own auto-advance (`/gsd:next`) that determines the correct next step.

## Orchestrator Routing

GSD commands are slash commands that the agent executes. The orchestrator sends commands, not task descriptions:

- ✅ `"/gsd:next"` → agent auto-determines and runs next step
- ✅ `"/gsd:execute-phase 3"` → agent executes phase 3
- ❌ `"implement the login feature"` → GSD expects its own commands

### Task Classification

| Situation | Command to send |
|---|---|
| Starting brand new project | `/gsd:new-project` |
| Existing codebase, new milestone | `/gsd:map-codebase` → `/gsd:new-milestone <name>` |
| Small standalone fix/task | `/gsd:quick` (add `--discuss` or `--research` if needed) |
| Don't know what's next | `/gsd:next` (auto-advance) |
| Resuming from previous session | `/gsd:resume-work` |
| Bug investigation | `/gsd:debug "description"` |
| Check project status | `/gsd:progress` |

## Workflow Sequence

**Phase loop** (repeat for each phase N):
1. `/gsd:discuss-phase N` — lock in preferences (invest time here!)
2. `/gsd:plan-phase N` — research + plan + verify
3. `/gsd:execute-phase N` — parallel wave execution + regression gate
4. `/gsd:verify-work N` — manual UAT + auto-diagnosis
5. `/clear` — clean context before next phase

**After all phases:**
`/gsd:audit-milestone` → `/gsd:plan-milestone-gaps` (if gaps) → `/gsd:ship` → `/gsd:complete-milestone`

**UI phases** add extra steps: `/gsd:ui-phase N` (before plan) and `/gsd:ui-review N` (after verify).

## Document Discipline

GSD manages its own state in `.planning/`:
- Phase plans, research notes, verification results — all auto-generated
- `STATE.md` — current position (auto-updated by GSD)
- `HANDOFF.json` — session handoff state (via `/gsd:pause-work`)

Orchestrator must verify:
- After `/gsd:verify-work`: check if verification passed or has issues
- After `/gsd:audit-milestone`: check for gaps before completing
- After `/gsd:ship`: confirm PR was created

## Critical Rules

1. **`/clear` between phases** — clean context after every verify/review cycle.
2. **Invest time in discuss-phase** — preferences drive planning accuracy.
3. **Use `/gsd:next` when unsure** — it auto-determines the correct next step.
4. **Always audit before completing** — `audit-milestone` → `plan-milestone-gaps` → `complete-milestone`.
5. **Verify agent completion after SQUAD SIGNAL** — SQUAD SIGNAL means the agent processed your message, not that the workflow is complete. Check if the agent is still in discussion, waiting for answers, or mid-execution before marking done.
