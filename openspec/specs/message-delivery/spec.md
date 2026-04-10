# message-delivery Specification

## Purpose
TBD - created by archiving change peer-messaging. Update Purpose after archive.
## Requirements
### Requirement: Publish updates sender's agent record

When `publish_message` is called, the system SHALL update the sender's `AgentRecord` in `BrokerStateInner`:

- Set `last_seen` to the current instant
- Set `status` to the message's status label (e.g. `"working"`, `"done"`, `"blocked"`)
- Set `last_message` to a clone of the published message
- If no `AgentRecord` exists for the sender's `agent_id`, one SHALL be created automatically (lazy registration)
- If no inbox queue exists for the sender's `agent_id`, one SHALL be created automatically

#### Scenario: First publish from an agent creates its record

- **GIVEN** a `BrokerState` with no known agents
- **WHEN** `publish_message` is called with an `agent.status` message from `agent_id = "feat-errors"`
- **THEN** `BrokerStateInner.agents` contains a record for `"feat-errors"`
- **AND** the record's `status` is `"working"`
- **AND** the record's `last_seen` is approximately `Instant::now()`

#### Scenario: Subsequent publish updates an existing record

- **GIVEN** a `BrokerState` with an existing record for `"feat-errors"` with status `"working"`
- **WHEN** `publish_message` is called with an `agent.artifact` message from `"feat-errors"` with status `"done"`
- **THEN** the record's `status` is updated to `"done"`
- **AND** `last_seen` is updated

#### Scenario: Publish creates an inbox for the sender

- **GIVEN** a `BrokerState` with no known agents
- **WHEN** `publish_message` is called from `"feat-errors"`
- **THEN** `BrokerStateInner.queues` contains an inbox entry for `"feat-errors"`

### Requirement: Status messages are not routed

When a `BrokerMessage::Status` is published, the system SHALL update the sender's agent record but SHALL NOT enqueue the message in any agent's inbox. Status messages are informational — the dashboard reads them via `agent_status_snapshot`.

#### Scenario: Status message does not appear in any inbox

- **GIVEN** agents `"feat-errors"` and `"feat-detect"` both with existing inboxes
- **WHEN** `publish_message` is called with an `agent.status` message from `"feat-errors"`
- **THEN** `poll_messages` for `"feat-detect"` returns no new messages
- **AND** `poll_messages` for `"feat-errors"` returns no new messages

### Requirement: Artifact messages are broadcast to all other agents

When a `BrokerMessage::Artifact` is published, the system SHALL enqueue the message in every known agent's inbox EXCEPT the sender's own inbox. Agents whose inboxes do not yet exist (not yet registered via a publish) SHALL NOT receive the broadcast.

#### Scenario: Artifact broadcast reaches all peers

- **GIVEN** three agents `"feat-errors"`, `"feat-detect"`, and `"feat-config"` all with existing inboxes
- **WHEN** `publish_message` is called with an `agent.artifact` message from `"feat-errors"`
- **THEN** `poll_messages` for `"feat-detect"` returns the artifact message
- **AND** `poll_messages` for `"feat-config"` returns the artifact message

#### Scenario: Artifact broadcast skips the sender

- **GIVEN** agents `"feat-errors"` and `"feat-detect"` both with existing inboxes
- **WHEN** `publish_message` is called with an `agent.artifact` message from `"feat-errors"`
- **THEN** `poll_messages` for `"feat-errors"` returns no new messages

#### Scenario: Artifact broadcast skips agents not yet registered

- **GIVEN** agent `"feat-errors"` has an existing inbox but `"feat-detect"` has never published
- **WHEN** `publish_message` is called with an `agent.artifact` message from `"feat-errors"`
- **THEN** no inbox exists for `"feat-detect"`
- **AND** no error occurs

### Requirement: Blocked messages are delivered to the target agent

When a `BrokerMessage::Blocked` is published, the system SHALL enqueue the message in the inbox of the agent identified by `payload.from` (the agent that can unblock the sender). If the target agent's inbox does not exist, the message SHALL be silently dropped (the target has not yet registered).

#### Scenario: Blocked message reaches the target agent

