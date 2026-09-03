## 1. GP-10 — spec CLI resolution under `--supervisor` + doctor honesty

- [ ] 1.1 Thread the 5-level spec-driven CLI resolution chain into the supervisor spec-launch dispatch so `default_spec_cli` is honoured under `--supervisor --specs` (no separate/bypassed resolution)
- [ ] 1.2 Tests: `--supervisor --specs` with `default_spec_cli` set launches the configured CLI on every spec worker; the non-supervisor resolution scenarios still pass
- [ ] 1.3 Add a `doctor` check that reports the effective spec-worker CLI and warns on a mismatch with configured `default_spec_cli`; read-only, informational when unconfigured
- [ ] 1.4 Tests: doctor warns on mismatch, passes on match

## 2. GP-12 — single default-branch resolver with `origin/HEAD` fallback

- [ ] 2.1 Reconcile the two resolvers (`default_branch()`/`origin/HEAD` at git.rs:145 and the `symbolic-ref --short HEAD` path at git.rs:289) into one resolver used by `start`, resume, `add`, and rebase
- [ ] 2.2 Fallback when `origin/HEAD` is absent: local `main` → local `master` → checked-out branch (documented precedence)
- [ ] 2.3 Tests: resolves without `origin/HEAD` (no abort); first launch and resume resolve identically; `origin/HEAD` still honoured when present

## 3. GP-13 — start reattaches instead of forking

- [ ] 3.1 Before `resolve_session_name` suffixing, look up an existing live session for the current repository (find-by-repo) and reattach when found
- [ ] 3.2 Reserve `-N` for genuinely distinct sessions (cross-repo name collisions); a non-interactive re-`start` refuses with an actionable error rather than forking
- [ ] 3.3 Tests: re-`start` reattaches (no `-2`); a distinct repo sharing a name still gets `-2`; non-interactive re-`start` refuses

## 4. Docs

- [ ] 4.1 CLI reference / user guide: `start`'s reattach behaviour + the default-branch fallback
- [ ] 4.2 Doctor chapter: the spec-CLI honesty check
- [ ] 4.3 `mdbook build docs/` succeeds

## 5. Gates

- [ ] 5.1 `just check` green; no `unwrap()`/`expect()` in non-test code; public items documented
- [ ] 5.2 Gate-5 security: GP-10 workers launch the configured (sandboxed) CLI; the doctor blind spot is closed
- [ ] 5.3 Every spec scenario maps to a test
