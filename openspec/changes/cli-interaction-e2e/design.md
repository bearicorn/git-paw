# Design — cli-interaction-e2e

## Context

The interactive surface is covered piecewise (trait mocks for prompt logic, pure formatters
for `init`, two PTY tests for the init specs flow) but never *as a whole*. No test asserts
which prompts appear, in what order, and which are bypassed as a function of config + flags +
TTY. Before the v1.0.0 CLI freeze this whole-surface gating contract should be proven
end-to-end.

## Decisions

### D1 — Promote the PTY helpers into a shared harness

`init_interactive_specs.rs` already contains the pattern: detached tmux session,
`send_keys`, `wait_for_pane` (poll capture-pane for a needle), `wait_for_file`, `capture`,
`tmux_available` skip guard, and `helpers::TmuxTestEnv` socket isolation. Promote these into
a shared module (`tests/support/pty.rs`, or extend `tests/helpers/mod.rs`) so the matrix
tests and the existing init tests share one implementation.

### D2 — One serial `prompt_matrix` binary

Keep the whole matrix in a single `#[serial]` test binary. PTY tests are the flakiest tier
and mutate a shared tmux socket; serialising avoids cross-test races. If the matrix grows
heavy, gate the most expensive combos behind a CI env flag and `log`/document any skipped
combo so coverage isn't silently narrowed.

### D3 — Poll-until-rendered, never fixed sleeps

Synchronise on observable state: poll `capture-pane` until the prompt's marker text renders
(bounded by a generous per-prompt timeout), then send keys. Fixed sleeps are permitted only
as a small settle after a keystroke, never as the primary gate.

### D4 — Assert outcomes, not pixels

`capture-pane` is used only to confirm a prompt is present and to synchronise. Correctness is
asserted on written state — the generated config, `session.json`, and the tmux panes created
— so the tests survive cosmetic prompt-wording changes.

### D5 — Prove wiring/gating, not prompt logic

This layer deliberately does **not** re-test prompt logic already covered by the `Prompter`
/ `SupervisorPrompt` mocks. It proves the keystroke→outcome wiring and the config/flag/TTY
gating those mocks can't reach. The capability spec's scenarios are the gating contract; the
trait-logic capabilities keep their own scenarios.

## Matrix (from the roadmap)

- **init**: supervisor confirm (fresh+TTY vs non-TTY); test-command input (after yes vs
  no/non-TTY); spec-system select (fresh+TTY vs non-TTY commented template); migrate confirm
  (existing config missing `[supervisor]`+TTY vs present/non-TTY).
- **start**: branch picker (¬`--branches`); mode picker (¬`--cli`); uniform CLI picker;
  per-branch CLI picker (×N); supervisor confirm (chain→prompt vs
  `--supervisor`/`--no-supervisor`/`--unattended`/non-TTY).
- **start --from-specs**: spec picker (from-specs+TTY); CLI picker
  (`--cli`/`paw_cli`/`default_spec_cli` short-circuit vs fallthrough); spec-format error when
  neither `--specs-format` nor `[specs]` configured.
- **destructive**: stop confirm (¬`--force`); purge confirm (¬`--force`). (remove is guarded
  by `--force`/dirty-check, not a prompt.)

## Non-goals

- Not pixel/layout testing; not re-testing prompt logic (D5).
- The exact non-TTY behaviour of the `--from-specs` spec picker is confirmed during
  implementation (noted open in the roadmap).

## Risks

- **Flakiness** is the primary risk — mitigated by D2/D3. Any deliberately-skipped combo must
  be logged so the matrix's coverage stays honest.
