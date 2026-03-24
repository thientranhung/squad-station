# Git Rules — GSD

## Branching
- NEVER commit to `develop` or `master` directly.
- Create branch before code changes: `milestone/<name>`, `phase/<N>-<desc>`, `fix/<desc>`, `quick/<desc>`.
- Merge via PR only. Use `/gsd:ship` for PRs.

## Commits
- NEVER auto-commit. Wait for orchestrator/user instruction.
- Convention: `feat(phase-3):`, `fix(phase-1):`, `refactor:`, `test:`, `docs:`
- Run tests before committing.
