## MODIFIED Requirements

### Requirement: Cursor-based message polling

`poll_messages(state, agent_id, since)` SHALL return a tuple `(Vec<BrokerMessage>, u64)` containing:

- All messages in the agent's inbox with sequence numbers strictly greater than `since`
- A cursor equal to the **greater of `since` and the highest sequence number among the returned messages**. The cursor SHALL be monotonic: it SHALL NOT regress below `since`, and in particular an empty result (no messages newer than `since`) SHALL return `since` itself, not `0`. This lets a client advance with `cursor = last_seq` on every poll — including empty polls — without ever re-reading already-seen messages.

Polling SHALL be non-destructive — messages are retained in the inbox and can be re-read with a smaller `since` value. Each message SHALL have a globally unique, auto-incrementing `u64` sequence number assigned at publish time. Cursor advancement SHALL be independent of message type: no message variant (including `Question`) may wedge the cursor or prevent later messages from being delivered on subsequent polls.

#### Scenario: Poll returns all messages when since is 0

- **GIVEN** agent `"feat-x"` has 3 messages in its inbox with sequences 1, 2, 3
- **WHEN** `poll_messages(&state, "feat-x", 0)` is called
- **THEN** the result contains 3 messages
- **AND** `last_seq` is `3`

#### Scenario: Poll returns only newer messages

- **GIVEN** agent `"feat-x"` has messages with sequences 1, 2, 3, 4, 5
- **WHEN** `poll_messages(&state, "feat-x", 3)` is called
- **THEN** the result contains 2 messages (sequences 4 and 5)
- **AND** `last_seq` is `5`

#### Scenario: Poll with since equal to latest returns empty but holds the cursor

- **GIVEN** agent `"feat-x"` has messages up to sequence 5
- **WHEN** `poll_messages(&state, "feat-x", 5)` is called
- **THEN** the result contains 0 messages
- **AND** `last_seq` is `5` (the cursor holds at `since`; it does NOT regress to `0`)

#### Scenario: Repeated polls return the same messages

- **GIVEN** agent `"feat-x"` has messages with sequences 1, 2, 3
- **WHEN** `poll_messages(&state, "feat-x", 0)` is called twice
- **THEN** both calls return the same 3 messages with the same `last_seq`

#### Scenario: Poll for unknown agent returns empty

- **GIVEN** no agent `"feat-unknown"` has ever published
- **WHEN** `poll_messages(&state, "feat-unknown", 0)` is called
- **THEN** the result contains 0 messages
- **AND** `last_seq` is `0`
- **AND** no error occurs

#### Scenario: Poll uses a read lock only

- **WHEN** `poll_messages` is called
- **THEN** it acquires a read lock on `BrokerState` (not a write lock)

#### Scenario: A Question does not wedge later messages in a mixed inbox

- **GIVEN** the `"supervisor"` inbox receives, in order, an `agent.question` (sequence `q`) then an `agent.artifact` (sequence `a`, with `a > q`)
- **WHEN** a client polls with `since = 0`, advances to the returned `last_seq`, and polls again with that cursor
- **THEN** the first poll returns the question (and any messages up to its cursor) and reports `last_seq >= q`
- **AND** the second poll returns the artifact and reports `last_seq >= a`
- **AND** at no point does a poll re-return the question after the cursor has advanced past `q`

## ADDED Requirements

### Requirement: Duplicate question suppression

When routing a `BrokerMessage::Question` to the `"supervisor"` inbox, the broker SHALL suppress the enqueue if an identical question — same `agent_id` and same `payload.question` text — is already resident in the supervisor inbox. This prevents a blocked agent that re-publishes the same question every poll cycle from flooding the supervisor inbox with duplicates.

Suppression SHALL be scoped to identical `(agent_id, question)` pairs; a question with different text, or the same text from a different agent, SHALL still be enqueued. Suppression SHALL NOT drop the message silently in a way that loses the first copy — the first occurrence is always enqueued; only exact re-publishes of a still-resident question are dropped.

#### Scenario: Identical re-published question is enqueued only once

- **GIVEN** the `"supervisor"` inbox is empty
- **WHEN** agent `"feat-x"` publishes an `agent.question` with `question = "Which error type?"` and then publishes the identical `agent.question` again before the first is drained
- **THEN** the supervisor inbox contains exactly one copy of that question

#### Scenario: Distinct questions from the same agent both enqueue

- **GIVEN** the `"supervisor"` inbox is empty
- **WHEN** agent `"feat-x"` publishes an `agent.question` with `question = "Which error type?"` and then an `agent.question` with `question = "Which module?"`
- **THEN** the supervisor inbox contains both questions

#### Scenario: Same question text from a different agent still enqueues

- **GIVEN** the supervisor inbox already holds a question `"Which error type?"` from `"feat-x"`
- **WHEN** agent `"feat-y"` publishes an `agent.question` with the identical text `"Which error type?"`
- **THEN** the supervisor inbox holds both copies (one per agent)
