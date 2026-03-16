# GSD (Get Shit Done) — Playbook v1.22

> Practical operational handbook for the GSD system for developers — from installation, Day 1 workflow, to best practices and advanced troubleshooting.

---

## Part I: Installation & Environment Setup

### 1.1. Quick Installation (30 seconds)

```bash
npx get-shit-done-cc@latest
```

The installer will ask you 2 questions:
1. **Runtime:** Claude Code, OpenCode, Gemini, Codex, or All
2. **Location:** Global (all projects) or Local (current project only)

**Verify successful installation:**

| Runtime | Verification Command |
|---|---|
| Claude Code | `/gsd:help` |
| Gemini CLI | `/gsd:help` |
| OpenCode | `/gsd-help` |
| Codex | `$gsd-help` |

### 1.2. Non-Interactive Installation (Docker, CI, Scripts)

```bash
# Claude Code
npx get-shit-done-cc --claude --global   # → ~/.claude/
npx get-shit-done-cc --claude --local    # → ./.claude/

# Gemini CLI
npx get-shit-done-cc --gemini --global   # → ~/.gemini/

# OpenCode
npx get-shit-done-cc --opencode --global # → ~/.config/opencode/

# Codex
npx get-shit-done-cc --codex --global    # → ~/.codex/

# All
npx get-shit-done-cc --all --global
```

### 1.3. Updating

```bash
npx get-shit-done-cc@latest
```

From v1.17+, the installer automatically backs up local modifications to `gsd-local-patches/`. Use `/gsd:reapply-patches` to restore them.

### 1.4. Skip Permissions Mode (Recommended)

```bash
claude --dangerously-skip-permissions
```

GSD is designed for continuous automation. Stopping to approve `date` and `git commit` 50 times per session breaks the "flow" and significantly reduces productivity.

**Alternative — Granular Permissions:**

```json
{
  "permissions": {
    "allow": [
      "Bash(date:*)", "Bash(echo:*)", "Bash(cat:*)", "Bash(ls:*)",
      "Bash(mkdir:*)", "Bash(wc:*)", "Bash(head:*)", "Bash(tail:*)",
      "Bash(sort:*)", "Bash(grep:*)", "Bash(tr:*)",
      "Bash(git add:*)", "Bash(git commit:*)", "Bash(git status:*)",
      "Bash(git log:*)", "Bash(git diff:*)", "Bash(git tag:*)"
    ]
  }
}
```

### 1.5. Security — Configuring Deny List

**Mandatory** configuration before using GSD with any project containing secrets.

Add to `.claude/settings.json`:

```json
{
  "permissions": {
    "deny": [
      "Read(.env)",
      "Read(.env.*)",
      "Read(**/secrets/*)",
      "Read(**/*credential*)",
      "Read(**/*.pem)",
      "Read(**/*.key)"
    ]
  }
}
```

### 1.6. Docker/Container Environments

If you encounter tilde (`~`) path errors:

```bash
CLAUDE_CONFIG_DIR=/home/youruser/.claude npx get-shit-done-cc --global
```

---

## Part II: Day 1 — New Project (Greenfield)

### 2.1. Complete A→Z Workflow

```bash
# Step 0: Launch Claude Code
claude --dangerously-skip-permissions

# Step 1: Initialize project (Q&A → Research → Requirements → Roadmap)
/gsd:new-project

# Step 2: Clear context, begin phase loop
/clear

# Steps 3-6: Loop for each phase
/gsd:discuss-phase 1        # Lock in preferences
/gsd:plan-phase 1           # Research + Plan + Verify
/gsd:execute-phase 1        # Parallel wave execution
/gsd:verify-work 1          # Manual UAT
/clear

# Repeat for phase 2, 3, ...
/gsd:discuss-phase 2
/gsd:plan-phase 2
/gsd:execute-phase 2
/gsd:verify-work 2
/clear

# When all phases are complete
/gsd:audit-milestone         # Check DoD
/gsd:complete-milestone      # Archive + Tag release
```

### 2.2. Initializing from an Existing Document

If you already have a PRD or idea doc:

```bash
/gsd:new-project --auto @prd.md
```

The system automatically runs research → requirements → roadmap from the document. Continue the normal workflow from discuss-phase.

### 2.3. Tips for `/gsd:new-project`

**Invest time in Q&A.** This is the most important step.
- Answer in detail, don't accept defaults hastily.
- Clearly state edge cases, technical constraints, preferences.
- Clearly describe MVP (v1) versus v2.
- The AI will keep asking until it understands 100%.

---

