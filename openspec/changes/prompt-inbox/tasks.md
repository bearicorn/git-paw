## 1. Add agent.question message type

- [ ] 1.1 Add `QuestionPayload` struct with `question: String` field to `src/broker/messages.rs`
- [ ] 1.2 Derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize` on `QuestionPayload`
- [ ] 1.3 Add `Question { agent_id: String, payload: QuestionPayload }` variant to `BrokerMessage` enum
- [ ] 1.4 Add `"agent.question"` as the serde discriminator value
- [ ] 1.5 Extend `Display` impl for `BrokerMessage::Question`: `[{agent_id}] question: {question}`
- [ ] 1.6 Extend `agent_id()` helper to return `agent_id` for `Question` variant
- [ ] 1.7 Extend `status_label()` helper to return `"question"` for `Question` variant
- [ ] 1.8 Add validation: `question` field MUST NOT be empty

## 2. Add question routing to message delivery

- [ ] 2.1 In `src/broker/delivery.rs` (or equivalent), add routing rule for `BrokerMessage::Question`
- [ ] 2.2 Route question messages to the `"supervisor"` inbox
- [ ] 2.3 Create the `"supervisor"` inbox if it does not exist (lazy registration, same as other agents)
- [ ] 2.4 Do NOT enqueue in the sender's inbox or any other agent's inbox
- [ ] 2.5 Question messages SHALL still be stored in the message log (like all other messages)

## 3. Define QuestionEntry struct

- [ ] 3.1 Add `pub struct QuestionEntry` to `src/dashboard.rs` (or a new `src/inbox.rs` if complexity warrants)
- [ ] 3.2 Fields: `agent_id: String`, `pane_index: usize`, `question: String`, `seq: u64`
- [ ] 3.3 Derive `Debug` and `Clone`
- [ ] 3.4 Add doc comment explaining the pane_index field's purpose

## 4. Extend run_dashboard with multi-section layout

- [ ] 4.1 Extend `run_dashboard` signature to accept an optional pane map: `pane_map: HashMap<String, usize>`
- [ ] 4.2 Add `questions: Vec<QuestionEntry>` and `focused_question: Option<usize>` to dashboard state
- [ ] 4.3 Add `input_buffer: String` to dashboard state
- [ ] 4.4 Update the ratatui layout to three sections: status table (flex), prompts (fixed ~8 lines), input (fixed 3 lines)
- [ ] 4.5 Render prompts section title "Questions (N pending)" with question entries, focused entry prefixed with `>`
- [ ] 4.6 Render input field: `Reply to <agent_id>>` prefix followed by buffer content and cursor `_`
- [ ] 4.7 Show "(no pending questions)" when `questions` is empty

## 5. Implement supervisor inbox polling

- [ ] 5.1 In the dashboard tick loop, after `agent_status_snapshot`, call `poll_messages(&state, "supervisor", last_question_seq)`
- [ ] 5.2 For each new `BrokerMessage::Question` in the result, create a `QuestionEntry` and push to `questions`
- [ ] 5.3 Update `last_question_seq` from the poll result
- [ ] 5.4 Non-question messages in the supervisor inbox SHALL be ignored by the question poller

## 6. Implement key handling

- [ ] 6.1 In the event loop, match on `KeyCode::Tab` → advance `focused_question` (wrap around)
- [ ] 6.2 Match on `KeyCode::Enter` → if `input_buffer` is non-empty and a question is focused, send reply
- [ ] 6.3 Match on `KeyCode::Char(c)` for printable chars → append to `input_buffer`
- [ ] 6.4 Match on `KeyCode::Backspace` → pop last char from `input_buffer`
- [ ] 6.5 Keep `KeyCode::Char('q')` → exit dashboard
- [ ] 6.6 All other keys → no-op

## 7. Implement reply sending

- [ ] 7.1 On Enter with non-empty buffer and focused question: get `pane_index` from `QuestionEntry`
- [ ] 7.2 Run `tmux send-keys -t <session_name>:<pane_index> "<escaped_input>" Enter` via `std::process::Command`
- [ ] 7.3 Shell-escape the input buffer before passing to `tmux send-keys`
- [ ] 7.4 Remove the answered question from `questions` and clear `input_buffer`
- [ ] 7.5 Advance focus to next question or clear focus if no more questions

## 8. Unit tests

- [ ] 8.1 Test: `QuestionPayload { question: "" }` fails validation
- [ ] 8.2 Test: `BrokerMessage::Question` round-trips through serde with `"type": "agent.question"`
- [ ] 8.3 Test: `Display` for `Question` variant produces `[feat-config] question: ...`
- [ ] 8.4 Test: `status_label()` returns `"question"` for `Question` variant
- [ ] 8.5 Test: Publishing `agent.question` enqueues in `"supervisor"` inbox
- [ ] 8.6 Test: Publishing `agent.question` does NOT enqueue in sender's inbox
- [ ] 8.7 Test: Publishing `agent.question` does NOT enqueue in other agents' inboxes
- [ ] 8.8 Test: `format_agent_rows` still works after question message is in broker log
- [ ] 8.9 Test: Tab advances focus from 0 to 1 with two questions
- [ ] 8.10 Test: Tab wraps from last to first
- [ ] 8.11 Test: Enter with empty input is a no-op (no `tmux send-keys` call)
- [ ] 8.12 Test: Enter with input removes question from list and clears buffer

## 9. Quality gates

- [ ] 9.1 `cargo fmt` clean
- [ ] 9.2 `cargo clippy --all-targets -- -D warnings` clean
- [ ] 9.3 `cargo test` — all tests pass (new + existing)
- [ ] 9.4 `cargo doc --no-deps` — no warnings
- [ ] 9.5 `just check` — full pipeline green
- [ ] 9.6 Verify all existing broker-messages tests still pass
- [ ] 9.7 Verify all existing dashboard tests still pass (multi-section layout is backward-compatible for existing scenarios)

## 10. Handoff readiness

- [ ] 10.1 `agent.question` wire format is documented in the `QuestionPayload` doc comment
- [ ] 10.2 `QuestionEntry` is a public type
- [ ] 10.3 `run_dashboard` signature change is backward-compatible (pane_map defaults to empty)
- [ ] 10.4 Reply sending via `tmux send-keys` is isolated in a testable helper function
- [ ] 10.5 Modified files: `src/broker/messages.rs`, `src/broker/delivery.rs`, `src/dashboard.rs` only (plus test files)
- [ ] 10.6 Commit with message: `feat(dashboard): add prompt inbox with question routing and reply via tmux send-keys`