- **GIVEN** agents `"feat-config"` and `"feat-errors"` both with existing inboxes
- **WHEN** `publish_message` is called with an `agent.blocked` message from `"feat-config"` with `payload.from = "feat-errors"`
- **THEN** `poll_messages` for `"feat-errors"` returns the blocked message

#### Scenario: Blocked message does not reach other agents

- **GIVEN** agents `"feat-config"`, `"feat-errors"`, and `"feat-detect"` all with existing inboxes
- **WHEN** `publish_message` is called with an `agent.blocked` message from `"feat-config"` with `payload.from = "feat-errors"`
- **THEN** `poll_messages` for `"feat-detect"` returns no new messages

#### Scenario: Blocked message to unregistered target is silently dropped

- **GIVEN** agent `"feat-config"` has an existing inbox but `"feat-errors"` has never published
- **WHEN** `publish_message` is called with an `agent.blocked` message from `"feat-config"` with `payload.from = "feat-errors"`
- **THEN** no error occurs
- **AND** no inbox is created for `"feat-errors"`

### Requirement: Cursor-based message polling

`poll_messages(state, agent_id, since)` SHALL return a tuple `(Vec<BrokerMessage>, u64)` containing:

- All messages in the agent's inbox with sequence numbers strictly greater than `since`
- The highest sequence number among the returned messages, or `0` if no messages match

Polling SHALL be non-destructive — messages are retained in the inbox and can be re-read with the same `since` value. Each message SHALL have a globally unique, auto-incrementing `u64` sequence number assigned at publish time.

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

#### Scenario: Poll with since equal to latest returns empty

- **GIVEN** agent `"feat-x"` has messages up to sequence 5
- **WHEN** `poll_messages(&state, "feat-x", 5)` is called
- **THEN** the result contains 0 messages
- **AND** `last_seq` is `0`

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

### Requirement: Agent status snapshot

`agent_status_snapshot(state)` SHALL return an `AgentStatusEntry` for every known agent. The function SHALL:

- Take a read lock on `BrokerState`
- Clone each agent's record into an `AgentStatusEntry`
- Release the lock before returning

The returned snapshot SHALL be an owned value that can be used for rendering or serialization without holding any lock.

#### Scenario: Snapshot contains all registered agents

- **GIVEN** three agents have published at least one message each
- **WHEN** `agent_status_snapshot(&state)` is called
- **THEN** the result contains exactly 3 `AgentStatusEntry` values

#### Scenario: Snapshot reflects latest status

- **GIVEN** agent `"feat-errors"` has published two messages: first `agent.status` with status `"working"`, then `agent.artifact` with status `"done"`
- **WHEN** `agent_status_snapshot(&state)` is called
- **THEN** the entry for `"feat-errors"` has `status = "done"`

#### Scenario: Snapshot is empty when no agents have published

- **GIVEN** a fresh `BrokerState` with no published messages
- **WHEN** `agent_status_snapshot(&state)` is called
- **THEN** the result is an empty `Vec`

#### Scenario: Snapshot uses a read lock

- **WHEN** `agent_status_snapshot` is called
- **THEN** it acquires a read lock on `BrokerState` (not a write lock)

### Requirement: Sequence number assignment

Each message stored in any agent's inbox SHALL be assigned a globally unique, auto-incrementing `u64` sequence number. The sequence SHALL start at `1` for the first message in a session and SHALL monotonically increase. The sequence counter SHALL be shared across all agents — sequence numbers are globally ordered, not per-agent.

#### Scenario: First message gets sequence 1

- **GIVEN** a fresh `BrokerState`
- **WHEN** an `agent.artifact` message is published and broadcast to one peer
- **THEN** the peer's inbox contains the message with sequence `1`

#### Scenario: Sequence numbers are globally monotonic

- **GIVEN** agents `"a"` and `"b"` both with existing inboxes
- **WHEN** agent `"a"` publishes an artifact (broadcast to `"b"`) and then agent `"b"` publishes an artifact (broadcast to `"a"`)
- **THEN** the message in `"b"`'s inbox has a lower sequence than the message in `"a"`'s inbox

