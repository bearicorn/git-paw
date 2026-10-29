## Why

v1.0.0 freezes exit codes and makes `--from-specs` a hard error. Before that, the codes have to
mean one thing each, every user-facing error has to say what to do next, and the alias has to warn
for at least one release. An inventory of `src/error.rs`, the single `process::exit` call
(`PawError::exit`), clap, and `run_drive_loop` shows:

- **Codes in use:** 0; 1 for every `PawError` except `UserCancelled` and `NotAttempted`; 2 for
  `UserCancelled` (empty picker selection, Ctrl+C at a prompt), for v0.17.0's `NotAttempted`
  (`git paw setup-cli` declined, run without a terminal, or asked for an unsupported item) **and**
  for clap's own argument errors. The docs (`cli-reference.md`, `architecture.md`) say 2 means only
  "user cancelled". `git paw doctor` maps ✗ to 1 through `DoctorFailed`. v0.21.0's `finish` adds
  `IntegrationRefused` at 3.
- **`git paw start --unattended` exits 0** for `escalated-for-review`, `stuck`, `heartbeat` and
  `session-torn-down` — only `cli-unavailable` is non-zero. A script cannot tell a completed wave
  from one that stopped, which is how `--unattended && purge` walked into the GP-21/GP-20
  data-loss chain. Since v0.17.0 every live permission prompt is escalated rather than approved,
  and any escalation makes the outcome `escalated-for-review`, so this now covers every run in
  which a prompt went to a human.
- **Declining `purge` exits 0** ("Purge cancelled."), while an empty picker selection exits 2.
  Purge with no terminal and no `--force` exits 2 with `error: Cancelled.` — the user cancelled
  nothing, and the message does not name `--force`.
