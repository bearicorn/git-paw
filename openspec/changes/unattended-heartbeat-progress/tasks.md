## 1. Pure decision (seam first, behaviour-preserving)

- [ ] 1.1 Extract `heartbeat_due(last_progress, now, interval) -> bool` from the bottom of `drive_loop`; call it with `last_progress = start` so behaviour is unchanged
- [ ] 1.2 Confirm `loop_exits_on_heartbeat_when_never_completing` and the rest of the drive-loop tests still pass unchanged

## 2. Observe progress

- [ ] 2.1 Add a narrow tip query to the `MergeCheck` port (`GitMergeCheck` backs it with `git::branch_tip_short`; do not reuse `MergeState.tip`, which is empty for a merged branch); per tick, read each coding branch's tip and compare with the previous tick. A failed read is no observation; a branch's first read is recorded, not counted
- [ ] 2.2 Treat a rise in a coding agent's `resolved_phase` rank as progress
- [ ] 2.3 Add a pure `tick_made_progress(prev_tips, tips, phase_advanced) -> bool` and advance `last_progress` when it holds; switch the exit check to `heartbeat_due(last_progress, …)`. The loop's own escalations, re-escalations, learnings, nudges and buffered-input submits are not inputs to it

## 3. Tests

- [ ] 3.1 Unit, fake clock: `branch_tip_movement_defers_the_heartbeat` — tip moves inside the interval, loop keeps running; exits one full quiet interval later
- [ ] 3.2 Unit, pure table: `progress_kinds_reset_the_heartbeat` — tip move and phase advance reset; changed pane output, a repeated `working` status, a re-escalation of an unanswered prompt and a recorded learning do not
- [ ] 3.3 Unit, fake clock: `re_escalations_do_not_defer_the_heartbeat` — one coding pane shows an unanswered live prompt for longer than the interval; the loop re-escalates it each dedup window and records learnings, and still exits with `heartbeat` one quiet interval after start
- [ ] 3.4 Unit: a fake `MergeCheck` whose tip query errors never counts as progress and never aborts the loop
- [ ] 3.5 E2E: `detached_unattended_run_completes_past_the_heartbeat` in `tests/unattended_drive_e2e.rs` — stdin null, no TTY, short `GIT_PAW_DRIVE_HEARTBEAT_MS`, a dummy agent that commits on a cadence shorter than the interval for longer than it, then publishes a terminal status with its branch merged; assert outcome `completed`, socket- and HOME-isolated, condition-based waits per `core-ci-hygiene`
- [ ] 3.6 Confirm `unattended_heartbeat_exits_with_summary` (idle dummies) still exits with `heartbeat`

## 4. Docs

- [ ] 4.1 `docs/src/user-guide/supervisor.md`: the heartbeat fires after ~25 minutes with no progress (define progress: a branch tip move or a phase advance; the loop's own escalations and learnings do not count), not 25 minutes after launch; note that a progressing wave is no longer capped at 25 minutes, and that a wave whose agents all wait on unanswered prompts still exits on the heartbeat
- [ ] 4.2 "~25-minute heartbeat" → "after ~25 minutes without progress" in the `README.md` unattended paragraph, `docs/src/cli-reference.md` (`start --unattended`) and `docs/src/user-guide/tiered-model-workflow.md`
- [ ] 4.3 Check `start --unattended` `--help` text; update only if it describes the heartbeat (it does not today)
- [ ] 4.4 No configuration-reference change (no new setting)

## 5. Gates

- [ ] 5.1 `cargo fmt`, then `just lint`
- [ ] 5.2 `GIT_PAW_ALLOW_LIVE_SESSION=1 cargo test --no-fail-fast`, verifying the real exit code
- [ ] 5.3 `just deny`; `mdbook build docs/`
- [ ] 5.4 Security and safety review of the extended unattended run window (`security-and-safety-review`): the loop approves nothing, and its nudge and submit keystrokes stay behind the live-prompt gate and the per-pane claim
- [ ] 5.5 `openspec validate unattended-heartbeat-progress --strict`
