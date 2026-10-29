## 1. Rebase on the pre-freeze cycles

- [ ] 1.1 Take v0.24.0's errors-and-exit-codes outcome: if any code or outcome mapping changed, update the table in `specs/core-stability-contract/spec.md` and its scenarios before writing code
- [ ] 1.2 Take v0.21.0's config DTO, wire module and `--json` schemas as the sources for the manifest's config and format sections; add `git paw finish`'s row to the exit-code table
- [ ] 1.3 Take v0.23.0's support-tier table as the source of the first-class list
- [ ] 1.4 Resolve the replay gate (design.md Open Questions): freeze `replay`, or hide + register it

## 2. Seams (no behaviour change)

- [ ] 2.1 Add the exit-code `const` table (code, meaning) beside `error::exit_code`; doc-comment the frozen meanings; no value changes
- [ ] 2.2 `src/deprecation.rs` (exists since v0.21.0 `supervisor-vocabulary-split`): add the policy check that an entry deprecated in a 1.x release names a later major (`X.0.0`) or no removal release; module docs state the policy
- [ ] 2.3 Unit row for the policy check — covers "A 1.x deprecation schedules its removal at a major"

## 3. Manifest and guard

- [ ] 3.1 Write `tests/contract_manifest.rs` generating each code-backed section (clap walk skipping hidden, config schema, exit codes, MCP `tools/list`, broker message types, first-class CLIs, deprecations) and diffing against `docs/src/stability-manifest.txt`; opt-in regeneration via `GIT_PAW_BLESS_CONTRACT=1`
- [ ] 3.2 Guard rules: missing entry under the same major fails naming the entry and the deprecation policy; unrecorded live entry fails naming it; removal at a new major passes only for registered deprecations
- [ ] 3.3 Probe the hand-maintained entries: each broker route answers non-404 on a test broker; each bundled helper's `--help` exits 0
- [ ] 3.4 Generate the first manifest (`contract-major: 1`) only after `remove-supervisor-vocabulary` and `remove-from-specs-alias` are applied, and review it line by line against the stability chapter's surface classes
- [ ] 3.5 Test: no hidden subcommand (`__dashboard`, `__prompt-profile`, `selftest`, any `__*`) appears in the command section
- [ ] 3.6 Exclude parse-only, no-effect configuration keys (`[supervisor.auto_approve]`, `[supervisor.common_dev_allowlist]`) from the generated configuration section even if the schema still carries them; test that the configuration section has no `auto_approve` / `common_dev_allowlist` entry and the deprecations section lists both with no removal release — covers "A deprecated no-effect table is listed only under deprecations"
- [ ] 3.7 Test: the command section contains `start --coordinated` and `start --bare`, the configuration section contains `default_mode` and the `[orchestrator]` / `[workers]` / `[drive]` keys, and no entry outside the deprecations section contains `supervisor` — covers "The coordinated-session entries are stable" and "No supervisor entry is frozen"

## 4. Exit-code contract tests

- [ ] 4.1 `tests/exit_code_contract.rs` with one row per table entry, asserting the real process exit status: `--help`/`--version` → 0; `stop` with no session → 0; `status` outside a repo → 1; `start --no-such-flag` → 2 with no worktree, session file or tmux session created
- [ ] 4.2 doctor rows: a ✗ environment exits 1 with and without `--json`; a ⚠-only environment exits 0 (extend `doctor_e2e.rs` if it already covers one side)
- [ ] 4.3 Cancellation row: empty branch selection on an interactive `start` exits 2 (PTY harness, `tests/helpers/pty.rs`)
- [ ] 4.4 Unattended rows: `completed` exits 0; `escalated-for-review`, `stuck`, `heartbeat` and `session-torn-down` exit 3, including a run that escalated a permission prompt and then merged every branch, and a run whose coding agent's CLI became unavailable (escalated, so `escalated-for-review`) (drive-loop e2e with the short-poll env overrides)

## 5. Deprecation registry at the freeze

- [ ] 5.1 Render the registry (from `deprecation::REGISTRY`) in the manifest's deprecations section and the stability chapter — covers "The registry is published"
- [ ] 5.2 Test at crate version `1.0.0`: no registry entry has a removal release of `1.0.0` or earlier (the `core-deprecation` guard, run as part of the freeze checks) — covers "The 1.0.0 registry has no overdue entry"
- [ ] 5.3 Test: the stability chapter's "Removed in 1.0.0" list names each removed item with its replacement — covers "The removals are listed with their replacements"
- [ ] 5.4 The warning behaviour (once per process, stderr only, stdout and exit code unchanged) and doctor's deprecated-key ⚠ are `core-deprecation` / `preflight-diagnostics` requirements from v0.21.0; confirm their tests still pass, add none here

## 6. Docs

- [ ] 6.1 Write `docs/src/stability.md`: semver rules, stable surface classes, unstable surfaces, the exit-code table, format evolution, support tiers, the coordinated-session / orchestrator / worker / drive-loop vocabulary, the deprecation policy and registry, the "Removed in 1.0.0" list, `{{#include stability-manifest.txt}}`; add to `SUMMARY.md`
- [ ] 6.2 `docs/src/cli-reference.md` Exit Codes: replace the three-row table with the frozen table including argument errors and the per-command rows; link the stability chapter
- [ ] 6.3 `docs/src/supported-clis.md`: state that the tier definitions and first-class list are frozen; link the chapter
- [ ] 6.4 README: one line linking the stability chapter from the CLI section
- [ ] 6.5 Crate-level rustdoc (`src/lib.rs`): state the library API is outside the semver contract
- [ ] 6.6 AGENTS.md release flow: before tagging, regenerate the manifest, confirm every removal is a registered deprecation at a major bump, and that each deprecation has warned for at least two minor releases
- [ ] 6.7 Backfill `## Purpose` in `openspec/specs/core-stability-contract/spec.md` at archive (text in proposal.md) and add the capability to the specifications page

## 7. Gates

- [ ] 7.1 Every scenario in the delta spec maps to a test named in 2–5
- [ ] 7.2 `cargo fmt`, then `just lint`
- [ ] 7.3 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, checking the real exit code
- [ ] 7.4 `just deny`
- [ ] 7.5 `mdbook build docs/`
- [ ] 7.6 `openspec validate cli-contract-freeze --strict`