## Part III: Day 1 — Existing Project (Brownfield)

### 3.1. Map Codebase First

```bash
# Step 0: Analyze codebase (4 mapper agents in parallel)
/gsd:map-codebase

# Output:
# codebase/STACK.md          → Tech stack, dependencies
# codebase/ARCHITECTURE.md   → Code structure, patterns
# codebase/CONVENTIONS.md    → Naming, style, practices
# codebase/CONCERNS.md       → Tech debt, security, risks

# Step 1: Initialize (questions focus on NEW features)
/gsd:new-project
# (normal workflow from here)
```

### 3.2. Why Map First?

- AI **understands the existing code** → doesn't re-ask things already clear in the code
- Plans **follow conventions** discovered automatically
- Avoids creating **"broken windows"** in the existing architecture

---

## Part IV: Daily Operations — Phase Loop

### 4.1. Discuss Phase — "Shape the Build"

```bash
/gsd:discuss-phase 1
```

**When to use:** Before every planning. **ALWAYS** use if you have specific ideas.

**The system analyzes and asks questions based on feature type:**

| Feature Type | Gray Areas AI Will Ask About |
|---|---|
| **Visual (UI)** | Layout, density, interactions, empty states, responsive behavior |
| **APIs/CLIs** | Response format, flags, error handling, verbosity, authentication |
| **Content** | Structure, tone, depth, flow, localization |
| **Organization** | Grouping criteria, naming conventions, duplicates, exceptions |

**Pro-tip:**
- **Deep = precise.** The more detailed your answers, the more accurately AI builds to your intent.
- **Shallow = defaults.** Skipping discuss = receiving a generic product.
- **Output:** `{N}-CONTEXT.md` — this file guides all research + planning.

### 4.2. Plan Phase — "Blueprint the Build"

```bash
/gsd:plan-phase 1
```

**When to use:** After discuss, before execute.

**Automated 3-step process:**
1. **Research** — 4 researchers in parallel (stack, features, architecture, pitfalls)
2. **Plan** — Create 2-3 atomic plans with XML structure
3. **Verify** — Plan checker validates 8 dimensions, loops up to 3 times

**Useful flags:**

| Flag | Effect |
|---|---|
| `--skip-research` | Skip research (familiar domain) |
| `--skip-verify` | Skip plan checker (quick iterations) |

**Before planning — see what AI intends:**

```bash
/gsd:list-phase-assumptions 1
```

This command shows you how Claude intends to approach the phase, before committing to a plan. If the direction is wrong → correct it with discuss.

### 4.3. Execute Phase — "Build It"

```bash
/gsd:execute-phase 1
```

**When to use:** After plans are approved.

**Mechanism:**
- Plans run in **waves** (parallel when independent, sequential when dependent)
- Each executor receives **200K clean tokens** + PROJECT.md + plan
- **Atomic commit** as soon as each task completes
- Auto-verify when all plans finish

**Walk away.** This is when you can go grab a coffee. Come back, check git log.

### 4.4. Verify Work — "Test It"

```bash
/gsd:verify-work 1
```

**When to use:** After execute, before moving to the next phase.

**Guided UAT process:**
1. System lists **testable deliverables**
2. Guides you to test **each one**: "Can you login with email?"
3. You respond: ✅ Yes / ❌ No + error description

**If there are errors:**
- Debug agents automatically diagnose root cause
- Create specific fix plans
- Re-run `/gsd:execute-phase 1` → fix plans are applied
- **You DON'T need to manually debug**

### 4.5. Context Management Between Phases

`/clear` between phases is a **best practice**. GSD is designed around fresh contexts.

```bash
/gsd:execute-phase 1
/gsd:verify-work 1
/clear                    # ← Clear context before new phase
/gsd:discuss-phase 2
```

---

## Part V: Milestone Management

### 5.1. Audit Milestone

```bash
/gsd:audit-milestone
```

Check if the milestone has met the **Definition of Done**:
- All requirements implemented?
- Any stubs/placeholder code?
- Test coverage meets requirements?

### 5.2. Complete Milestone

```bash
/gsd:complete-milestone
```

- Archive milestone documents
- Tag release on git
- Clean up state for the next milestone

### 5.3. Starting a New Milestone

```bash
/gsd:new-milestone [name]
```

Like `/gsd:new-project` but for an existing codebase:
- Describe the new milestone goals
- Research domain for new features
- Scope new requirements
- Create a new roadmap

### 5.4. Handling Gaps After Audit

```bash
/gsd:audit-milestone           # Discover gaps
/gsd:plan-milestone-gaps       # Create phases to close gaps
# (normal phase workflow)
/gsd:complete-milestone        # Archive when done
```

