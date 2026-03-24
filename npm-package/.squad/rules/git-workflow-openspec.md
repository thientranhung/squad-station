# Git Rules — OpenSpec

## Branching
- NEVER commit to `develop` or `master` directly.
- Each change = one branch: `feat/<change-name>`, `fix/<change-name>`.
- Merge via PR only. Run `/opsx:verify` before creating PR.

## Commits
- NEVER auto-commit. Wait for orchestrator/user instruction.
- Convention: `feat(<change-name>):`, `fix:`, `refactor:`, `test:`, `docs:`
- Verify specs before committing. Only stage files for current change.
