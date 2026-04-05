# GSD (Get Shit Done) — Agent Playbook

## Workflow Sequence

**New project (greenfield):**
1. `/gsd:new-project` — Initialize: Q&A → Research → Requirements → Roadmap
2. `/clear`
3. **Phase loop** (repeat for each phase N):
   - `/gsd:discuss-phase N` — Lock in preferences
   - `/gsd:ui-phase N` — UI design contract (frontend phases only)
   - `/gsd:plan-phase N` — Research + Plan + Verify
   - `/gsd:execute-phase N` — Parallel wave execution + regression gate
   - `/gsd:verify-work N` — Manual UAT + auto-diagnosis
   - `/gsd:ui-review N` — Visual audit (frontend phases only)
   - `/clear`
4. `/gsd:audit-milestone` — Check Definition of Done
5. `/gsd:ship` — Create PR from planning artifacts
6. `/gsd:complete-milestone` — Archive + Tag release

**Existing project (brownfield):**
1. `/gsd:map-codebase [area]` — Full codebase analysis (4 parallel mappers)
2. `/gsd:new-milestone [name]` — Start milestone
3. Continue with phase loop above

**Quick task:** `/gsd:quick [--discuss] [--research] [--full]` — Ad-hoc task with GSD guarantees

**Auto-advance:** `/gsd:next` — Determines and runs the next logical step

**Autonomous:** `/gsd:autonomous [--from N]` — Run all remaining phases automatically

## Command Reference

### Initialization

| Command | Description |
|---|---|
| `/gsd:new-project [--auto @file.md]` | Initialize project: Q&A → Research → Roadmap |
| `/gsd:new-milestone [name]` | Start new milestone |
| `/gsd:map-codebase [area]` | Analyze existing codebase (4 parallel mappers) |

### Core Phase Loop

| Command | Description |
|---|---|
| `/gsd:discuss-phase [N] [--auto] [--batch]` | Lock in preferences before planning |
| `/gsd:ui-phase [N]` | UI design contract (frontend phases) |
| `/gsd:plan-phase [N] [--auto] [--skip-research] [--skip-verify]` | Research + Plan + Verify |
| `/gsd:execute-phase N` | Parallel wave execution + node repair + regression gate |
| `/gsd:verify-work [N]` | Manual UAT + auto-diagnosis |
| `/gsd:ui-review [N]` | 6-pillar visual audit (frontend) |
| `/gsd:validate-phase [N]` | Retroactive test coverage audit |
| `/gsd:ship [N] [--draft]` | Create PR from planning artifacts |

### Milestone Management

| Command | Description |
|---|---|
| `/gsd:audit-milestone` | Check Definition of Done |
| `/gsd:complete-milestone` | Archive + Tag release |
| `/gsd:plan-milestone-gaps` | Create phases for gaps from audit |
| `/gsd:stats` | Project statistics dashboard |

### Phase Management

| Command | Description |
|---|---|
| `/gsd:add-phase` | Add phase to end of roadmap |
| `/gsd:insert-phase [N]` | Insert emergency phase (decimal numbering) |
| `/gsd:remove-phase [N]` | Remove phase + renumber |
| `/gsd:list-phase-assumptions [N]` | View AI's intended approach |
| `/gsd:research-phase [N]` | Dedicated deep research |
| `/gsd:add-tests [N]` | Generate tests for completed phase |

### Session & Navigation

| Command | Description |
|---|---|
| `/gsd:progress` | Where am I? What's next? |
| `/gsd:next` | Auto-advance to next logical step |
| `/gsd:resume-work` | Restore context from previous session |
| `/gsd:pause-work` | Save handoff state for later |
| `/gsd:session-report` | Session summary |
| `/gsd:help` | All commands |
| `/gsd:update` | Update GSD + changelog preview |

### Utilities

| Command | Description |
|---|---|
| `/gsd:quick [--discuss] [--research] [--full]` | Ad-hoc task with GSD guarantees |
| `/gsd:autonomous [--from N]` | Run all remaining phases autonomously |
| `/gsd:do` | Freeform text → auto-route to right command |
| `/gsd:note [text\|list\|promote N]` | Zero-friction idea capture |
| `/gsd:debug [desc]` | Systematic debugging + persistent knowledge base |
| `/gsd:profile-user` | Developer behavioral profile |
| `/gsd:health [--repair]` | Check + repair `.planning/` integrity |
| `/gsd:cleanup` | Archive completed milestone directories |
| `/gsd:set-profile <quality\|balanced\|budget>` | Switch model profile |
| `/gsd:settings` | Configure workflow + model |

## Critical Rules

1. **`/clear` between phases** — Clean context window after every verify/review cycle.
2. **Invest time in discuss-phase** — The clearer the preferences, the more accurate the plan.
3. **Vertical slices over horizontal layers** — Split features end-to-end, not by layer.
4. **Always audit before completing a milestone** — `audit-milestone` → `plan-milestone-gaps` → `complete-milestone`.
5. **Use `/gsd:next` when unsure** — It auto-determines the correct next step.
