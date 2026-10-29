## 1. Observation seam (tmux unchanged)

- [ ] 1.1 Rebase over `headless-backend-adapter` and the archived v0.18.0 `drive-loop-functional-core`, v0.17.0 `idle-pane-nudge-submission` / `cli-unavailable-detection-gaps`, v0.19.0 `worker-usage-budget`, v0.18.0 `unattended-heartbeat-progress`; confirm the decision and effect names this change extends
- [ ] 1.2 `SurfaceObservation` gains `interactive` and `activity`; the shell fills them from the port; tmux observations are `interactive = true`, `Unknown`
- [ ] 1.3 Gate: every existing drive-loop decision table, the unattended-drive e2e suite and the tmux suites pass unchanged

## 2. Prompts and nudges on non-interactive surfaces

- [ ] 2.1 `decide_prompt`: a non-interactive observation returns no effects and an unchanged state (no `permission-prompt` escalation, manual-decision record or learning); buffered-input row never fires there
- [ ] 2.2 Shell: `SubmitText` on a non-interactive surface records done/failed from the port result; no settle delay, re-capture, clear-line recovery or `nudge-wedged` path
- [ ] 2.3 Tests: decision table rows (prompt-shaped headless tail → no effect, no escalation; tmux live-prompt row still escalates with no keystroke); fake-backend sweep records no operation and publishes no escalation for a headless surface; re-engagement is text + `Enter` and logged done; a failing `Enter` is logged failed with no clear-line key

## 3. Turn outcomes

- [ ] 3.1 Pure `decide_turn` with `TurnState` (per-agent escalated failed turn, interruption count, last no-progress window) and `TurnObservation` (activity, output tail, resolved unavailable markers, now)
- [ ] 3.2 Escalation labels `turn-failed`, `turn-interrupted`, `no-progress` on the existing `Question` escalation path; `cli-unavailable` reuses the (agent, marker) dedup; continuation text from `assets/headless-continue.md`
- [ ] 3.3 Tests: the seven-row turn table; a repeated failed observation escalates once; fake backend busy-without-output for two windows → no kill, one `no-progress` per window; confirm the v0.18.0 heartbeat decision is unchanged (a turn boundary, a wake or a continuation does not reset it)

## 4. Inbox wake in the broker host

- [ ] 4.1 Delivery layer exposes, per worker, the highest sequence of a waiting addressed message; pure `wakes_worker(&BrokerMessage)` as an exhaustive match (`Feedback`, `Answer`, `Blocked` wake; every other variant does not)
- [ ] 4.2 Pure `decide_wake(&WakeState, &WakeObservation, now)`: idle + newer addressed message → `SubmitText(wake text)`; record the sequence; suppressed after a logged budget-exhausted escalation; never for an interactive backend
- [ ] 4.3 Wake loop as its own thread in the `__broker` host with a fixed tick of at most 15 s (not read from the deprecated `[supervisor.auto_approve]` keys; there is no approver thread since v0.17.0), in attended and unattended sessions, applying effects through the terminal port
- [ ] 4.4 Tests: decision table (idle+new → wake; busy → none; broadcast → none; same message twice → one wake; budget breach → none); integration with the headless adapter and a stub CLI: an `agent.answer` starts a wake turn within 15 s, an `agent.artifact` / `agent.intent` starts none; a tmux-backed session receives no wake text

## 5. Assets and boot block

- [ ] 5.1 `assets/headless-boot-addendum.md`, `assets/headless-continue.md`, `assets/headless-wake.md`, project-agnostic, no helper-script path; loaded with `include_str!`
- [ ] 5.2 Boot-prompt rendering appends the addendum for non-interactive surfaces only
- [ ] 5.3 Tests: headless boot prompt contains the addendum; tmux boot prompt byte-identical to before; no-language-leak audit covers the three assets; the texts are loaded from assets
- [ ] 5.4 `export-agnosticism` review of the three assets

## 6. Purity guard, docs, gates

- [ ] 6.1 Source audit: the headless decision module uses no terminal port, broker client, `std::process`, `std::fs`, `Instant::now` or `SystemTime::now`
- [ ] 6.2 AGENTS.md §7: add the `wakes_worker` match to the `BrokerMessage` compile-forcing list (maintainer edit at implementation time)
- [ ] 6.3 `docs/src/user-guide/supervisor.md` (coordinated mode) and the unattended chapter: a short headless section (no prompts to escalate on a headless worker, its CLI's non-interactive policy decides; messages as turns; inbox wake; the three escalation labels; nothing is killed); `mdbook build docs/` succeeds
- [ ] 6.4 Every scenario maps to a named test; `just check` and `just deny` green; full suite green against the merge-base with `cargo test --no-fail-fast`; exit codes verified with `cmd; echo $?`
