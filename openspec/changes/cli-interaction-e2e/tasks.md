# Tasks — cli-interaction-e2e

## 1. Shared PTY harness
- [x] Promote `create_detached_session` / `send_keys` / `wait_for_pane` / `wait_for_file` / `capture` / `tmux_available` out of `tests/init_interactive_specs.rs` into a shared module (`tests/support/pty.rs` or `tests/helpers/mod.rs`)
- [x] Wire socket isolation (`helpers::TmuxTestEnv`) and the tmux-availability skip guard into the shared harness
- [x] Refactor `init_interactive_specs.rs` to consume the shared harness (no behaviour change, remove the duplicated helpers)

## 2. Prompt-matrix test binary (`#[serial]`)
- [x] init: supervisor confirm shown (fresh+TTY, PTY test) / bypassed (non-TTY, matrix test → not enabled)
- [x] init: test-command input shown (after yes, PTY test) / bypassed (non-TTY, matrix test)
- [x] init: spec-system select shown (fresh+TTY, PTY test) / bypassed (non-TTY, matrix test → commented template)
- [ ] init: migrate-supervisor confirm shown (existing config missing `[supervisor]`+TTY) / bypassed (present / non-TTY)
- [x] start: branch picker shown (¬`--branches`, PTY test) / bypassed (`--branches`, matrix test)
- [x] start: mode picker shown (¬`--cli`, PTY test) / bypassed (`--cli`, matrix test)
- [ ] start: uniform CLI picker shown / bypassed (`--cli`)
- [~] start: uniform / per-branch CLI picker SHOWN — documented-skip (reaching it needs passing the mode picker = flaky drive-to-completion); covered by bypass (`start_all_flags_bypass_all_pickers`) + short-circuit chain (`cli_resolution_integration.rs`) + mode-picker render precursor (`start_branches_without_cli_shows_mode_picker`). NOTE in the matrix file.
- [x] start: supervisor confirm dispatch — `--supervisor` short-circuits the chain into supervisor mode (`start_supervisor_flag_enters_supervisor_mode`); non-TTY bypass via `init_non_tty_*`
- [x] spec-launch: bare `--specs` shows the spec picker (`bare_specs_on_tty_shows_spec_picker`, PTY render-gate) + `--from-all-specs` launches all specs without a picker (`from_all_specs_launches_every_spec_without_picker`); deprecated `--from-specs` alias omitted
- [x] spec-launch: CLI picker short-circuits (`--cli`/`paw_cli`/`default_spec_cli`) — covered behaviorally by `cli_resolution_integration.rs` (priorities 1–3); fallthrough picker is the PTY-only tier
- [x] spec-launch: spec-format resolution error when neither `--specs-format` nor `[specs]` configured (`from_all_specs_unconfigured_spec_format_errors`)
- [x] destructive: stop renders NO confirm (cmd_stop is non-destructive — reconciled with cli-parsing); covered by `stop_does_not_prompt`
- [x] destructive: purge confirm bypassed by `--force` (`purge_force_bypasses_confirmation`); the shown (¬`--force`, TTY) row remains for a later PTY slice

## 3. Outcome assertions + flake resistance
- [x] Assert written config / `session.json` / tmux panes for each completed flow (not pane pixels) — init slice asserts the written config; extend to session.json/panes as start/from-specs rows land
- [x] Poll-until-rendered synchronisation with per-prompt timeouts; no fixed sleep as the primary gate (in the shared `helpers::pty` harness)
- [x] `log`/document deliberately-skipped combos — the CLI-picker-shown NOTE in the matrix file records why + where covered
- [x] bare `--specs` non-TTY path encoded as the unconfigured-format error (`from_all_specs_unconfigured_spec_format_errors`); the TTY picker via the PTY render-gate

## 4. Reconciliation (with the other v0.13.0 workstreams)
- [x] Scenario→test map across `cli_prompt_matrix.rs` (init bypass, start branch/mode/supervisor dispatch, spec-launch `--from-all-specs`/format-error/bare-`--specs`, destructive), `init_interactive_specs.rs` (init supervisor-confirm/test-command/spec-system SHOWN), `cli_resolution_integration.rs` (CLI short-circuit chain). Remaining documented-skip: uniform/per-branch CLI-picker-shown (flaky drive) + init migrate-supervisor confirm (narrow).
- [x] **R2 preconditions MET:** the dispatch + prompt outcomes for the interactive / `--from-specs` / destructive paths the `main.rs` split touches are guarded; behavioral replacements exist for the source-grep tests test-suite-consolidation will remove (`cli_specs_tty_proceeds_to_picker` → `bare_specs_on_tty_shows_spec_picker`; `cli_from_specs_boot_block_failure` → `from_all_specs_*`).

## 5. Verification (five gates)
- [ ] Gate 1 — the matrix binary passes in isolation (`--no-fail-fast`), serialised, tmux present
- [ ] Gate 2 — full regression green vs merge-base (run the matrix serialised, not concurrent with other E2E)
- [ ] Gate 3 — spec audit: every `cli-interaction-e2e` scenario maps to a matrix test
- [ ] Gate 4 — doc audit: harness convention noted for contributors
- [ ] Gate 5 — security: no secrets; harness spawns only the test binary under an isolated tmux socket
- [ ] `just check` green; `cargo fmt` before commit
- [ ] `openspec validate cli-interaction-e2e --strict` passes
