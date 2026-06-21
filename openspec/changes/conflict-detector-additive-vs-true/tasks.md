## 1. Classification core

- [ ] 1.1 In `src/broker/conflict.rs`, add an additive-vs-true classifier for an in-flight triple `(a, b, file)` that reads `a`'s and `b`'s active `IntentRecord.files[file]` regions and returns `True` when either side lacks an active intent for `file`, either declares `file` at file level (`regions == None`), or both declare regions that intersect via the existing `regions_intersect`; returns `Additive` only when both declared regions and the sets are disjoint
- [ ] 1.2 Add the informational additive-downgrade `agent.feedback` error-text builder (prefixed `[conflict-detector]`, indicating "shared file, additive — resolve at merge", with the file path and both agent_ids), reusing `CONFLICT_DETECTOR_TAG` / `CONFLICT_DETECTOR_SENDER`

## 2. Wire classification into the escalation tick

- [ ] 2.1 Change the window-elapsed escalation path (`take_due_escalations` and its broker-side consumer) so a due triple is classified before acting: True → emit the existing `agent.question` to the supervisor inbox; Additive → emit exactly one downgrade `agent.feedback` to both agents and NO `agent.question`
- [ ] 2.2 Mark the triple's escalation decision as made in both branches (reuse the `escalated` flag; update its doc comment to mean "decision made/acted on") so neither the question nor the additive feedback re-emits on later ticks while regions are unchanged
- [ ] 2.3 Confirm the triple stays in `in_flight_pairs` after a downgrade (only `sweep_in_flight_pairs` removes it when an agent stops touching the file) so the additive overlap is recorded, not dropped

## 3. Tests (one per spec scenario)

- [ ] 3.1 True collision (same anchor / overlapping ranges) after the window → `agent.question` to supervisor inbox; triple marked decided
- [ ] 3.2 Additive overlap (disjoint declared ranges) after the window → no `agent.question`; one informational `agent.feedback` containing the tag, additive/resolve-at-merge wording, and the file path
- [ ] 3.3 Additive downgrade is recorded and not re-emitted: triple remains in the tracker, and subsequent ticks emit neither additive feedback nor a question
- [ ] 3.4 Conservative fallback: no regions / file-level intents → still escalates as `agent.question`
- [ ] 3.5 Regression: existing in-flight tests (initial warning, escalate-once, resolve-on-stop) still pass

## 4. Docs and gates

- [ ] 4.1 Update the conflict-detector module doc comment in `src/broker/conflict.rs` and any user-guide / dashboard prose describing in-flight escalation to mention additive-vs-true downgrade
- [ ] 4.2 Run `just check` (fmt + clippy + tests) and `just deny`; confirm no `unwrap()`/`expect()` in non-test code and all public items have doc comments