- **Errors without context:** `PawError::IoError` is `#[from] std::io::Error`, so a bare `?`
  prints `error: I/O: No such file or directory (os error 2)` with no path
  (`worktree_provision.rs`'s `.env` copy and `.env.local` write, `random.rs`). `CliNotFound`
  names the CLI but no next step. ~360 construction sites carry free-form strings.
- **Spec drift:** seven `Test:` pointers in `core-error-handling` name tests that were deleted as
  tautologies (`test_worktree_error_includes_detail`, `test_debug_derived`, …).
- **`--from-specs`** is a silent alias, and the spec forbids a warning.

## What Changes

- **The exit-code contract**, which v1.0.0 freezes: `0` the command did what was asked; `1` it was
  attempted and failed (including a `doctor` ✗ and an error that stops the drive loop); `2` nothing was attempted —
  an argument error (clap's included), a cancelled selection (`UserCancelled`), or a condition for
  proceeding that was not met (`NotAttempted`, the variant v0.17.0 added: a declined
  proceed-confirmation, a required confirmation with no terminal to show it, an unsupported item);
  `3` the command ran but stopped short of its goal — an `--unattended` run that ended without
  completing its wave (`escalated-for-review`, `stuck`, `heartbeat`, `session-torn-down`), or a
  `finish` integration refused by its merge policy (`IntegrationRefused`, v0.21.0). An
  `--unattended` run that escalated any permission prompt ends `escalated-for-review` and exits `3`
  even when all its work completed; the contract says so wherever exit codes are documented.
  `purge`'s declined and no-terminal confirmations are reported as `NotAttempted`, so no new
  variant is added for them.
  **BREAKING (pre-1.0):** `--unattended` non-completion moves 0 → 3; declining `purge` moves 0 → 2.
- **Actionable errors:** every user-facing error names what failed, the object it failed on, and a
  next step where one exists. I/O errors carry the operation and the path — the context-free
  `#[from] std::io::Error` conversion is removed so a bare `?` no longer compiles. A
  table-driven e2e matrix drives each user-reachable failure through the binary.
- **`--from-specs` warns** on stderr that it is deprecated and will be removed in v1.0.0, by
  registering it in the deprecation registry v0.21.0 introduced (`core-deprecation`) and warning
  through that registry's single helper and raw-argument scan. Parsing and behaviour are otherwise
  unchanged.
- Docs list the contract in every layer that mentions exit codes.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `core-error-handling`: *Actionable error messages for each variant* (rule, I/O context, dead
  `Test:` pointers fixed) and *Exit codes distinguish cancellation from errors* (the 0/1/2/3
  contract, built on v0.17.0's `NotAttempted` text and v0.21.0's `IntegrationRefused`, with the
  `purge` no-terminal rule folded in) MODIFIED; ADDED *I/O failures name the operation and the
  path*.
- `supervisor-unattended-operation`: ADDED *Unattended exit status reflects the wave outcome*.
- `cli-parsing`: *--from-specs is a hidden alias of --from-all-specs* MODIFIED — it now warns.

## Impact

- **Code:** `src/error.rs` (`exit_code()` arms, a `WaveIncomplete` variant mapping to 3, the I/O
  variant carrying operation + path, constructors per `code-standards`); every bare-`?` I/O site the
  compiler then flags; `src/supervisor/drive.rs::run_drive_loop` (return `WaveIncomplete` after
  printing the summary); `src/main.rs` (`cmd_purge` decline and no-TTY paths return
  `NotAttempted`); `src/deprecation.rs`
  (one registry entry for `--from-specs`; the existing post-parse scan then warns for it — no new
  warn function); messages found by the construction-site audit.
- **Tests:** `src/error.rs` table tests; `tests/error_matrix_e2e.rs` (new); `tests/cli_tests.rs::test_purge_cancel_preserves_worktrees`
  (exit 2 + `--force` named); `tests/cross_format_spec_selection.rs::from_specs_alias_emits_no_deprecation_warning_on_stderr`
  (inverted); `tests/unattended_drive_e2e.rs` (exit 0 for a clean completion, exit 3 for
  heartbeat / escalated-for-review, including the v0.17.0 escalation test that asserts exit 0 today
  and a new test for an escalated prompt whose wave otherwise completed);
  `tests/cli_prompt_matrix.rs` purge rows.
- **Docs:** `docs/src/cli-reference.md` Exit Codes, `docs/src/architecture.md`,
  `docs/src/user-guide/supervisor.md` (unattended exit status, including exit 3 after any escalated
  prompt), `docs/src/user-guide/doctor.md`
  (cross-reference), root `--help` `after_help` exit-code line, README if it states exit codes,
  `CHANGELOG` via git-cliff (`!` breaking marker).
- **Enum-variant ripple:** `PawError` gains `WaveIncomplete` (`NotAttempted` exists since v0.17.0,
  `IntegrationRefused` since v0.21.0) and changes the I/O variant's shape —
  every `match` on `PawError` (`exit_code`, tests) is compiler-enumerated. `BrokerMessage` and
  `SpecBackendKind` untouched.
- **Dependencies:** none.
- **Cross-version dependencies:**
  - Builds on v0.21.0 `supervisor-vocabulary-split`, which introduces the deprecation registry,
    the stderr warn helper and the raw-argument scan (`core-deprecation`) this change registers
    `--from-specs` in. Its removal-version guard then forces v1.0.0 to delete the entry.
  - Feeds the v1.0.0 contract freeze (MILESTONE v1.0 scope item 1). v1.0's exit-code table must
    include `3`; v1.0 adopts the same registry; its `--from-specs` removal modifies the requirement
    this change modifies and deletes the registry entry.
  - Builds on v0.17.0 `cli-setup-consent`, which MODIFIES *Exit codes distinguish cancellation
    from errors* to add `NotAttempted` (exit 2); this delta starts from that text and keeps its
    *NotAttempted exit code and message* scenario. v0.21.0 `finish-command` MODIFIES the same
    requirement again to add `IntegrationRefused` (exit 3) on top of v0.17.0's text; this delta
    carries both. Archive order: v0.17.0 → v0.21.0 → this change.
  - Builds on v0.17.0 `classifier-retirement`: every live prompt is escalated and any escalation
    makes the outcome `escalated-for-review`, so a run that escalated a prompt exits 3 even when
    every branch merged. Its rewrite of `unattended_auto_approves_safe_prompt_then_completes` into
    an escalation test removes the clean-completion pointer this change used;
    `unattended_reports_completed_when_branch_is_merged` covers exit 0 instead.
  - Built on the released v0.17.0 `stop-without-confirmation` (removed `stop --force`; scripts
    passing it get clap's exit 2), `purge-unmerged-branch-safety` (reworked the purge confirmation
    flow — this change adds only the exit status), and `cli-unavailable-detection-gaps` (its orchestrator
    `cli-unavailable` exit was retired by v0.18.0 `supervisor-pane-removal`; a coding agent's
    unavailable CLI is an escalation, so its wave exits 3 here).
  - Built on the released v0.18.0 `orchestrator-escalation-handoff` (modified *Exit summary*) and
    `cmd-library-surface` (handlers return structured results; the exit mapping stays at the entry).
  - Pairs with v0.18.0's `unattended-heartbeat-progress` (pulled forward from this release):
    `heartbeat` then means "no progress for the interval", and exits 3.
