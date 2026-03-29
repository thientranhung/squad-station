# Superpowers — Agent Playbook

## How Superpowers Works

Superpowers is **intent-driven** — skills auto-trigger via the 1% rule. The agent receiving the task decides which skills to invoke. The orchestrator does NOT select skills — it delegates the right TYPE of task to the right agent.

## Orchestrator Routing

Route tasks by intent, not by skill name. The agent handles skill selection:

| Task intent | Route to | Agent will auto-trigger |
|---|---|---|
| Build new feature / add functionality | **brainstorm agent** | `brainstorming` → `writing-plans` → `SDD` |
| Fix bug / unexpected behavior | **implement agent** | `systematic-debugging` → `TDD` fix |
| Code review / quality check | **brainstorm agent** | `requesting-code-review` |
| Implement from existing spec/plan | **implement agent** | `SDD` or `executing-plans` |
| Research / analysis / design only | **brainstorm agent** | `brainstorming` (ends at spec, no implementation) |

### What to send

Send **intent**, not skill commands. The agent's 1% rule handles the rest:

- ✅ `"Add user authentication to the API"` → agent triggers brainstorming → planning → SDD
- ✅ `"Fix: login returns 500 when password is empty"` → agent triggers systematic-debugging
- ❌ `"Use brainstorming skill to design auth"` → don't micromanage skill selection

## Workflow Sequences

**Feature (full flow):**
brainstorming → spec review (max 3 iterations) → user review gate → git worktree → writing-plans → plan review → SDD/inline execution → finishing-a-development-branch

**Bug fix:**
systematic-debugging (4 phases: root cause → pattern → hypothesis → TDD fix) → verification-before-completion

**Execution modes** (agent asks user to choose):
- **SDD** (recommended) — fresh subagent per task + two-stage review (spec compliance → code quality)
- **Inline** — sequential in same session, for platforms without subagent support

## Document Discipline

After task completes, verify:
- **Feature built** → spec saved to `docs/plans/`, plan saved, code committed per task
- **Bug fixed** → root cause documented, failing test added BEFORE fix, fix committed
- **Branch finished** → agent presents 4 options: merge / PR / keep / discard — verify user chose

Superpowers auto-commits per task. But orchestrator must verify:
- Plan document exists in `docs/plans/` after brainstorming + planning
- Spec document exists after design approval
- Branch is not left orphaned after completion

## Critical Rules

1. **Skills are for the agent, not the orchestrator** — you send intent, agent triggers skills. Don't send skill names as commands.
2. **Brainstorming is always first** — even "simple" features. The agent enforces this (iron law: NO code before design approved).
3. **TDD is mandatory** — NO production code without failing test first. The agent enforces this.
4. **Verify before claiming done** — agent must run command, read output, THEN claim. If agent says "should work" without evidence, send back.
5. **3 review iterations max** — if still unresolved after 3 rounds, escalate to user (not infinite loop).
6. **Verify agent completion after SQUAD SIGNAL** — SQUAD SIGNAL means the agent processed your message, not that the workflow is complete. Check if the agent is still in brainstorming questions, review loops, or waiting for approval before marking done.
