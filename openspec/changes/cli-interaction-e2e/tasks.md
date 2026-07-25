# Tasks — cli-interaction-e2e

## 1. Shared PTY harness
- [ ] Promote `create_detached_session` / `send_keys` / `wait_for_pane` / `wait_for_file` / `capture` / `tmux_available` out of `tests/init_interactive_specs.rs` into a shared module (`tests/support/pty.rs` or `tests/helpers/mod.rs`)
- [ ] Wire socket isolation (`helpers::TmuxTestEnv`) and the tmux-availability skip guard into the shared harness
- [ ] Refactor `init_interactive_specs.rs` to consume the shared harness (no behaviour change, remove the duplicated helpers)

## 2. Prompt-matrix test binary (`#[serial]`)
- [ ] init: supervisor confirm shown (fresh+TTY) / bypassed (non-TTY → `enabled=false`)
- [ ] init: test-command input shown (after yes) / bypassed (no / non-TTY)
- [ ] init: spec-system select shown (fresh+TTY) / bypassed (non-TTY → commented template)
- [ ] init: migrate-supervisor confirm shown (existing config missing `[supervisor]`+TTY) / bypassed (present / non-TTY)
- [ ] start: branch picker shown (¬`--branches`) / bypassed (`--branches`)
- [ ] start: mode picker shown (¬`--cli`) / bypassed (`--cli`)
- [ ] start: uniform CLI picker shown / bypassed (`--cli`)
- [ ] start: per-branch CLI picker shown ×N / bypassed (`--cli`)
- [ ] start: supervisor confirm shown (chain→prompt) / bypassed (`--supervisor` / `--no-supervisor` / `--unattended` / non-TTY)
- [ ] from-specs: spec picker shown (from-specs+TTY)
- [ ] from-specs: CLI picker short-circuits (`--cli` / `paw_cli` / `default_spec_cli`) vs shown on fallthrough
- [ ] from-specs: spec-format resolution error when neither `--specs-format` nor `[specs]` configured
- [ ] destructive: stop confirm shown (¬`--force`) / bypassed (`--force`)
- [ ] destructive: purge confirm shown (¬`--force`) / bypassed (`--force`)

## 3. Outcome assertions + flake resistance
- [ ] Assert written config / `session.json` / tmux panes for each completed flow (not pane pixels)
- [ ] Poll-until-rendered synchronisation with per-prompt timeouts; no fixed sleep as the primary gate
- [ ] `log`/document any combo deliberately skipped or CI-flag-gated so coverage isn't silently narrowed
- [ ] Confirm the `--from-specs` non-TTY spec-picker behaviour and encode it

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
