---
phase: 07-ci-cd-pipeline
plan: 01
subsystem: infra
tags: [github-actions, ci-cd, cross-compilation, rust, musl, cargo, cross-rs, release-automation]

requires: []
provides:
  - Cross-platform GitHub Actions release workflow producing 4 binary assets on v* tag push
  - Binary naming convention: squad-station-{os}-{arch} (darwin-arm64, darwin-x86_64, linux-x86_64, linux-arm64)
affects:
  - 08-npm-package (downloads binaries from GitHub Releases)
  - 09-install-script (curl | sh install downloads from GitHub Releases)

tech-stack:
  added:
    - softprops/action-gh-release@v2 (GitHub Release creation and asset upload)
    - dtolnay/rust-toolchain@stable (Rust toolchain setup in CI)
    - cross-rs/cross (Docker-based cross-compilation for aarch64-unknown-linux-musl)
  patterns:
    - Matrix strategy with fail-fast: false for independent platform builds
    - SQLX_OFFLINE=true for compile-time DB query skipping in CI
    - musl targets for fully static Linux binaries (no glibc dependency)

key-files:
  created:
    - .github/workflows/release.yml
  modified: []

key-decisions:
  - "musl over gnu for Linux targets: produces fully static binaries that run on any Linux distro without glibc version mismatch"
  - "cross tool only for aarch64-unknown-linux-musl: macOS and x86_64 Linux build natively without Docker overhead"
  - "softprops/action-gh-release@v2: creates release if absent, appends assets if present — safe for concurrent matrix uploads"
  - "fail-fast: false: all 4 targets attempted even if one fails, enabling partial failure debugging"
  - "SQLX_OFFLINE=true: skips compile-time DB connection check since DATABASE_URL is unavailable in CI"
  - "Binary naming convention squad-station-{os}-{arch}: established for Phase 8 (npm) and Phase 9 (install script) consumption"

patterns-established:
  - "Release workflow: push v* tag → 4 parallel matrix jobs → single GitHub Release with 4 assets"
  - "Cross-compilation: native cargo for darwin and linux-x86_64, cross tool for linux-arm64"

requirements-completed:
  - CICD-01
  - CICD-02
  - CICD-03

duration: 8min
completed: 2026-03-08
---

# Phase 7 Plan 01: CI/CD Release Workflow Summary

**GitHub Actions matrix workflow cross-compiling squad-station to 4 platforms (darwin-arm64, darwin-x86_64, linux-x86_64, linux-arm64) and auto-publishing GitHub Releases with binary assets on v* tag push**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-03-08T15:22:31Z
- **Completed:** 2026-03-08T15:30:00Z
- **Tasks:** 1 of 2 executed (Task 2 is human-verify checkpoint)
- **Files modified:** 1

## Accomplishments

- Created `.github/workflows/release.yml` with matrix strategy for all 4 target platforms
- Linux targets use musl for fully static binaries with zero glibc dependency — critical for Phase 9 install script portability
- `aarch64-unknown-linux-musl` built via cross tool (Docker-based) — only target requiring cross-compilation tooling
- `SQLX_OFFLINE=true` set on build step — required since no DATABASE_URL is available in CI runners
- `softprops/action-gh-release@v2` safely handles concurrent asset uploads from 4 parallel matrix jobs to the same release
- Binary naming convention `squad-station-{os}-{arch}` established — Phases 8 and 9 depend on this exact naming

## Task Commits

1. **Task 1: Create GitHub Actions release workflow** - `ceb38fd` (ci)

**Note:** Task 2 is a `checkpoint:human-verify` — requires pushing the test tag and confirming all 4 CI jobs pass with 4 binary assets on the GitHub Release.

## Files Created/Modified

- `.github/workflows/release.yml` — Cross-platform release workflow: matrix strategy (4 targets), conditional cross tool, SQLX_OFFLINE, softprops release upload

## Decisions Made

- **musl over gnu for Linux:** Produces fully static binaries. A dynamically linked gnu binary would fail on Alpine or older glibc distros — fatal for Phase 9 install script.
- **cross tool only for linux-arm64:** The `aarch64-unknown-linux-musl` target requires cross-compilation tooling on ubuntu-latest. macOS arm64 builds natively on `macos-latest`. x86_64 Linux musl is supported natively on ubuntu-latest with `rustup target add`.
- **softprops/action-gh-release@v2:** Idempotent — creates release if absent, appends assets if present. Enables 4 parallel matrix jobs to upload safely to the same release without race conditions.
- **fail-fast: false:** Allows all 4 targets to complete even if one fails. Useful for diagnosing platform-specific compilation errors without re-running the entire matrix.
- **SQLX_OFFLINE=true:** sqlx performs compile-time query validation against a live database by default. CI has no DATABASE_URL, so offline mode must be explicitly enabled.
- **Binary naming `squad-station-{os}-{arch}`:** Phases 8 and 9 will construct download URLs using this convention. Changing it after this point would break downstream phases.

## Deviations from Plan

None - plan executed exactly as written. The `SQLX_OFFLINE: "true"` condition in the task (check for .sqlx metadata) was evaluated — no .sqlx directory or sqlx-data.json exists — confirming that SQLX_OFFLINE must always be set.

## Issues Encountered

None during Task 1. Task 2 (human verification) is pending.

## User Setup Required

**Human verification required for Task 2.** Push the test tag and verify on GitHub Actions:

1. Push the workflow file and create a test tag:
   ```bash
   git push
   git tag v0.1.0-test && git push origin v0.1.0-test
   ```

2. Visit the GitHub Actions tab — confirm all 4 matrix jobs turn green (allow 5-10 minutes).

3. Navigate to GitHub Releases — confirm `v0.1.0-test` was created with 4 assets:
   `squad-station-darwin-arm64`, `squad-station-darwin-x86_64`, `squad-station-linux-x86_64`, `squad-station-linux-arm64`

4. Download and verify:
   ```bash
   chmod +x squad-station-darwin-arm64 && ./squad-station-darwin-arm64 --version
   ```

5. Clean up after verification:
   - Delete the release via GitHub Releases UI
   - `git push origin --delete v0.1.0-test && git tag -d v0.1.0-test`

## Next Phase Readiness

- `.github/workflows/release.yml` is ready. Phase 8 (npm package) and Phase 9 (install script) can begin planning immediately.
- Both Phase 8 and Phase 9 must reference the binary naming convention: `squad-station-{os}-{arch}`.
- Actual release assets are only available after a real `v*` tag is pushed — coordinate timing with Phase 8/9 implementation.

---
*Phase: 07-ci-cd-pipeline*
*Completed: 2026-03-08*
