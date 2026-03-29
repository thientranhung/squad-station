# OpenSpec — Agent Playbook

## How OpenSpec Works

OpenSpec is **spec-driven** — every feature is a "change" with its own folder of planning artifacts: proposal → specs → design → tasks. One change = one unit of work. Changes live in `openspec/changes/<name>/`.

## Orchestrator Routing

OpenSpec uses slash commands. The orchestrator selects the right command based on where the change is:

| Situation | Command to send |
|---|---|
| New feature, reasonably clear | `/opsx:propose <feature-name>` |
| Unclear requirements, need exploration | `/opsx:explore <topic>` then `/opsx:propose <name>` |
| Change exists, ready to implement | `/opsx:apply <name>` |
| Implementation done, need validation | `/opsx:verify <name>` |
| Validated, ready to close | `/opsx:archive <name>` |
| Multiple completed changes | `/opsx:bulk-archive` |

### Profile awareness

**Core profile** (default): `propose` → `apply` → `archive` (3 commands, fast)
**Expanded profile**: `new` → `continue` (step by step) → `apply` → `verify` → `archive` (granular control)

If you don't know the profile, use core commands — they always work.

## Workflow Sequence

**Quick path (core):**
1. `/opsx:propose <feature-name>` — creates ALL artifacts (proposal, specs, design, tasks)
2. `/opsx:apply` — implement tasks from the plan
3. `/opsx:archive` — archive completed change, merge delta specs into source-of-truth

**Complex feature (expanded):**
1. `/opsx:explore [topic]` — explore problem space (no artifacts)
2. `/opsx:new <feature-name>` — scaffold change folder
3. `/opsx:continue [name]` — create next artifact, review (repeat)
4. `/opsx:apply [name]` — implement
5. `/opsx:verify [name]` — validate vs specs
6. `/opsx:archive [name]` — close

**Parallel changes:** Start new with `/opsx:new <name>` anytime. Resume with `/opsx:apply <name>`.

## Document Discipline

OpenSpec is self-documenting by design — artifacts are the documentation:
- `proposal.md` — WHY and SCOPE
- `specs/**/*.md` — WHAT (behavioral specs, not implementation)
- `design.md` — HOW (technical approach)
- `tasks.md` — STEPS (checkboxes, trackable)

Orchestrator must verify:
- After `/opsx:propose`: all 4 artifacts exist in `openspec/changes/<name>/`
- After `/opsx:apply`: tasks.md checkboxes are checked
- After `/opsx:archive`: change moved to `archive/`, delta specs merged into `openspec/specs/`

**Key**: Specs describe behavior, NOT implementation (no class names, no framework choices).

## Critical Rules

1. **One change = one unit of work** — never mix unrelated features in a single change.
2. **Name changes clearly** — descriptive kebab-case: `add-dark-mode`, `fix-login-redirect`. Never `update` or `wip`.
3. **Explore before committing** — `/opsx:explore` when requirements unclear.
4. **Verify before archiving** — `/opsx:verify` catches spec-implementation mismatches.
5. **Verify agent completion after SQUAD SIGNAL** — SQUAD SIGNAL means the agent processed your message, not that the workflow is complete. Check if the agent is still generating artifacts, reviewing, or asking questions before marking done.
