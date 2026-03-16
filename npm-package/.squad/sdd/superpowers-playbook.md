# Superpowers — Playbook v5.0.0

> Practical guide for Superpowers — installation, usage, troubleshooting.

---

## Installation & Setup

### Claude Code — Official Marketplace

```bash
/plugin install superpowers@claude-plugins-official
```

### Claude Code — Community Marketplace

```bash
/plugin marketplace add obra/superpowers-marketplace
/plugin install superpowers@superpowers-marketplace
```

### Cursor

```text
/plugin-add superpowers
```

### Codex

```
Fetch and follow instructions from https://raw.githubusercontent.com/obra/superpowers/refs/heads/main/.codex/INSTALL.md
```

### OpenCode

```
Fetch and follow instructions from https://raw.githubusercontent.com/obra/superpowers/refs/heads/main/.opencode/INSTALL.md
```

### Verify Installation

After installation, start a new session and try:
- Say: **"help me plan this feature"** → Superpowers auto-triggers brainstorming skill
- Say: **"let's debug this issue"** → Superpowers auto-triggers systematic-debugging skill
- If the agent uses a skill automatically → ✅ installation successful

### Update

```bash
/plugin update superpowers
```

---

## Day 1 Workflow

### Scenario: Build a new feature

```
1. Open Claude Code / Cursor (Superpowers installed)
2. Say: "Build feature X for project Y"
3. Agent AUTOMATICALLY invokes brainstorming
4. Agent asks you questions (1 question at a time, usually MCQ)
5. You answer → Agent continues asking until it has enough understanding
6. Agent proposes 2-3 approaches + recommendation
7. You choose an approach
8. Agent presents design section by section → you approve/modify
9. Agent writes spec → docs/superpowers/specs/
10. Agent AUTOMATICALLY transitions to writing-plans
11. Agent creates plan with bite-sized tasks
12. You say "Go" → Agent dispatches subagents (SDD workflow)
13. Each task: Implement → Spec Review → Quality Review
14. All done → Agent asks: Merge / PR / Keep / Discard?
```

### Your actions on Day 1

| Step | What you do | What the agent does |
|------|-------------|---------------------|
| 1 | Describe feature | Invoke brainstorming |
| 2 | Answer questions | Ask 1 question at a time, MCQ preferred |
| 3 | Choose approach | Propose 2-3 options |
| 4 | Approve design sections | Present section by section |
| 5 | Say "Go" | Create plan + dispatch subagents |
| 6 | (Wait) | SDD: implement → review → fix per task |
| 7 | Choose Merge/PR/Keep/Discard | Execute choice + cleanup |

**Autonomous run time:** The agent can run **for hours** after you say "Go" without intervention (unless BLOCKED).

---

## Daily Operations

### When you want to build a new feature

```
"Build [feature description]"
→ Agent auto-triggers: brainstorming → writing-plans → SDD
```

### When you want to fix a bug

```
"Fix [bug description]"
→ Agent auto-triggers: systematic-debugging → 4-phase investigation → TDD fix
```

### When you want to debug a complex issue

```
"Debug: [issue description]"
→ Agent MUST: Root Cause Investigation → Pattern Analysis → Hypothesis → Implementation
→ NO random fixes allowed
```

### When you want to review code

```
"Review the changes I made in [branch/file]"
→ Agent dispatches code-reviewer subagent
```

### When you want to create a custom skill

```
"Help me create a skill for [technique description]"
→ Agent triggers: writing-skills (TDD for documentation)
→ RED: Baseline test → GREEN: Write skill → REFACTOR: Close loopholes
```

### When you want to brainstorm only (no implementation)

```
"Let's brainstorm about [topic]"
→ Agent follows full brainstorming process
→ Ends with spec document, does not auto-transition to implementation
```

---

## Strategic Configuration

### Instruction Priority

```
User instructions (CLAUDE.md, AGENTS.md) > Superpowers skills > System prompt
```

Override example: If `CLAUDE.md` says "don't use TDD" → Superpowers TDD skill is overridden.

### Personal Skills (Shadowing)

Personal skills override superpowers skills with the same name:

```
~/.claude/skills/brainstorming/SKILL.md    ← Agent uses this
plugin/skills/brainstorming/SKILL.md       ← Shadowed
```

Force using the superpowers version:
```
Invoke superpowers:brainstorming   ← Bypasses personal skill
```

### Skill Types

| Type | Behavior | Example |
|------|----------|---------|
| **Rigid** | Follow exactly, no adaptation | TDD, debugging, verification |
| **Flexible** | Adapt principles to context | Patterns, techniques |

### Model Selection for SDD

| Task | Model recommendation | Signal |
|------|---------------------|--------|
| Mechanical implementation | Cheap/Fast | 1-2 files, clear spec |
| Integration tasks | Standard | Multi-file, pattern matching |
| Design/Architecture/Review | Most Capable | Judgment, broad understanding |

### Output Locations

| Content | Default Path |
|---------|-------------|
| Specs | `docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md` |
| Plans | `docs/superpowers/plans/YYYY-MM-DD-<feature-name>.md` |
| Personal skills | `~/.claude/skills/` (CC) / `~/.agents/skills/` (Codex) |

---

## Cheat Sheet

### Skills Reference

