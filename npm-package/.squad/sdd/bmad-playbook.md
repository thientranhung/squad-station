# BMad Method — Playbook v6.0.4

> Comprehensive operational guide for BMad Method — installation, Day 1 workflow, cheat sheet, troubleshooting, best practices.

---

## Installation & Setup

### Prerequisites

| Requirement | Version | Notes |
|---|---|---|
| **Node.js** | ≥ 20.0.0 | Required |
| **Git** | Latest | Recommended |
| **AI IDE** | Latest | Claude Code, Cursor, Codex CLI, Windsurf, OpenCode |
| **npm** | Bundled with Node.js | Needed for `npx` |

### Installation — Interactive Mode

```bash
npx bmad-method install
npx bmad-method@6.0.4 install  # If stuck on stale beta version
```

**Steps the installer will ask:**
1. **Select modules** — BMM, BMB, TEA, GDS, CIS
2. **Select IDE** — Claude Code, Cursor, Windsurf...
3. **Configure paths** — Output artifacts path
4. **Project name** — Project name
5. **Skill level** — beginner / intermediate / expert
6. **Confirm** — Review and confirm

### Installation — Non-Interactive Mode (CI/CD)

```bash
npx bmad-method install --directory /path/to/project --modules bmm --tools claude-code --yes
npx bmad-method install --directory ./my-project --modules bmm,tea,bmb --tools cursor --yes
```

| Flag | Description |
|---|---|
| `--directory <path>` | Project path |
| `--modules <list>` | Modules: `bmm`, `bmb`, `tea`, `gds`, `cis` |
| `--tools <ide>` | IDE: `claude-code`, `cursor`, `windsurf`, `codex`, `opencode` |
| `--yes` | Skip confirmation |

### Post-Installation Result

```
your-project/
├── _bmad/                         # BMad configuration
│   ├── agents/                    # Agent persona files
│   ├── workflows/                 # Workflow configs
│   ├── tasks/                     # Reusable tasks
│   └── data/                      # Reference data
├── _bmad-output/
│   ├── planning-artifacts/        # PRD, Architecture, Epics
│   ├── implementation-artifacts/  # Sprint status, Stories
│   └── project-context.md         # (optional) Implementation rules
├── .claude/skills/                # Generated skills (depends on IDE)
└── docs/                          # Project knowledge
```

---

## Day 1 Workflow

### Greenfield Project (New Project)

```
Install BMad → Open AI IDE → Run bmad-help → Choose Planning Track:
  - Bug fix → Quick Flow: bmad-quick-spec → bmad-quick-dev → Done!
  - Product → BMad Method: brainstorming → create-prd → create-ux-design → create-architecture → create-epics-and-stories → check-implementation-readiness → sprint-planning → Build Cycle
  - Enterprise → (same as BMad Method, more thorough)
```

### Brownfield Project (Existing Project)

1. **Install BMad** — `npx bmad-method install` in the existing project folder
2. **Run `/bmad-help`** — BMad-Help will detect project state
3. **Create Project Context** — `/bmad-generate-project-context`
4. **Choose track** — Quick Flow for small changes, BMad Method for major features
5. **Start from the appropriate phase**

### Build Cycle (Repeat for each story)

| Step | Agent | Command | Description |
|---|---|---|---|
| 1 | Scrum Master | `/bmad-create-story` | Create story file from epic |
| 2 | Developer | `/bmad-dev-story` | Implement story (new chat!) |
| 3 | Developer | `/bmad-code-review` | Review code quality |
| 4 | Scrum Master | `/bmad-retrospective` | After completing an epic |

⚡ **IMPORTANT:** Always use a **fresh chat** for each workflow!

---

## Daily Operations

### Daily Operations Checklist

1. Open AI IDE in project folder
2. Run `/bmad-help` → view project state and next steps
3. Run `/bmad-create-story` for the next story (new chat)
4. Run `/bmad-dev-story` to implement (new chat)
5. Run `/bmad-code-review` when finished (new chat)
6. Commit code frequently

When an epic is completed: `/bmad-retrospective` (new chat)
When scope changes: `/bmad-correct-course` (new chat)

---

## Strategic Configuration

### Presets by project type

| Project Type | Modules | Track |
|---|---|---|
| **Side project / MVP** | `bmm` | Quick Flow |
| **Startup product** | `bmm` + `cis` | BMad Method |
| **Enterprise app** | `bmm` + `tea` | Enterprise |
| **Game** | `bmm` + `gds` | BMad Method |
| **Custom agents** | `bmm` + `bmb` | BMad Method |

### When to use each Agent?

