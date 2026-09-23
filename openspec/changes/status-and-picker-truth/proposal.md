## Why

Two user-facing outputs say things that are not true.

- **DS-9 — dead agent panes reported as a vanished session.** When the tmux session still
  exists but every agent pane has reverted to a bare shell, `session_liveness_for`
  (`src/tmux/session.rs` ~:150-171) returns `SessionLiveness::Stale`, the same value as "tmux
  session absent". `git paw status` then prints `🔴 stale`, "tmux session no longer exists",
  `Tmux: not running` and JSON `"tmux_running": false` — all false. `doctor` says the receipt's
  "tmux session is gone" and recommends `purge --stale`, which does nothing for this session
  (`purge --stale` probes `has-session` only). *session-state / Liveness reflects dead agent panes,
  not just session existence* already requires the effective status to be **Stopped**, and *Status
  distinguishes stale receipts from active sessions* keeps `stale` for an absent tmux session.
- **DS-8 — `default_cli` is ignored on bare `git paw start`.** README (~:253) says `default_cli`
  pre-selects in the interactive picker; the configuration reference (~:147) says it skips the
  picker. Neither happens: `run_selection` (`src/interactive/resolver.rs` ~:268) has no default
  parameter and calls `select_cli(clis, None)` (~:299), so the picker always starts on the first
  CLI. Only the spec-driven chain (`resolve_cli_for_specs`, ~:385) passes `default_cli`. No spec
  covers bare start. Two more DS-8 defects on the same flow:
  - **Per-branch mode never pre-selects either.** `Prompter::select_cli_for_branch` takes no
    default, and `TerminalPrompter`'s implementation hard-codes `.default(0)`
    (`src/interactive/prompter.rs` ~:67-73), so choosing per-branch mode always starts every
    picker on the first CLI.
  - **A supervised launch throws the picked CLI away.** With no `--branches` and no specs,
    `cmd_supervisor` runs the full mode + CLI picker and then keeps only the branches —
    `selection.mappings.into_iter().map(|(b, _)| b)` (`src/commands/supervisor.rs` ~:471) — so
    every worker launches with the `agent_cli` fallback (`--cli` > `default_cli` > supervisor
    CLI) whatever the user picked.

## What Changes

- `SessionLiveness` gains an `AgentsExited` variant: the tmux session exists, the launch grace
  period has passed, and no agent pane runs a live CLI process. `session_liveness_for` returns it
  where it now returns `Stale`. `Stale` again means only "tmux session absent".
- `DisplayStatus::from_receipt` maps `AgentsExited` to `Stopped`. `git paw status` shows
  `🟡 stopped` with a hint that the agent panes exited and `git paw start` recovers them,
  `Tmux: running`, and JSON `"status": "stopped"`, `"tmux_running": true`.
- `git paw doctor` stops calling such a session stale. It reports ⚠ naming the session, saying its
  agent panes exited, with the remedy `git paw start`.
- `start`, `purge --stale` and `attach` are unchanged. `invalidate_if_stale` and
  `cmd_purge_stale` keep the plain `has-session` probe, so a dead-panes session is still recovered
  by `start` and never purged by `purge --stale`.
- `run_selection` takes a `default_cli: Option<&str>` and passes it to the uniform-mode
  `select_cli` and to every per-branch `select_cli_for_branch`, which gains the same `default`
  parameter (`Prompter` trait, `TerminalPrompter`, test prompters). Its callers (`start.rs` ~:137,
  `supervisor.rs` ~:464) pass `config.default_cli`. The picker is still shown; `default_cli` is only
  its pre-selected entry, which matches the documented `select_cli` default semantics (unknown or
  unset means the first entry).
- A supervised launch that shows the picker honours it: each worker launches with the CLI the
  selection mapped to its branch. The `--cli` > `default_cli` > supervisor-CLI fallback stays for
  paths with no picker (`--branches`).
- README and the configuration reference both say "pre-selects in the CLI picker".

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `session-state`: *Liveness reflects dead agent panes, not just session existence* — dead
  agent panes are a liveness outcome separate from an absent session. Status shows them as
  `stopped` with tmux running, never as `stale`. `purge --stale` leaves them alone.
