# BMad Method — Agent Playbook

## Workflow Sequence

**Greenfield (new project):**
1. `/bmad-brainstorming` — Explore ideas
2. `/bmad-product-brief-preview` — Create product brief (guided/yolo/autonomous)
3. `/bmad-create-prd` — Write requirements
4. `/bmad-create-ux-design` — UX design document
5. `/bmad-create-architecture` — Architecture document
6. `/bmad-create-epics-and-stories` — Break PRD into epics
7. `/bmad-check-implementation-readiness` — Validate planning cohesion
8. `/bmad-sprint-planning` — Initialize sprint tracking
9. **Build Cycle** (repeat per story): `create-story` → `dev-story` → `code-review` → `retrospective`

**Brownfield (existing project):**
1. `/bmad-help` — Detect project state, get recommendations
2. `/bmad-generate-project-context` — Document tech preferences
3. Pick appropriate phase above based on project state

**Quick fix / small change:** `/bmad-quick-dev` — Unified workflow (clarify → plan → implement → review → present)

## Command Reference

### Phase 1: Analysis

| Command | Description |
|---|---|
| `/bmad-brainstorming` | Guided ideation session |
| `/bmad-product-brief-preview` | Guided/yolo/autonomous product brief |
| `/bmad-create-product-brief` | Foundation document |
| `/bmad-market-research` | Market analysis, competitive landscape |
| `/bmad-domain-research` | Industry domain deep dive |
| `/bmad-technical-research` | Technical feasibility analysis |
| `/bmad-document-project` | Analyze existing project for docs |

### Phase 2: Planning

| Command | Description |
|---|---|
| `/bmad-create-prd` | Product Requirements Document |
| `/bmad-validate-prd` | Validate PRD completeness |
| `/bmad-edit-prd` | Improve existing PRD |
| `/bmad-create-ux-design` | UX Design document |

### Phase 3: Solutioning

| Command | Description |
|---|---|
| `/bmad-create-architecture` | Architecture document |
| `/bmad-create-epics-and-stories` | Break PRD into epics |
| `/bmad-check-implementation-readiness` | Validate cohesion between PRD, Architecture, Epics |
| `/bmad-generate-project-context` | Generate project context from codebase |

### Phase 4: Implementation

| Command | Description |
|---|---|
| `/bmad-sprint-planning` | Initialize sprint tracking |
| `/bmad-sprint-status` | Summarize sprint status |
| `/bmad-create-story` | Create story file from epic |
| `/bmad-dev-story` | Implement story |
| `/bmad-quick-dev` | Unified quick flow |
| `/bmad-code-review` | Sharded parallel code review (4 steps) |
| `/bmad-qa-generate-e2e-tests` | Generate E2E tests |
| `/bmad-correct-course` | Handle scope changes |
| `/bmad-retrospective` | Epic retrospective |

### Utilities

| Command | Description |
|---|---|
| `/bmad-help` | Intelligent guide — ask anything |
| `/bmad-party-mode` | Multi-agent collaboration |
| `/bmad-review-adversarial-general` | Adversarial content review |
| `/bmad-review-edge-case-hunter` | Edge case analysis for code |
| `/bmad-distillator` | Lossless LLM-optimized document compression |
| `/bmad-index-docs` | Create doc index for LLM scanning |
| `/bmad-shard-doc` | Split large markdown file |

## Critical Rules

1. **Fresh chat per workflow** — Each workflow must run in a clean context. Never chain workflows.
2. **Run implementation-readiness before coding** — Validates cohesion between PRD, Architecture, and Epics.
3. **Code review every story** — Always `/bmad-code-review` after `/bmad-dev-story`.
4. **Start with `/bmad-help`** — It inspects project state and recommends the exact next step.