---

## Part VI: Scope Management — Mid-Course Changes

### 6.1. Add Phase

```bash
/gsd:add-phase                 # Append to end of roadmap
```

### 6.2. Insert Emergency Phase

```bash
/gsd:insert-phase 3            # Insert between phase 3 and 4 (auto-renumber)
```

### 6.3. Remove Phase

```bash
/gsd:remove-phase 7            # Descope phase 7, renumber
```

---

## Part VII: Quick Mode — Ad-hoc Tasks

### 7.1. Quick Task

```bash
/gsd:quick
> "Fix login button not responding on mobile Safari"
```

- **Same quality agents** (planner + executor)
- **Skip:** Research, plan checker, verifier
- **Separate tracking:** `.planning/quick/001-fix-login-button/`
- **Use for:** Bug fixes, small features, config changes

### 7.2. Quick with Full Checks

```bash
/gsd:quick --full              # Adds plan-checking + verification
```

### 7.3. Quick with Discussion

```bash
/gsd:quick --discuss           # Pre-planning discussion before execute
```

---

## Part VIII: Session Management

### 8.1. Pausing Mid-Session

```bash
/gsd:pause-work                # Create handoff note
```

### 8.2. Resuming Work

```bash
/gsd:resume-work               # Full context restoration
# or
/gsd:progress                  # See where you are + next steps
```

### 8.3. Systematic Debugging

```bash
/gsd:debug "Login fails after password change"
```

- Persistent debug state
- Root cause analysis
- Fix plan generation

---

## Part IX: Strategic Configuration

### 9.1. Switching Model Profiles

```bash
/gsd:set-profile quality       # Max quality, Opus heavy
/gsd:set-profile balanced      # Default - Opus for planning only
/gsd:set-profile budget        # Cost-effective - Sonnet/Haiku
```

### 9.2. Comprehensive Configuration via Settings

```bash
/gsd:settings                  # Interactive config
```

### 9.3. Git Branching

| Strategy | When to use | Branch name template |
|---|---|---|
| `none` | Solo dev, simple projects | N/A |
| `phase` | Code review per phase | `gsd/phase-{phase}-{slug}` |
| `milestone` | Release branches | `gsd/{milestone}-{slug}` |

### 9.4. Workflow Presets by Scenario

```bash
# Rapid prototyping - speed > quality
/gsd:settings
→ mode: yolo, granularity: coarse, profile: budget
→ research: off, plan_check: off, verifier: off

# Normal development
/gsd:settings
→ mode: interactive, granularity: standard, profile: balanced
→ research: on, plan_check: on, verifier: on

# Production code - quality > speed
/gsd:settings
→ mode: interactive, granularity: fine, profile: quality
→ research: on, plan_check: on, verifier: on
```

---

## Part X: Cheat Sheet — Quick Reference

### Group 1: Initialization

| Command | Meaning |
|---|---|
| `/gsd:map-codebase` | Analyze existing codebase (brownfield) |
| `/gsd:new-project [--auto @file.md]` | Initialize project: Q&A → Research → Roadmap |
| `/gsd:new-milestone [name]` | Start new milestone for current project |

### Group 2: Core Phase Loop

| Command | Meaning |
|---|---|
| `/gsd:discuss-phase [N] [--auto]` | Lock in preferences before planning |
| `/gsd:plan-phase [N] [--auto]` | Research + Plan + Verify (`--skip-research`, `--skip-verify`) |
| `/gsd:execute-phase <N>` | Parallel wave execution with atomic commits |
| `/gsd:verify-work [N]` | Manual UAT + auto-diagnosis |
| `/gsd:validate-phase [N]` | Retroactive test coverage audit (Nyquist) |

### Group 3: Milestone Management

| Command | Meaning |
|---|---|
| `/gsd:audit-milestone` | Check DoD |
| `/gsd:complete-milestone` | Archive + Tag release |
| `/gsd:plan-milestone-gaps` | Create phases for gaps from audit |

### Group 4: Phase Management

| Command | Meaning |
|---|---|
| `/gsd:add-phase` | Add phase to end of roadmap |
| `/gsd:insert-phase [N]` | Insert emergency phase |
| `/gsd:remove-phase [N]` | Remove phase + renumber |
| `/gsd:list-phase-assumptions [N]` | View Claude's intended approach |
| `/gsd:research-phase [N]` | Dedicated deep research |

### Group 5: Session & Navigation

