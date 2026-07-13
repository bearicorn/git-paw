## 1. Message type

- [ ] 1.1 Add `AnswerPayload { from, answer, re: Option<String> }` and the `Answer` variant (`#[serde(rename = "agent.answer")]`) to `src/broker/messages.rs`; serde round-trip tests incl. `re` omission
- [ ] 1.2 Extend `validate()` (+ `MessageError` variants for empty from/answer), `agent_id()`, `status_label()`, `Display`; update the variant-count test to eleven

## 2. Server + delivery

- [ ] 2.1 `src/broker/server.rs`: `check_placeholder_fields` arm for the new payload
- [ ] 2.2 `src/broker/delivery.rs`: `message_sender()` returns `payload.from`; `route_message()` routes to the target agent's inbox; record-update filter unchanged unless required; tests: targeted delivery, no phantom roster row

## 3. E2E

- [ ] 3.1 Integration test: HTTP publish `agent.answer` → target agent poll receives it; other agents do not (cross-module E2E rule)

## 4. Skills + docs

- [ ] 4.1 coordination.md: document `agent.answer` under "messages you receive" (authoritative reply, not an error); supervisor.md: answer via `agent.answer`, feedback reserved for corrective errors
- [ ] 4.2 Update pinned skill-content tests for both files
- [ ] 4.3 mdBook coordination chapter message-type table + `mdbook build docs/` passes
