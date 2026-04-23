## 1. Payload structs

- [ ] 1.1 Define `VerifiedPayload` in `src/broker/messages.rs` with fields `verified_by: String`, `message: Option<String>`. Derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`
- [ ] 1.2 Define `FeedbackPayload` with fields `from: String`, `errors: Vec<String>`. Derive same
- [ ] 1.3 Add doc comments on both structs and all fields

## 2. BrokerMessage variants

- [ ] 2.1 Add `Verified { agent_id: String, payload: VerifiedPayload }` variant with `#[serde(rename = "agent.verified")]`
- [ ] 2.2 Add `Feedback { agent_id: String, payload: FeedbackPayload }` variant with `#[serde(rename = "agent.feedback")]`
- [ ] 2.3 Update `agent_id()` match to include `Verified` and `Feedback`
- [ ] 2.4 Update `status_label()`: Verified → `"verified"`, Feedback → `"feedback"`

## 3. Validation

- [ ] 3.1 Add `MessageError::EmptyVerifiedBy` variant
- [ ] 3.2 Add `MessageError::EmptyErrors` variant
- [ ] 3.3 Extend `validate()` for Verified: reject empty `verified_by`
- [ ] 3.4 Extend `validate()` for Feedback: reject empty `from`, reject empty `errors` list

## 4. Display

- [ ] 4.1 Add Display arm for Verified: `[{agent_id}] verified by {verified_by}` or `[{agent_id}] verified by {verified_by} — {message}` when message is Some
- [ ] 4.2 Add Display arm for Feedback: `[{agent_id}] feedback from {from}: {N} errors`

## 5. Delivery routing

- [ ] 5.1 Add match arm in `publish_message` for `Verified` → broadcast to all except sender (same as Artifact)
- [ ] 5.2 Add match arm for `Feedback` → deliver to target agent_id only (same routing pattern as Blocked, but target is `agent_id` not `payload.from`)
- [ ] 5.3 Verify both new types update the sender's agent record

## 6. Coordination skill template

- [ ] 6.1 Add a "Messages you may receive" section to `assets/agent-skills/coordination.md`
- [ ] 6.2 Document `agent.verified` — no action needed
- [ ] 6.3 Document `agent.feedback` — read errors, fix, re-publish artifact

## 7. Unit tests — messages.rs

- [ ] 7.1 Verified round-trip serde (with message, without message)
- [ ] 7.2 Feedback round-trip serde
- [ ] 7.3 Validation: empty verified_by rejected
- [ ] 7.4 Validation: empty from in feedback rejected
- [ ] 7.5 Validation: empty errors list rejected
- [ ] 7.6 Display: verified without message
- [ ] 7.7 Display: verified with message
- [ ] 7.8 Display: feedback with 3 errors
- [ ] 7.9 status_label for Verified → "verified"
- [ ] 7.10 status_label for Feedback → "feedback"
- [ ] 7.11 agent_id for both new variants

## 8. Unit tests — delivery.rs

- [ ] 8.1 Verified broadcast reaches all peers, skips sender
- [ ] 8.2 Feedback delivered to target agent only, not to others
- [ ] 8.3 Both new types update sender's agent record

## 9. Integration test

- [ ] 9.1 HTTP POST /publish with valid agent.verified returns 202
- [ ] 9.2 HTTP POST /publish with valid agent.feedback returns 202
- [ ] 9.3 Published verified message appears in peer's inbox via GET /messages
- [ ] 9.4 Published feedback appears in target agent's inbox only

## 10. Skill template test

- [ ] 10.1 Embedded coordination skill contains `agent.verified`
- [ ] 10.2 Embedded coordination skill contains `agent.feedback`

## 11. Quality gates

- [ ] 11.1 `cargo fmt` clean
- [ ] 11.2 `cargo clippy --all-targets -- -D warnings` clean
- [ ] 11.3 `cargo test` — all tests pass
- [ ] 11.4 `just check` — full pipeline green

## 12. Handoff readiness

- [ ] 12.1 Confirm no changes outside `src/broker/messages.rs`, `src/broker/delivery.rs`, `assets/agent-skills/coordination.md`, and test files
- [ ] 12.2 Confirm existing v0.3.0 message tests still pass
- [ ] 12.3 Commit with message: `feat(broker): add verified and feedback message types`
