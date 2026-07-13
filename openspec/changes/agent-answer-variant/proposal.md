## Why

W15-11: the broker has no clean shape for a non-error supervisor→agent reply. Answering an `agent.question` today means abusing `agent.feedback`, whose `FeedbackPayload` REQUIRES a non-empty `errors: Vec<String>` — a plain answer ("yes, use the existing helper") must masquerade as an error list, which agents then mis-read as corrective feedback. `/tell` covers the interactive case, but v0.9.0's broker-mediated approvals made the in-band question→answer loop a first-class flow; it deserves a first-class message.

## What Changes

- New broker message variant `agent.answer` (eleventh `BrokerMessage` variant) with `AnswerPayload { from, answer, re: Option<String> }` — `from` and `answer` required non-empty, `re` an optional short reference to the question being answered.
- Delivery routes `agent.answer` to the TARGET agent's inbox (the envelope's `agent_id`), mirroring `agent.feedback` routing.
- Validation, placeholder checks, display/accessor arms, and the variant-count test extended accordingly.
- Bundled skills document the shape: coordination.md's "messages you receive" section (agents treat an answer as an authoritative supervisor reply, not an error to fix); supervisor.md instructs answering questions via `agent.answer`, reserving `agent.feedback` for corrective errors.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `broker-messages`: ADDED requirement — the `agent.answer` message type (wire shape, payload validation).
- `message-delivery`: ADDED requirement — answer routing to the target agent's inbox.

## Impact

- `src/broker/messages.rs`: enum variant + `AnswerPayload` + `validate()` + `agent_id()`/`status_label()`/`Display` arms + variant-count test (10 → 11).
- `src/broker/server.rs`: `check_placeholder_fields` arm; publish path unchanged otherwise.
- `src/broker/delivery.rs`: `message_sender()`, record-update filter, `route_message()` (target-inbox routing like Feedback).
- `assets/agent-skills/coordination.md` + `supervisor.md`: message documentation — ⚠ pinned by skill-content tests.
- E2E: publish → poll round-trip test (cross-module rule).
- Backward compatible: additive variant; existing publishers/consumers unaffected.