### Requirement: Message log accumulation

Every message passed to `publish_message` SHALL be stored in an in-memory log within `BrokerStateInner` as a tuple of `(seq, timestamp, message)`. This log SHALL be append-only and SHALL never be truncated during a session. The log serves as the data source for the periodic background flush to disk.

#### Scenario: Published messages appear in the message log

- **GIVEN** a fresh `BrokerState`
- **WHEN** 3 messages are published
- **THEN** the in-memory message log contains exactly 3 entries
- **AND** each entry has a unique sequence number, a timestamp, and the original message

#### Scenario: Message log includes all message types

- **WHEN** one `agent.status`, one `agent.artifact`, and one `agent.blocked` message are published
- **THEN** the in-memory message log contains all three, regardless of routing (status messages are logged even though they are not routed to inboxes)

### Requirement: Periodic log flush to disk

The system SHALL spawn a `std::thread` (not a tokio task) that periodically flushes new message log entries to a plain text file. The flush thread SHALL:

- Run every ~5 seconds
- Take a read lock on `BrokerState`, read entries with `seq > last_flushed_seq`, release the lock
- Append formatted lines to the log file outside of any lock
- Use the `Display` impl of `BrokerMessage` for formatting each line as `[seq] timestamp [agent_id] message_display`
- Be best-effort — disk write failures SHALL NOT affect message delivery or crash the broker
- Perform one final flush when signaled to stop (on `BrokerHandle` drop)

If no log path is configured in `BrokerState` (e.g. during tests), the flush thread SHALL NOT be spawned.

#### Scenario: Flush thread writes new messages to disk

- **GIVEN** a `BrokerState` with a configured log path and 3 published messages
- **WHEN** the flush thread runs its periodic cycle
- **THEN** the log file contains 3 lines, one per message
- **AND** each line contains the sequence number and the `Display` output of the message

#### Scenario: Flush thread only writes new entries

- **GIVEN** a flush thread has already written messages 1-3 to the log file
- **WHEN** 2 more messages are published and the flush thread runs again
- **THEN** the log file now contains 5 lines total (original 3 + 2 new)

#### Scenario: Final flush on shutdown

- **GIVEN** messages have been published since the last periodic flush
- **WHEN** `BrokerHandle` is dropped
- **THEN** the flush thread performs one final flush before exiting
- **AND** all messages are present in the log file

#### Scenario: No flush thread without log path

- **GIVEN** a `BrokerState` with no configured log path
- **WHEN** `start_broker` is called
- **THEN** no flush thread is spawned
- **AND** message delivery works normally

#### Scenario: Disk write failure does not affect delivery

- **GIVEN** a `BrokerState` with a log path pointing to a read-only directory
- **WHEN** a message is published and the flush thread attempts to write
- **THEN** the write fails silently
- **AND** the message is still present in the in-memory log and routable via `poll_messages`

### Requirement: BrokerMessage helper methods

The system SHALL add two public methods to the `BrokerMessage` type in `src/broker/messages.rs`:

- `pub fn agent_id(&self) -> &str` — returns the `agent_id` field from whichever variant the message is
- `pub fn status_label(&self) -> &str` — returns a short label: `"working"` for `Status` (from `payload.status`), `"done"` for `Artifact` (from `payload.status`), `"blocked"` for `Blocked`

These methods SHALL be purely additive — no existing code in `messages.rs` is changed.

#### Scenario: agent_id returns the correct value for each variant

- **WHEN** `agent_id()` is called on a `Status` message with `agent_id = "feat-x"`
- **THEN** the result is `"feat-x"`

#### Scenario: status_label returns payload status for Status variant

- **WHEN** `status_label()` is called on a `Status` message with `payload.status = "working"`
- **THEN** the result is `"working"`

#### Scenario: status_label returns payload status for Artifact variant

- **WHEN** `status_label()` is called on an `Artifact` message with `payload.status = "done"`
- **THEN** the result is `"done"`

#### Scenario: status_label returns blocked for Blocked variant

- **WHEN** `status_label()` is called on a `Blocked` message
- **THEN** the result is `"blocked"`

