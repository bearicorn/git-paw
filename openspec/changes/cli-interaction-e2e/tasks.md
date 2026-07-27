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
- [ ] start: per-branch CLI picker shown ×N / bypassed (`--cli`)
- [ ] start: supervisor confirm shown (chain→prompt) / bypassed (`--supervisor` / `--no-supervisor` / `--unattended` / non-TTY)
- [ ] spec-launch: spec picker shown for bare `--specs` (+TTY); `--from-all-specs` launches all specs without a picker (deprecated `--from-specs` alias omitted — removal at v1.0.0)
- [ ] spec-launch: CLI picker short-circuits (`--cli` / `paw_cli` / `default_spec_cli`) vs shown on fallthrough
- [ ] spec-launch: spec-format resolution error when neither `--specs-format` nor `[specs]` configured (via `--from-all-specs`)
- [x] destructive: stop renders NO confirm (cmd_stop is non-destructive — reconciled with cli-parsing); covered by `stop_does_not_prompt`
- [x] destructive: purge confirm bypassed by `--force` (`purge_force_bypasses_confirmation`); the shown (¬`--force`, TTY) row remains for a later PTY slice

## 3. Outcome assertions + flake resistance
- [x] Assert written config / `session.json` / tmux panes for each completed flow (not pane pixels) — init slice asserts the written config; extend to session.json/panes as start/from-specs rows land
- [x] Poll-until-rendered synchronisation with per-prompt timeouts; no fixed sleep as the primary gate (in the shared `helpers::pty` harness)
- [ ] `log`/document any combo deliberately skipped or CI-flag-gated so coverage isn't silently narrowed
- [ ] Confirm the bare `--specs` non-TTY spec-picker behaviour and encode it

## 4. Reconciliation (with the other v0.13.0 workstreams)
- [ ] Cross-check the matrix against the spec-audit traceability pass so each scenario maps to a covering test
- [ ] Hand the test-consolidation workstream the subsumed set (`stop_confirmation_test.rs`, `cli_specs_tty_proceeds_to_picker.rs`, `cli_from_specs_boot_block_failure.rs`, `init_interactive_specs.rs` fold-in) — cut those only after this lands

## 5. Verification (five gates)
- [ ] Gate 1 — the matrix binary passes in isolation (`--no-fail-fast`), serialised, tmux present
- [ ] Gate 2 — full regression green vs merge-base (run the matrix serialised, not concurrent with other E2E)
- [ ] Gate 3 — spec audit: every `cli-interaction-e2e` scenario maps to a matrix test
- [ ] Gate 4 — doc audit: harness convention noted for contributors
- [ ] Gate 5 — security: no secrets; harness spawns only the test binary under an isolated tmux socket
- [ ] `just check` green; `cargo fmt` before commit
- [ ] `openspec validate cli-interaction-e2e --strict` passes
