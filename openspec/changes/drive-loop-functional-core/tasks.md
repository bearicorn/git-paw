## 1. Seam: effects and the shell's apply

- [ ] 1.1 Decision module (e.g. `src/supervisor/decide.rs`, module + item docs): closed `DriveEffect` enum (record manual decision, publish escalation, record learning, submit text with settle delay, send key, refresh branches after an observed merge, rebase + notify, mark correction cycle, note judgment call — no approval variant), observation / policy / state structs
- [ ] 1.2 The shell's `apply(effect)` routes each effect through the existing ports (`ManualDecisionSink` for manual-decision records; the per-pane claim for keystroke effects) and appends `(effect, outcome)` to an ordered effect log returned in the run result
- [ ] 1.3 Route the existing inline actions through `apply` with no logic change; gate: drive-loop unit suites + `tests/unattended_drive_e2e.rs` unchanged

## 2. Extract the exit summary

- [ ] 2.1 `summarize(&LoopState, merge_states)`: outcome, unmerged escalations, wind-down learning effect, resolved agent states, `N/M branches merged`; merge states gathered by the shell from `MergeCheck`
- [ ] 2.2 Table tests over loop-state fixtures (scenario "The exit summary takes merge truth as an input"); suite green

## 3. Extract the prompt decision

- [ ] 3.1 `decide_prompt(&PromptState, &SurfaceObservation, now)`: CLI-unavailability scan, live-prompt gate, buffered-input case, dedup, then (when the key is new) record manual decision → publish `permission-prompt` escalation → record `tooling_friction` learning, in v0.17.0's order — returning state + effects; no keystroke effect for a live prompt; the shell captures and applies
- [ ] 3.2 Table tests over captured fixtures (live prompt → manual-decision + escalation + learning once per dedup window, no keystroke; repeated prompt inside the window → nothing; not live with buffered input → submit; CLI-unavailable banner → escalation; repeated CLI-unavailable banner (same agent, same marker) inside the window → nothing, as the `drive.rs:1871` dedup does today; non-coding role → nothing) — no fakes, no clock (scenarios "A prompt decision is tested with data alone", "A repeated prompt within the dedup window yields no effect", "A repeated CLI-unavailable banner within the dedup window yields no effect")

## 4. Extract the message decision

- [ ] 4.1 `decide_message(&CorrectionState, &[BrokerMessage], &MessagePolicy)` from `observe_worker_messages` (`drive.rs:1575-1620`): `agent.advanced-main` → refresh-after-merge effect (matched before the sender gate); for coding-agent senders only, gate-failure `agent.feedback` → correction-state update, `agent.question` and merge-candidate `agent.artifact` outside wind-down → `NoteJudgmentCall` (effect-log only); everything else → nothing; the shell polls by cursor and applies
- [ ] 4.2 Table tests over message fixtures (question → judgment call, merge candidate → judgment call, non-candidate artifact → nothing, either during wind-down → nothing, non-coding sender → nothing, gate-failure feedback → correction marked, advanced-main → refresh effect) — no fakes (scenarios "A question and a merge candidate are judgment calls in the message decision", "Messages from non-coding senders yield nothing"); suite green

## 5. Extract the nudge decision

- [ ] 5.1 `decide_nudge(&CorrectionState, &NudgeObservation, &NudgePolicy)`: finished-worker gate, budget/exhaustion, early heads-up, re-engagement text, recovery attempts and the wedged verdict, wind-down suppression — returning state + effects
- [ ] 5.2 Table tests over correction-state fixtures; suite green; `heartbeat_due` / `tick_made_progress` (from `unattended-heartbeat-progress`) stay as they are and the shell keeps calling them

## 6. Guards, docs, gates

- [ ] 6.1 Source audit: the decision module uses no terminal port, broker client, `std::process`, `std::fs` or clock reads; effect-log ordering test (scenario "Effects are applied in order and logged")
- [ ] 6.2 `docs/src/architecture.md` (drive loop as observe → decide → apply), `docs/src/architecture-direction.md` Drive loop row; `mdbook build docs/` succeeds
- [ ] 6.3 Rewrite the worked example in `docs/src/architecture-direction.md` ("Example: approving a permission prompt", the `decide_approval` / `ApprovalDecision::Approve { option }` code) to the escalate-only prompt decision: the pure core returns not-live / escalate / CLI-unavailable / submit-buffered-input, and the shell records the manual decision and publishes the escalation through ports, sending no keystroke for a live prompt; retitle the section to match
- [ ] 6.4 Every scenario maps to a named test; `just check` + `just deny` green; full suite green vs merge-base (`cargo test --no-fail-fast`)
