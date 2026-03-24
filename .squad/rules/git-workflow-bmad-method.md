# Git Rules — BMad Method

## Branching
- NEVER commit to `develop` or `master` directly.
- Create branch before code changes: `feat/epic-<N>-<name>`, `feat/story-<epic>-<story>-<name>`, `fix/<desc>`, `quick/<desc>`.
- Merge via PR only. Run `/bmad-code-review` before every PR.

## Commits
- NEVER auto-commit. Wait for orchestrator/user instruction.
- Convention: `feat(epic-3/story-2):`, `fix:`, `refactor:`, `test:`, `docs:`
- Run tests before committing. One story = one atomic commit.