| Situation | Agent | Skill |
|---|---|---|
| New idea | Analyst (Mary) | `/bmad-brainstorming` |
| Market/tech research | Analyst (Mary) | `/bmad-research` |
| Writing requirements | PM (John) | `/bmad-create-prd` |
| UI/UX design | UX Designer (Sally) | `/bmad-create-ux-design` |
| Architecture design | Architect (Winston) | `/bmad-create-architecture` |
| Sprint planning | Scrum Master (Bob) | `/bmad-sprint-planning` |
| Code implementation | Developer (Amelia) | `/bmad-dev-story` |
| Code review | Developer (Amelia) | `/bmad-code-review` |
| Quick bug fix | Quick Flow (Barry) | `/bmad-quick-spec` + `/bmad-quick-dev` |
| Don't know what to do next | BMad-Help | `/bmad-help` |

---

## Cheat Sheet

### Core Skills

| Skill | Phase | Description |
|---|---|---|
| `/bmad-help` | Any | Intelligent guide |
| `/bmad-brainstorming` | 1 | Guided ideation session |
| `/bmad-research` | 1 | Market/technical research |
| `/bmad-create-product-brief` | 1 | Foundation document |
| `/bmad-create-prd` | 2 | Product Requirements Document |
| `/bmad-create-ux-design` | 2 | UX Design document |
| `/bmad-quick-spec` | 2 | Quick Flow: tech-spec |
| `/bmad-create-architecture` | 3 | Architecture document |
| `/bmad-create-epics-and-stories` | 3 | Break PRD → epics |
| `/bmad-check-implementation-readiness` | 3 | Validate planning cohesion |
| `/bmad-sprint-planning` | 4 | Initialize sprint tracking |
| `/bmad-create-story` | 4 | Create story file |
| `/bmad-dev-story` | 4 | Implement story |
| `/bmad-quick-dev` | 4 | Quick Flow: implement |
| `/bmad-code-review` | 4 | Adversarial code review |
| `/bmad-retrospective` | 4 | Epic retrospective |
| `/bmad-correct-course` | Any | Handle scope changes |

### Agent Personas

| Skill | Persona |
|---|---|
| `/bmad-analyst` | Mary — Analyst |
| `/bmad-pm` | John — Product Manager |
| `/bmad-architect` | Winston — Architect |
| `/bmad-sm` | Bob — Scrum Master |
| `/bmad-dev` | Amelia — Developer |
| `/bmad-qa` | Quinn — QA Engineer |
| `/bmad-master` | Barry — Quick Flow Solo Dev |
| `/bmad-ux-designer` | Sally — UX Designer |
| `/bmad-tech-writer` | Paige — Technical Writer |
| `/bmad-party-mode` | All agents in one room |

### Utilities

| Skill | Description |
|---|---|
| `/bmad-shard-doc` | Split large markdown file |
| `/bmad-index-docs` | Index project documentation |
| `/bmad-generate-project-context` | Generate project context file |

---

## Troubleshooting

| Error | Fix |
|---|---|
| Skills not showing | Restart IDE, check settings |
| Old version | `npx bmad-method@6.0.4 install` |
| Old skills module | `rm -rf .claude/skills/bmad-*` → re-install |
| Nested install | Install at project root |
| Context noise | Fresh chat for each workflow |
| Agent wrong role | Fresh chat + invoke correct skill |
| Quick Flow scope creep | Escalate to BMad Method |
| Cannot find module | `nvm install 20` |

### Debug Steps

```bash
node --version            # Must be >= 20
npm cache clean --force
npx bmad-method@6.0.4 install
ls -la .claude/skills/
cat _bmad/module.yaml
```

---

## Best Practices

### ✅ Gold Rules

1. **Always use a fresh chat** for each workflow — avoid context pollution
2. **Start with `/bmad-help`** — inspect project state → recommend next step
3. **Create Project Context early** — agents understand tech preferences
4. **Choose the right planning track** — Quick Flow / BMad Method / Enterprise
5. **Run Implementation Readiness** before coding
6. **Code Review every story** — adversarial review catches bugs early
7. **Retrospective every epic** — continuous improvement
8. **Commit code frequently** — keep diffs small
9. **Upgrade BMad regularly** — bug fixes, better behavior

### ❌ Anti-Patterns

| Anti-Pattern | Solution |
|---|---|
| Running multiple workflows in the same chat | Fresh chat for each workflow |
| Skip PRD and go straight to code | At minimum need a PRD or tech-spec |
| Vibe-coding instead of following the process | Use structured workflows |
| Not creating Project Context | Create `project-context.md` early |
| Using Quick Flow for complex features | Escalate to BMad Method |
| Not running Code Review | Always `/bmad-code-review` after every story |
