# Superpowers — Agent Playbook

## How It Works

Superpowers is **intent-driven** — describe what you want and the agent auto-triggers the correct skill. No explicit commands needed.

**Build a feature:**
1. Describe the feature → agent auto-triggers `brainstorming`
2. Answer questions (1 at a time, usually MCQ) → agent proposes 2-3 approaches
3. Approve design sections → agent writes spec to `docs/superpowers/specs/`
4. Approve spec (user review gate) → agent transitions to `writing-plans`
5. Agent creates plan with bite-sized tasks → approve plan
6. Choose execution mode: **SDD** (subagents, recommended) or **Inline** (sequential)
7. Agent executes: implement → spec review → quality review per task
8. Choose: Merge / Create PR / Keep branch / Discard

**Fix a bug:** Describe the bug → agent auto-triggers `systematic-debugging` → 4-phase investigation → TDD fix

**Brainstorm only:** Say "let's brainstorm about [topic]" → ends with spec, no auto-implementation

## Skills Reference

| Skill | Triggers When | Iron Law |
|---|---|---|
| `brainstorming` | Any creative work or new feature | NO code before design approved |
| `writing-plans` | Spec/requirements exist for multi-step task | Bite-sized tasks (2-5 min each) |
| `subagent-driven-development` | Plan exists + user chose SDD | Fresh subagent per task + two-stage review |
| `executing-plans` | Plan exists + user chose inline / no subagent platform | Execute continuously, stop only on blocker |
| `test-driven-development` | All implementation | NO production code without failing test first |
| `systematic-debugging` | All technical issues | NO fixes without root cause investigation |
| `verification-before-completion` | Before claiming done/fixed/passing | NO claims without fresh verification evidence |
| `using-git-worktrees` | Before starting implementation | Isolated workspace, verify clean baseline |
| `finishing-a-development-branch` | Implementation done, tests pass | Verify tests → present 4 options → execute |
| `requesting-code-review` | After each task (SDD) or before merge | Dispatch code-reviewer subagent |
| `receiving-code-review` | Receiving feedback from reviewer | Fix Critical immediately, Important before continuing |
| `dispatching-parallel-agents` | Multiple independent domains | Identify → Create → Dispatch → Integrate |
| `writing-skills` | Creating a new skill | TDD for documentation |
| `using-superpowers` | Every conversation | 1% rule: if a skill COULD apply → MUST invoke |

## Execution Modes

| Mode | When to Use | Platform |
|---|---|---|
| **SDD** (recommended) | Full autonomous with review | Claude Code, Codex (subagent support) |
| **Inline** | Manual control, step-by-step | Any platform |
| **Fallback** | Platform lacks subagents | Gemini CLI, OpenCode |

**SDD per-task flow:** Dispatch implementer → handle status (DONE/BLOCKED/NEEDS_CONTEXT) → spec reviewer → quality reviewer → mark complete

**Review loop:** Reviewer gets complete document → only flag real implementation problems → max 3 iterations → escalate to human if unresolved

## Critical Rules

1. **Always brainstorm first** — Even for "simple" projects. No code before design is approved.
2. **No code before tests** — RED (failing test) → GREEN (minimal code) → REFACTOR.
3. **Verify before claiming** — Run the command, read the output, THEN claim done.
4. **Spec compliance before quality** — Two-stage review: spec review FIRST, code quality SECOND.
5. **Escalate when stuck** — After 3 review iterations, ask the human.