| Command | Meaning |
|---|---|
| `/gsd:progress` | Where am I? Next step? |
| `/gsd:resume-work` | Restore context from previous session |
| `/gsd:pause-work` | Create handoff note |
| `/gsd:help` | All commands |
| `/gsd:update` | Update GSD |

### Group 6: Utilities

| Command | Meaning |
|---|---|
| `/gsd:quick [--full] [--discuss]` | Ad-hoc task with GSD guarantees |
| `/gsd:debug [desc]` | Systematic debugging |
| `/gsd:add-todo [desc]` | Jot down an idea |
| `/gsd:check-todos` | View pending todos |
| `/gsd:settings` | Configure workflow + model |
| `/gsd:set-profile <profile>` | Switch quality/balanced/budget |
| `/gsd:health [--repair]` | Check + repair `.planning/` integrity |
| `/gsd:reapply-patches` | Restore local edits after update |

---

## Part XI: Troubleshooting

### 11.1. Command Not Found

- **Restart runtime** to reload commands/skills
- **Verify files:**
  - Claude: `~/.claude/commands/gsd/` (global) or `./.claude/commands/gsd/` (local)
  - Codex: `~/.codex/skills/gsd-*/SKILL.md`
- **Reinstall:** `npx get-shit-done-cc@latest`

### 11.2. "Project already initialized"

`.planning/PROJECT.md` already exists. If you want to start over: delete the `.planning/` directory.

### 11.3. AI Quality Degradation (Context Degradation)

```bash
/clear                         # Clear context window
/gsd:resume-work               # Restore state from files
```

### 11.4. Plans Going in Wrong Direction

```bash
/gsd:list-phase-assumptions 1  # See what Claude intends
/gsd:discuss-phase 1           # Correct direction with preferences
/gsd:plan-phase 1              # Re-plan
```

### 11.5. Execution Creates Stubs/Incomplete Code

- Plans are too large → break them down further (2-3 tasks/plan max)
- Re-plan with smaller scope

### 11.6. Costs Too High

```bash
/gsd:set-profile budget        # Switch to Sonnet/Haiku
/gsd:settings                  # Turn off research, plan_check, verifier
```

### 11.7. Subagent Reports Fail But Code Was Written

Check `git log` — GSD's orchestrators spot-check output. If commits exist → the work was actually completed.

### 11.8. Uninstall

```bash
# Global
npx get-shit-done-cc --claude --global --uninstall
npx get-shit-done-cc --opencode --global --uninstall
npx get-shit-done-cc --codex --global --uninstall

# Local
npx get-shit-done-cc --claude --local --uninstall
npx get-shit-done-cc --opencode --local --uninstall
npx get-shit-done-cc --codex --local --uninstall
```

---

## Part XII: Best Practices from the Field

### 12.1. Gold Rules — 7 Golden Rules

1. **`/clear` between phases.** Clean context window = peak quality.
2. **Invest time in Discuss.** The clearer you are, the more accurate AI becomes.
3. **Vertical Slices > Horizontal Layers.** Split features E2E, don't split by layer.
4. **Use `/gsd:progress` frequently.** This is your compass.
5. **No manual debugging.** Use `/gsd:verify-work` or `/gsd:debug` to let AI self-diagnose.
6. **Commit initial docs.** `commit_docs: true` so team members and future-you understand context.
7. **Configure deny list BEFORE starting.** Defense-in-depth for secrets.

### 12.2. Git Workflow Tips

- **Atomic commits** enable `git bisect` to find the exact failing task
- **Branch per phase** when code review is needed
- **Squash merge milestones** for a clean main branch

### 12.3. When to Use Quick vs Full Workflow?

| Scenario | Use |
|---|---|
| Small bug fix | `/gsd:quick` |
| Config change | `/gsd:quick` |
| Small feature, familiar domain | `/gsd:quick --full` |
| Complex feature, new domain | Full workflow (discuss → plan → execute → verify) |
| Critical system, production | Full workflow + `fine` granularity + `quality` profile |

### 12.4. Anti-Patterns — What to Avoid

| ❌ Anti-Pattern | ✅ Best Practice |
|---|---|
| Accepting defaults through Discuss too quickly | Answer in detail, describe your personal vision |
| Not using `/clear` between phases | Clear context after every verify |
| Manual debugging when verify fails | Let GSD spawn debug agents |
| Re-running `/gsd:execute-phase` when fixes are needed | Use `/gsd:quick` for targeted fixes |
| Plans too large (5+ tasks) | Keep 2-3 tasks/plan, use `fine` granularity |
| Skipping audit before complete | ALWAYS audit → plan gaps → complete |