| Skill | Triggered when | Iron Law |
|-------|---------------|----------|
| `brainstorming` | Any creative work | NO code before design approved |
| `writing-plans` | Spec/requirements exist for multi-step task | Bite-sized tasks (2-5 min each) |
| `subagent-driven-development` | Plan exists + platform supports subagent | Fresh subagent per task + two-stage review |
| `executing-plans` | Plan exists + platform does NOT have subagent | Execute continuously, stop only on blocker |
| `test-driven-development` | All implementation | NO production code without failing test first |
| `systematic-debugging` | All technical issues | NO fixes without root cause investigation |
| `verification-before-completion` | Before claiming done/fixed/passing | NO claims without fresh verification evidence |
| `using-git-worktrees` | Before starting implementation | Isolated workspace, verify clean baseline |
| `finishing-a-development-branch` | Implementation done, tests pass | Verify tests → Present 4 options → Execute |
| `requesting-code-review` | After each task (SDD) or before merge | Dispatch code-reviewer subagent |
| `receiving-code-review` | Receiving feedback from reviewer | Fix Critical immediately, Important before continuing |
| `dispatching-parallel-agents` | Multiple independent domains | Identify domains → Create tasks → Dispatch → Integrate |
| `writing-skills` | Creating a new skill | TDD for documentation: RED → GREEN → REFACTOR |
| `using-superpowers` | Every conversation | 1% rule: if a skill COULD apply → MUST invoke |

### TDD Cycle Quick Reference

```
1. RED   — Write failing test (one behavior, clear name)
2. VERIFY RED — Run test, confirm fails correctly (not errors)
3. GREEN — Write MINIMAL code to pass
4. VERIFY GREEN — Run test, confirm ALL pass
5. REFACTOR — Clean up (keep green)
6. REPEAT — Next behavior
```

### SDD Per-Task Flow

```
1. Dispatch implementer subagent (full task text + context)
2. Handle status: DONE → review | BLOCKED → assess | NEEDS_CONTEXT → provide
3. Dispatch spec reviewer → approved? → yes → next | no → fix → re-review
4. Dispatch quality reviewer → approved? → yes → next task | no → fix → re-review
5. Mark task complete
```

### Finishing Branch Options

| Option | Command | Cleanup worktree? |
|--------|---------|-------------------|
| 1. Merge locally | `git checkout main && git merge <branch>` | ✅ |
| 2. Create PR | `git push -u origin <branch> && gh pr create` | ✅ |
| 3. Keep as-is | (nothing) | ❌ |
| 4. Discard | Type "discard" to confirm | ✅ |

---

## Troubleshooting

### Installation Issues

| Error | Fix |
|-------|-----|
| Skills not triggering | Re-run `/plugin install superpowers` |
| "Legacy skills dir" warning | Move skills to `~/.claude/skills/` then delete old dir |
| Codex not loading skills | Follow `docs/README.codex.md` step by step |

### Runtime Issues

| Error | Fix |
|-------|-----|
| Agent doesn't brainstorm and codes directly | Say explicitly: "Let's brainstorm first" or check plugin install |
| Agent skips TDD | Check if CLAUDE.md has an override |
| Subagent BLOCKED continuously | Break task into smaller pieces, upgrade model |
| Review loop runs forever | After 5 iterations → escalate to human |
| Agent uses "should work" | Agent MUST run command before claiming |
| Personal skill override | Use `superpowers:skill-name` prefix to force |

### SDD Issues

| Error | Fix |
|-------|-----|
| Implementer NEEDS_CONTEXT | Provide missing context, re-dispatch |
| Spec reviewer reject | Implementer fixes, re-review |
| Multiple subagents conflict | NEVER dispatch parallel implementation subagents |

---

## Best Practices

### ✅ Gold Rules

1. **Always let the agent brainstorm first** — Even if the project seems "simple"
2. **No code before tests** — If you've already coded → DELETE. Start over
3. **Verify before claiming** — Run command → Read output → THEN claim
4. **1 question per message** — Don't overwhelm with multiple questions
5. **Strict YAGNI** — Remove unnecessary features from every design
6. **Commit frequently** — Every completed step → commit
7. **Worktree for all implementation** — Don't code on main/master
8. **Trust the process** — Systematic debugging > random fixes
9. **Two-stage review** — Spec compliance FIRST, code quality SECOND
10. **Escalate when stuck** — After 5 review iterations → ask human

### ❌ Anti-Patterns

| Anti-Pattern | Replace with |
|-------------|-------------|
| "Too simple to need a design" | Keep brainstorming brief but it MUST happen |
| "I'll test after" | RED-GREEN-REFACTOR |
| "Should work now" | Run verification command |
| "Just this once" | No exceptions |
| "TDD is dogmatic" | TDD IS pragmatic: faster than debugging |
| "This doesn't need a skill" | If skill exists → use it. 1% rule |
| "Fix multiple things at once" | One variable at a time |
| Skip spec review, go to quality | Spec compliance ✅ THEN quality |

### When NOT to use Superpowers workflow

| Scenario | Reason |
|----------|--------|
| Throwaway prototypes | TDD not needed |
| Generated code | Machine-generated, manual TDD not needed |
| Configuration files | TDD can be skipped |
| Quick one-line fix | Brainstorming can be skipped |

---

## Custom Skills — Creating New Skills

### SKILL.md Template

```markdown
---
name: my-skill-name
description: Use when [specific triggering conditions]
---

# My Skill Name

## Overview
Core principle in 1-2 sentences.

## When to Use
- Symptom A
- Symptom B
- NOT for: [exclusions]

## The Process
[Steps, flowcharts, checklists]

## Red Flags
[Signs to STOP]

## Common Mistakes
[Anti-patterns + fixes]
```

### TDD Process for Skills

```
1. RED: Run baseline scenario (agent WITHOUT skill) → document violations
2. GREEN: Write minimal SKILL.md addressing those violations
3. REFACTOR: Find new rationalizations → plug loopholes → re-verify
```
