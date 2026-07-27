# Tasks — git-paw-doctor

## 1. CLI surface
- [ ] Add a `Doctor { json: bool }` variant to the `Commands` enum in `src/cli.rs` with `about` + `long_about` + examples, and `--json` flag help
- [ ] Add the dispatch arm in `main.rs` calling into the new `doctor` module
- [ ] Ensure the root `after_help` / CLI table mentions `doctor` as the diagnostic entry point (`selftest` is now hidden — see §1b)

## 1b. selftest → doctor's live-smoke arm (user decision 2026-07-27)
- [x] Hide the `selftest` command from the public surface (`#[command(hide = true)]`) + reframe its help as the internal live arm of `doctor`; flip the help-visibility test to `selftest_is_hidden_from_help`; drop it from the root `after_help` quick-start (landed early on the branch as a `feat(cli)` slice)
- [ ] Give `doctor` a live-smoke arm that runs the selftest lifecycle as part of the suite (e.g. `git paw doctor --live` / `--smoke`), reusing `src/selftest.rs` — static preflight + live smoke under one diagnostic surface
- [ ] Keep `src/selftest.rs` as the harness + keep `tests/selftest_e2e.rs`; the harness is still reused by `unattended_drive_e2e`/`pause_e2e` — do NOT remove it
- [ ] Reconcile the `selftest` spec (`openspec/specs/selftest`) to hidden/internal + doctor-arm membership (defer the `openspec/specs/` edit until the spec-consolidation reorg settles, to avoid a concurrent-edit clash)
- [ ] Docs: fold the `selftest` user-guide chapter under doctoring / mark internal; drop it from the public CLI-reference table + `SUMMARY.md` as a top-level command (coordinate with the spec-consolidation §6 docs restructure)

## 2. Diagnostics module (`src/doctor.rs`)
- [ ] Define `CheckStatus { Ok, Warn, Fail }` and a `CheckResult { group, name, status, detail, remedy }`
- [ ] Probe layer: gather env (which git/tmux + versions, in-repo), detected CLIs, parsed config, spec-system resolution + count, bundled-script presence/exec/version, broker bind/port, supervisor gate commands, gitignore + stale-session state
- [ ] Pure check functions (one per group) mapping probed inputs → `Vec<CheckResult>`
- [ ] Environment checks (git/tmux present + min version, in git repo) → ✗ on missing
- [ ] CLIs check (detected roster + `[clis.*]`; none-resolve → ⚠, `NoCLIsFound` early)
- [ ] Config check (present + parses; report `worktree_placement`; unknown/deprecated fields → ⚠; unparseable → ✗)
- [ ] Spec-system check (explicit `--specs-format`/`[specs]` resolution + scanned count; unconfigured → ⚠ with add-`[specs]` guidance)
- [ ] Bundled-scripts check (`sweep`/`broker`/`docs-fetch`.sh present + executable + match embedded version; missing/stale → run `git paw init`)
- [ ] Broker check (enabled → bind/port free/reachable; disabled → informational ✓)
- [ ] Supervisor check (enabled → gate-command binaries on PATH sourced from resolved stack preset + `sweep.sh` installed; disabled → informational ✓)
- [ ] Hygiene check (required `.gitignore` entries incl. `.git-paw/worktrees/` under child placement; stale session / orphaned worktree → ⚠ suggest `git paw purge --stale`)

## 3. Rendering + exit code
- [ ] Human renderer: grouped, ✓/⚠/✗ glyphs, remedy line under each non-✓
- [ ] `--json` renderer over the same `Vec<CheckResult>`; suppress human output
- [ ] Exit code = worst status (any `Fail` → non-zero; else 0)
- [ ] Assert doctor writes nothing (read-only)

## 4. Tests
- [ ] Unit test each check function's ✓/⚠/✗ decision over injected state (tmux missing, not-a-repo, no CLIs, unparseable config, unconfigured spec system, missing/stale script, port busy, gate-binary missing, stale session, missing gitignore entry)
- [ ] Exit-code unit tests (all-✓ → 0; a ✗ → non-zero; ⚠-only → 0)
- [ ] `assert_cmd` integration test: `git paw doctor` and `git paw doctor --json` in a tempfile repo — assert exit code + JSON parses with required fields
- [ ] Guard test: `--help` does NOT advertise `--fix`
- [ ] Export-agnosticism: assert the supervisor check does not hard-code cargo/just verbs (sourced from preset)

## 5. Docs
- [ ] `--help` (about/long_about/examples) for `doctor`
- [ ] README CLI table row for `doctor`
- [ ] New mdBook page (`docs/src/`) documenting doctor + its checks; cross-link from `selftest` page; `mdbook build docs/` passes
- [ ] Configuration reference: note doctor (no new fields)

## 6. Verification (five gates)
- [ ] Gate 1 — `cargo test --no-fail-fast` for doctor's tests
- [ ] Gate 2 — full regression green vs merge-base
- [ ] Gate 3 — spec audit: every `preflight-diagnostics` scenario maps to a test
- [ ] Gate 4 — doc audit: `--help`, README, mdBook consistent
- [ ] Gate 5 — security: read-only confirmed; export-agnostic supervisor check; no secrets
- [ ] `just check` green; `cargo fmt` before commit
- [ ] `openspec validate git-paw-doctor --strict` passes
