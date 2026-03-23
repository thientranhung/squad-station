# Release Squad Station $ARGUMENTS

Execute the full release process for squad-station version $ARGUMENTS. Follow every step in order. Stop immediately if any step fails.

---

## 1. PRE-FLIGHT CHECKS

- [ ] Verify you are on the `develop` branch (`git branch --show-current`)
- [ ] Verify working tree is clean (`git status` — no uncommitted changes)
- [ ] Run `cargo test` — **abort the entire release if any test fails**

## 2. VERSION SYNC

All 3 locations **MUST** show the same version `$ARGUMENTS`:

- [ ] `Cargo.toml` → `version = "$ARGUMENTS"`
- [ ] `npm-package/package.json` → `"version": "$ARGUMENTS"`
- [ ] `npm-package/bin/run.js` → `VERSION` variable inside `installBinary()` must be `$ARGUMENTS`

Update any that don't match. Triple-check all 3 before proceeding.

## 3. CHANGELOG

- [ ] Add a new entry at the top of `CHANGELOG.md` following existing format:
  ```
  ## v$ARGUMENTS — <Title> (<date>)

  <summary>

  ### Added / Fixed / Changed
  - ...

  ---
  ```
- [ ] Use today's date. Include Added/Fixed/Changed sections as appropriate.
- [ ] Ask the user for the release title and summary if not obvious from recent commits.

## 4. BUILD & TEST

- [ ] `cargo build --release`
- [ ] `cargo test` — all tests must pass
- [ ] Verify the symlink works: `squad-station --version` must output `$ARGUMENTS`
- [ ] If version doesn't match, check `~/.cargo/bin/squad-station` symlink points to `target/release/squad-station`

## 5. GIT

- [ ] `git add` all changed files (Cargo.toml, Cargo.lock, package.json, bin/run.js, CHANGELOG.md)
- [ ] `git commit -m "release: v$ARGUMENTS — <summary>"`
- [ ] `git checkout master && git merge develop`
- [ ] `git tag v$ARGUMENTS`
- [ ] `git push origin master develop v$ARGUMENTS`
- [ ] `git checkout develop`

## 6. NPM PACKAGE

- [ ] Verify `npm-package/bin/run.js` has executable permission: `chmod +x npm-package/bin/run.js`
- [ ] Run `cd npm-package && npm pkg fix` to normalize package.json
- [ ] **DO NOT run `npm publish`** — tell the user to run it manually (requires OTP authentication)

## 7. POST-RELEASE VERIFICATION

- [ ] Check GitHub Actions started: `gh run list --limit 5`
- [ ] Tell the user:
  > Wait for GitHub Actions to complete, then run:
  > ```
  > cd npm-package && npm publish
  > ```
- [ ] After publish: verify `npx squad-station@$ARGUMENTS --version` works
- [ ] Verify GitHub release notes match CHANGELOG.md entry

---

## ⚠️ IMPORTANT — Lessons from past releases

1. **Version sync is critical.** All 3 locations (Cargo.toml, package.json, bin/run.js) MUST match. The v0.6.2 release had a version mismatch that required a follow-up v0.6.3.
2. **bin/run.js MUST be executable.** Always `chmod +x` before committing. The v0.6.3 release was specifically to fix this.
3. **Never use `generate_release_notes: true`** in GitHub Actions — we maintain CHANGELOG.md manually.
4. **npm publish requires user interaction** (OTP) — never attempt it automatically.
5. **Always push the tag** — `git push origin v$ARGUMENTS` — or GitHub Actions won't trigger.
