# Tasks — session-orchestration-robustness

## 1. Launch-readiness gate (G1)

- [x] 1.1 Add a pane-readiness poll helper in `src/tmux.rs` that runs
  `tmux capture-pane -p -t <session>:0.<idx>` on a short interval and
  classifies the pane as CLI-ready vs bare-shell, with a bounded timeout
  (reuse the `capture-pane` mechanism from `src/supervisor/poll.rs`).
- [x] 1.2 Wire the readiness gate into `cmd_start` (src/main.rs ~1358):
  replace the blind `sleep(2s)`-then-inject with poll-until-ready, then
  inject the boot block; apply per agent pane (and the supervisor pane).
- [x] 1.3 On readiness timeout with a still-bare-shell pane, relaunch the
  CLI command (clear input with `C-u` first) and re-poll, up to a bounded
  relaunch-attempt budget.
- [x] 1.4 Conservative fallback: when no readiness marker matches within
  the budget (unrecognised CLI), inject after the budget rather than
  failing the launch.
- [x] 1.5 Apply the same readiness gate on the `git paw add` launch path
  (`cmd_add` / `build_add_agent_commands` caller) so an added agent is
  protected identically.

## 2. Remove kills by resolved pane id (G2a)

- [x] 2.1 Add a worktree→pane resolver in `src/tmux.rs` using
  `tmux list-panes -F '#{pane_id} #{pane_current_path}'`.
- [x] 2.2 In `cmd_remove` (src/main.rs ~2536), replace the JSON-position
  `pane_idx`/`kill_pane(idx)` with: resolve the removed branch's worktree
  to a live pane and kill it by `#{pane_id}`, regardless of the running
  process (shell or CLI); no-op idempotently if no pane maps.

## 3. Add/remove pane preservation + reconciliation (G2b)

- [x] 3.1 Ensure the `add` re-tile (`build_add_agent_commands`) and the
  `remove` re-tile (`build_remove_retile_commands`) preserve every OTHER
  agent's pane — no pane dropped or orphaned during re-tile.
- [x] 3.2 Add a JSON↔tmux reconciliation helper that compares
  session-JSON agents against the live `pane_current_path` set and reports
  any JSON agent with no live pane.
- [x] 3.3 Run the reconciliation on the `add` path after the re-tile and
  surface any divergence to the user.

## 4. Equal-width agent-row rebalancing (G3)

- [x] 4.1 Add a per-row equal-width rebalance in `src/tmux.rs`: after a
  row's agent panes are created, scope `select-layout even-horizontal` to
  that row's agent panes OR `resize-pane` each to `100/agents_in_row`%.
- [x] 4.2 Apply the rebalance from `build_supervisor_session`,
  `build_add_agent_commands`, and `build_remove_retile_commands` so all
  paths yield equal-width rows.
- [x] 4.3 Verify the rebalance does NOT alter the top-row
  supervisor/dashboard 50/50 split nor the per-row vertical heights set by
  `push_supervisor_resize_pass`.

## 5. Minimum pane width at high N (G3)

- [x] 5.1 Confirm no agent row exceeds `SUPERVISOR_AGENTS_PER_ROW` (5)
  panes, bounding the equal-width target to a 20% floor per pane; assert
  this directly rather than adding a new minimum-width knob.

## 6. Regression tests (the tests that would have caught G2/G3)

- [x] 6.1 Applied-layout width test (live tmux): build/apply a 3-agent
  supervisor layout, query real pane widths via
  `tmux list-panes -F '#{pane_width}'`, assert the 3 agent panes are equal
  within a one-column tolerance and the row is NOT 50/25/25.
- [x] 6.2 Applied-layout width tests: top row stays ~50/50; a 5-agent row
  renders ~20% per pane.
- [x] 6.3 Real-session pane-integrity test (live tmux): drive a session
  through `git paw add` and assert pane count == JSON-agent count
  (+supervisor+dashboard) and every surviving agent's worktree→pane
  `pane_current_path` mapping is intact (would have caught G2's dropped
  pane).
- [x] 6.4 Real-session pane-integrity test (live tmux): `git paw remove`
  of a middle agent kills only that pane, no collateral loss, grid
  re-flows; include a case where the removed agent's pane is a bare shell.
- [x] 6.5 Reconciliation unit test: a JSON agent with no live pane is
  reported divergent; a fully-mapped session reports none.
- [x] 6.6 Launch-readiness test: boot block is not injected while the pane
  is a bare shell; relaunch fires on timeout; unrecognised-CLI fallback
  injects after the budget.
- [x] 6.7 Reuse the E2E-isolation conventions in `tests/`
  (`add_remove_e2e.rs`, `e2e_supervisor_launch.rs`): isolated tmux socket,
  serialized runs, real-repo tempdirs.

## 7. Docs

- [x] 7.1 Update the tmux/session-management mdBook chapters (`docs/src/`)
  to note the launch-readiness gate and equal-width agent-row behaviour at
  higher N.
- [x] 7.2 Update `--help`/README only if the CLI surface changes (it does
  not by default — pure robustness).
- [x] 7.3 `mdbook build docs/` succeeds.

## 8. Quality gates

- [x] 8.1 `just check` (fmt + clippy + all tests) passes.
- [x] 8.2 `just deny` passes.
- [x] 8.3 No `unwrap()`/`expect()` in non-test code; all public items have
  doc comments.
- [x] 8.4 Coverage >= 80% on logic (live-tmux layout/geometry assertions
  count toward the pane-integrity/width regression coverage; TUI draw
  loops remain exempt).

## 9. Test-port collision fix (F8 corrected root cause)
- [x] 9.1 `tests/e2e_supervisor_stop.rs::pick_broker_port` → OS-assigned ephemeral port (bind 127.0.0.1:0, read back) instead of `24_000 + (pid % 200)`; done in v0.8.0 directly
- [ ] 9.2 Migrate the remaining ~14 test files using `BASE + (pid % N)` broker ports to the same ephemeral scheme (broker.rs, broker_integration.rs, mcp_e2e.rs, learnings_mode_integration.rs, hook_integration.rs, conflict_detection_integration.rs, broker_agent_id_validation.rs, e2e_learnings_*.rs, e2e_qualitative_learnings.rs, …) — ideally via a shared `tests/helpers.rs::free_port()` helper
