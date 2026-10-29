## 1. Capture tooling

- [ ] 1.1 Write `tests/fixtures/compat/capture.sh <version>`: install the release into a temporary root (`cargo install git-paw --version =<version> --locked --root <tmp>`), create a throwaway repo with a stub agent CLI, run the fixed script (`init`, broker on, `start` two branches, publish broker messages, record one manual approval, `pause`, `stop`), copy the six files out. For a version ≥ 0.17.0, record the manual approval by an `--unattended` run whose stub agent shows one live permission prompt, which the drive loop escalates and logs (the only writer of that log since `classifier-retirement`)
- [ ] 1.2 Placeholder substitution (`{{HOME}}`, `{{REPO}}`, `{{WORKTREE_PARENT}}`, `{{CREATED_AT}}`); fail if `$HOME`, `$USER` or the temporary root survives in any output
- [ ] 1.3 `tests/fixtures/compat/README.md`: how to capture a row, the toolchain used, and the rule that no real config is ever committed

## 2. The released rows (v0.16.0, v0.17.0, v0.18.0)

- [ ] 2.1 Capture `tests/fixtures/compat/0.16.0/` with the script; record the capture in the README
- [ ] 2.2 Capture `tests/fixtures/compat/0.17.0/` with the script, recording the manual approval through an `--unattended` run (task 1.1); record the capture in the README
- [ ] 2.3 Capture `tests/fixtures/compat/0.18.0/` the same way (the first row with the relocated layout: a `broker` service window, no orchestrator pane); record the capture in the README
- [ ] 2.4 Review every captured file by eye for machine-specific content before committing

## 3. Matrix test

- [ ] 3.1 `tests/upgrade_compat_matrix.rs`: walk the rows, substitute placeholders with tempdir paths, install files under a test-owned HOME / data dir with the tmux socket isolated (per `core-ci-hygiene`)
- [ ] 3.2 `every_release_row_loads`: config loads with recorded values and unchanged bytes, exempting keys of the deprecated no-effect tables (`[supervisor.auto_approve]`, `[supervisor.common_dev_allowlist]`; read the list from the deprecation registry once v0.21.0 provides it); `status --json` finds and reports the session; `approvals` aggregates the recorded patterns
- [ ] 3.3 `every_release_row_resumes`: with status `active` and no tmux server, `start` rebuilds one agent surface per recorded worktree with its recorded CLI; `purge --force` then removes worktrees, created branches, discovery file, session file and `broker.log`
- [ ] 3.4 `broker_appends_to_old_logs`: a broker started by the current build appends after the old lines without rewriting them
- [ ] 3.5 `every_released_version_has_a_row`: parse `## [X.Y.Z]` headers in `CHANGELOG.md`; every version ≥ 0.16.0 has a row; name the missing release (passes at this change's tip only with the 0.16.0, 0.17.0 and 0.18.0 rows of task group 2, plus any later release already shipped)
- [ ] 3.6 `fixtures_carry_no_machine_paths`: no fixture contains an absolute path under a real home directory
- [ ] 3.7 File any incompatibility the matrix finds as its own v0.24.0 change; do not fix it here

## 4. Follow-on rows (release process, recorded here so it is not lost)

- [ ] 4.1 After v0.21.0 lands, extend the assertions: unversioned files read as version 1 and the config is not rewritten to migrate it
- [ ] 4.1a From v0.21.0 on, the capture script also records a second session with the broker on and the dashboard not enabled (row `<version>/broker-no-dashboard`); `every_release_row_resumes` asserts it recovers without a dashboard pane, agents from pane 0, even under a config that enables the dashboard (v0.21.0 `dashboard-usage-view`)
- [ ] 4.2 Before the v0.24.0 release candidate, confirm rows exist for every release v0.19.0–v0.23.0 (each release's prep adds its own; the completeness test enforces it)

## 5. Docs

- [ ] 5.1 `AGENTS.md` release section and `CONTRIBUTING.md` / `docs/src/contributing.md`: capture the compat row for the new version before the prep commit
- [ ] 5.2 After archive: replace the archive-generated `## Purpose` placeholder in `openspec/specs/core-upgrade-compatibility/spec.md` if needed, and add the capability to `docs/src/specifications/README.md` (guarded by `tests/specifications_page_lists_every_capability.rs` and `tests/spec_purpose_backfilled.rs`)
- [ ] 5.3 No `--help`, README or configuration-reference change (no user-facing surface)

## 6. Gates

- [ ] 6.1 `cargo fmt`, then `just lint`
- [ ] 6.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code
- [ ] 6.3 `just deny`; `mdbook build docs/`
- [ ] 6.4 `openspec validate upgrade-compat-matrix --strict`
