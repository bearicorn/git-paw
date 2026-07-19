## 1. Monotonic cursor

- [ ] 1.1 Write a failing unit test: `poll_messages` with `since` equal to the latest sequence returns 0 messages and `last_seq == since` (not `0`)
- [ ] 1.2 Change `poll_messages` to return `max(since, highest_returned)` as the cursor
- [ ] 1.3 Update the existing "since equal to latest" unit test expectation (`last_seq` 0 → `since`) and the `poll_messages` rustdoc cursor contract
- [ ] 1.4 Run the message-delivery unit tests — all green

## 2. Mixed-inbox drain (e2e)

- [ ] 2.1 Write a failing e2e: publish an `agent.question` then an `agent.artifact` to the supervisor inbox; poll with `since=0`, advance to `last_seq`, poll again; assert the second poll returns the artifact and the question is not re-returned
- [ ] 2.2 Confirm it passes with the monotonic cursor in place

## 3. Duplicate question suppression

- [ ] 3.1 Write a failing unit test: publishing the identical `(agent_id, question)` twice into an undrained supervisor inbox leaves exactly one copy
- [ ] 3.2 Add the residency dedup check to `Question` routing in `src/broker/delivery.rs`
- [ ] 3.3 Add tests: distinct questions from the same agent both enqueue; identical text from a different agent both enqueue
- [ ] 3.4 Run the message-delivery tests — all green

## 4. Complementary (agent-side, no capability spec)

- [ ] 4.1 Update the `coordination` skill guidance: before re-publishing a `Question`, the asker checks its own inbox for a matching `agent.answer` (shipped v0.11.0) and suppresses the re-publish if present. Respect the skill-content test pins in `skills.rs`/`*_skill_content.rs`

## 5. Verification

- [ ] 5.1 `openspec validate broker-delivery-hardening --strict` passes
- [ ] 5.2 `cargo test broker` green; every scenario maps to a test
