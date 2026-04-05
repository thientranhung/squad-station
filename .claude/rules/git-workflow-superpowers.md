# Git Rules — Superpowers

## Branching
- NEVER commit to `develop` or `master` directly.
- Create branch before code changes: `feat/<feature>`, `fix/<bug>`. Use `using-git-worktrees` skill.
- Merge via PR only. Use `finishing-a-development-branch` skill for merge decisions.

## Commits
- NEVER auto-commit. Wait for orchestrator/user instruction.
- Convention: `feat(<feature>):`, `fix:`, `test:`, `refactor:`, `docs:`
- TDD flow: `test:` → `feat:` → `refactor:`. All tests must pass before commit.
