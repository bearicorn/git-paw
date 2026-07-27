## Why

git-paw's interactive prompt *logic* is well covered — the launch pickers sit behind the
`Prompter` trait (mock-tested), the supervisor-mode chain behind `SupervisorPrompt`
(`StubPrompt`-tested), and `init` extracts pure formatters (`specs_section_for`,
`supervisor_section`) with unit tests; two PTY tests already exist
(`tests/init_interactive_specs.rs`). But **nothing asserts the surface as a whole** —
*which* prompts appear, *in what order*, and *which are bypassed* as a function of config +
flags + TTY. The v1.0.0 CLI freeze should rest on a proven interactive contract, not manual
QA. This change closes that gap with true end-to-end tests before the freeze.

## What Changes

- A reusable **PTY-driver harness** promoted from the ad-hoc helpers in
  `init_interactive_specs.rs` (`create_detached_session` / `send_keys` / `wait_for_pane` /
  `wait_for_file` / `capture`): spawn the real binary in a detached tmux pane, poll until a
  prompt renders, send keys, and assert on observable outcomes.
- A **parameterised prompt-matrix test** per command family (`init`, `start`,
  `start --from-all-specs`, destructive confirmations) asserting **prompt presence, order, and
  bypass** for each config/flag/TTY precondition.
- Assertions on **outcomes** (written config, `session.json`, tmux panes created), using
  `capture-pane` only for prompt-presence + synchronisation — never pixel-matching.
- Flake resistance: poll-until-rendered (no fixed sleeps as the gate), per-prompt timeouts,
  tmux-availability skip guard, `#[serial]`, socket isolation via `helpers::TmuxTestEnv`.
- A new **capability spec (`cli-interaction-e2e`)** enumerating the matrix rows as WHEN/THEN
  scenarios — the end-to-end gating contract, distinct from the prompt *logic* the trait
  mocks already cover.

Test-and-spec only: no change to any command, flag, config field, or wire format.

## Capabilities

### New Capabilities
- `cli-interaction-e2e`: the observable interactive-prompt **gating contract** — which
  prompts render vs. are bypassed as a function of config, flags, and TTY, proven
  end-to-end through a PTY driver.

### Modified Capabilities
_None._ This layer proves the keystroke→outcome wiring and gating; it does not restate the
prompt logic specified by `interactive-selection`, `supervisor-cli`, `cli-parsing`, and
`from-specs-launch`. (Reconcile against those during the v0.13.0 spec-audit + consolidation
workstream so the e2e contract and the trait-logic specs are counted together, not
duplicated.)

## Impact

- **Tests:** a shared PTY harness (extend `tests/helpers/mod.rs`, or a new
  `tests/support/pty.rs`); a serial `prompt_matrix` test binary; absorbs the duplicated
  helpers currently in `init_interactive_specs.rs`.
- **Code:** none expected. If a prompt proves unobservable/unsynchronisable end-to-end, the
  fix is a test-only affordance (e.g. a stable prompt marker), not a behaviour change.
- **Docs:** none user-facing; note the harness convention in CONTRIBUTING / test docs.
- **Reconciliation:** the v0.13.0 test-consolidation workstream must treat this matrix as
  coverage — several redundant per-field/per-flag unit tests it flags are subsumed here, so
  cut only after this lands (`stop_confirmation_test.rs`, the source-grep picker tests, and
  `init_interactive_specs.rs` fold in).
- **Risk:** PTY tests are the suite's flakiest tier — keep the matrix in one `#[serial]`
  binary; `log`/document any combos deliberately skipped so coverage isn't silently
  narrowed.