- `preflight-diagnostics`: *Hygiene check* — "stale" means the tmux session is absent. A session
  whose agent panes exited gets its own ⚠ with a `git paw start` remedy.
- `cli-interactive-selection`: ADDED *Configured default CLI is pre-selected in the interactive
  CLI picker* (both uniform and per-branch mode).
- `supervisor-launch`: ADDED *Supervised launch honours the CLIs chosen in the interactive
  picker*.

## Impact

- `src/tmux/session.rs` — new `SessionLiveness::AgentsExited`; `session_liveness_for_with`
  returns it in place of `Stale` (~:170); doc comments updated. `classify_liveness` and
  `session_liveness[_with]` are unchanged.
- **`SessionLiveness` consumers** (grep `SessionLiveness` / `session_liveness_for` under `src/`):
  - `src/session.rs` `DisplayStatus::from_receipt` (~:92). Its wildcard arm compiles anyway,
    but the new variant must get an explicit arm.
  - `src/commands/status.rs` (~:32-76): `tmux_running`, the hint line and the `Tmux:` line.
  - `src/doctor.rs` `probe_hygiene` (~:1441) and `check_hygiene` (~:902); `HygieneProbe` gains
    a field (the fixture at `src/doctor_tests.rs` ~:90 lists every field and must be updated).
  - `matches!(…, Alive)` sites need no change because `AgentsExited` is "not alive", as `Stale`
    was: `src/commands/start.rs` ~:62, ~:385; `src/commands/supervisor.rs` ~:386;
    `src/commands/pause.rs` ~:34; `src/commands/add.rs` ~:46.
  - Plain-probe sites never see the variant: `src/main.rs` `invalidate_if_stale` ~:1314,
    `cmd_purge_stale` ~:1383; `src/commands/attach.rs` ~:40.
  - Tests: `src/tmux/tests.rs` ~:2581 (now asserts `Stale` and must assert `AgentsExited`),
    `src/session.rs` display tests ~:1088-1145, and `tests/agent_pane_liveness_e2e.rs` (now
    asserts `"status":"stale"`; must change).
- `src/interactive/resolver.rs` — `run_selection` signature and its 13 test call sites;
  `src/commands/start.rs` ~:137 and `src/commands/supervisor.rs` ~:464 callers.
- `src/interactive/mod.rs` — `Prompter::select_cli_for_branch` gains `default: Option<&str>`;
  implementations in `src/interactive/prompter.rs` (`TerminalPrompter`, replacing `.default(0)`
  with the same index lookup `select_cli` uses) and the test prompters in
  `src/interactive/resolver.rs` (`TrackingPrompter` ~:486, `CancelOnSpecsPrompter` ~:1123).
- `src/commands/supervisor.rs` — the no-`--branches`/no-spec arm (~:455-472) keeps
  `selection.mappings` and feeds the chosen CLIs into the per-branch override map that
  `effective_agent_cli` already consults (`spec_cli_overrides`, ~:449/~:537), so dry-run output,
  pane commands, approval flags and the session record all use the picked CLI.
- Docs: `docs/src/cli-reference.md` status table and the `--json` paragraph (~:345-357);
  `docs/src/user-guide/doctor.md` Hygiene row (~:93, which now files dead agent panes under the
  `purge --stale` remedy);
  `docs/src/configuration/README.md` `default_cli` (~:18, ~:145-151); README (~:253) already
  says "pre-select" and only needs a check; `docs/src/user-guide/supervisor.md` for the
  supervised picker now being honoured.
- **Collisions**: no pending change MODIFIES/REMOVES any requirement touched here.
  `bundled-helper-discovery` modifies a different session-state requirement (*Find session by
  repository path*); `supervisor-boot-wiring` ADDs an unrelated session-state requirement;
  `test-session-state-isolation` and `start-spec-launch-parity` touch no session-state
  requirement. In `supervisor-launch` this change only ADDs; `purge-unmerged-branch-safety`
  modifies a different requirement there (*Purge warns about unmerged commits*). `retire-stale-requirements` (sibling) touches `cli-resolution`, not
  `cli-interactive-selection`.
