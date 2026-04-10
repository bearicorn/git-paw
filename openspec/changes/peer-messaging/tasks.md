## 1. BrokerMessage helper methods

- [ ] 1.1 Add `pub fn agent_id(&self) -> &str` to `BrokerMessage` in `src/broker/messages.rs` — match on all three variants, return the `agent_id` field
- [ ] 1.2 Add `pub fn status_label(&self) -> &str` to `BrokerMessage` — return `payload.status` for `Status` and `Artifact`, return `"blocked"` for `Blocked`
- [ ] 1.3 Add doc comments on both methods
- [ ] 1.4 Add unit tests in `src/broker/messages.rs` tests block: `agent_id()` returns correct value for each variant, `status_label()` returns correct value for each variant

## 2. Sequence number counter

- [ ] 2.1 Add a `next_seq: AtomicU64` field to `BrokerStateInner` (or `BrokerState` directly if using a separate atomic outside the `RwLock`)
- [ ] 2.2 Implement a `fn next_seq(state: &BrokerState) -> u64` helper that atomically increments and returns the next sequence number, starting at 1
- [ ] 2.3 Decide whether `next_seq` lives inside the `RwLock` (incremented under the write lock) or outside as a standalone `AtomicU64` (lock-free). If lock-free, document why in a code comment.

## 3. Message log accumulation

- [ ] 3.1 Add a `message_log: Vec<(u64, SystemTime, BrokerMessage)>` field to `BrokerStateInner`
- [ ] 3.2 In `publish_message`, after computing `seq`, push `(seq, SystemTime::now(), msg.clone())` to `message_log` before routing
- [ ] 3.3 Add a unit test: publish 3 messages, assert `message_log` has 3 entries with correct sequence numbers

## 4. Inbox storage with sequence numbers

- [ ] 4.1 Change the inbox type from `VecDeque<BrokerMessage>` to `Vec<(u64, BrokerMessage)>` (or similar — sequence number paired with message)
- [ ] 4.2 When enqueuing a message to an inbox (artifact broadcast or blocked delivery), store it as `(seq, msg.clone())`
- [ ] 4.3 Add a unit test: publish an artifact, verify the recipient's inbox contains the message paired with the correct sequence number

## 5. publish_message — routing logic

- [ ] 5.1 Implement `update_agent_record` helper: insert-or-update `AgentRecord` in `inner.agents`, create inbox in `inner.queues` if absent
- [ ] 5.2 Implement the routing match in `publish_message`:
  - `Status` → update record only, no inbox routing
  - `Artifact` → broadcast to every other agent's inbox (skip sender)
  - `Blocked` → enqueue in `payload.from`'s inbox (if exists, else silently drop)
- [ ] 5.3 Assign sequence number via `next_seq` before routing, use same `seq` for all copies of the message (broadcast)
- [ ] 5.4 Remove the `todo!()` stub — `publish_message` is now fully implemented
- [ ] 5.5 Unit test: status message updates record but does not appear in any inbox
- [ ] 5.6 Unit test: artifact message appears in all peers' inboxes, not in sender's
- [ ] 5.7 Unit test: artifact broadcast skips agents that haven't registered yet
- [ ] 5.8 Unit test: blocked message appears only in target agent's inbox
- [ ] 5.9 Unit test: blocked message to unregistered target is silently dropped (no error, no inbox created)
- [ ] 5.10 Unit test: first publish from an agent creates both record and inbox

## 6. poll_messages — cursor-based

- [ ] 6.1 Update `poll_messages` signature to `(state: &BrokerState, agent_id: &str, since: u64) -> (Vec<BrokerMessage>, u64)`
- [ ] 6.2 Implement: take read lock, filter inbox to entries with `seq > since`, clone messages, compute `last_seq` as max seq in result (or 0 if empty), release lock, return
- [ ] 6.3 Unit test: poll with `since = 0` returns all messages, correct `last_seq`
- [ ] 6.4 Unit test: poll with `since = 3` on inbox with sequences 1-5 returns only sequences 4, 5
- [ ] 6.5 Unit test: poll with `since = last_seq` returns empty with `last_seq = 0`
- [ ] 6.6 Unit test: repeated polls with same `since` return same results (non-destructive)
- [ ] 6.7 Unit test: poll for unknown agent returns `(Vec::new(), 0)`
- [ ] 6.8 Verify `poll_messages` uses `state.read()` not `state.write()`

## 7. agent_status_snapshot

- [ ] 7.1 Implement: take read lock, iterate `inner.agents`, map each `AgentRecord` to `AgentStatusEntry`, collect, release lock, return
- [ ] 7.2 Unit test: snapshot contains all registered agents with correct fields
- [ ] 7.3 Unit test: snapshot reflects latest status after multiple publishes
- [ ] 7.4 Unit test: snapshot on fresh state returns empty
- [ ] 7.5 Verify `agent_status_snapshot` uses `state.read()` not `state.write()`

## 8. Log flush thread

- [ ] 8.1 Add an optional `log_path: Option<PathBuf>` field to `BrokerState` (or pass to `start_broker`; coordinate with `broker-integration` on how the path is set)
- [ ] 8.2 Add a `stop_flag: Arc<AtomicBool>` for signaling the flush thread to exit
- [ ] 8.3 Implement `fn flush_loop(state: BrokerState, log_path: PathBuf, stop: Arc<AtomicBool>)`:
  - Track `last_flushed_seq: u64 = 0`
  - Loop: `sleep(5s)`, read lock, collect entries with `seq > last_flushed_seq`, release lock, append formatted lines to file, update `last_flushed_seq`
  - Exit loop when `stop` flag is set, perform one final flush before returning
- [ ] 8.4 Format each log line as `[{seq}] {timestamp} {message_display}` using `Display` impl on `BrokerMessage` and a human-readable timestamp (ISO 8601 or similar)
- [ ] 8.5 In `start_broker` (or wherever the broker is initialized), spawn the flush thread if `log_path` is `Some`. Store the `JoinHandle` in `BrokerHandle`.
- [ ] 8.6 In `BrokerHandle::drop`, set the `stop` flag and join the flush thread
- [ ] 8.7 Unit test: publish 3 messages, trigger a flush cycle, verify log file contains 3 lines
- [ ] 8.8 Unit test: publish 3 messages, flush, publish 2 more, flush again — file has 5 lines total
- [ ] 8.9 Unit test: verify final flush on handle drop captures unflushed messages
- [ ] 8.10 Unit test: no flush thread spawned when `log_path` is `None`
- [ ] 8.11 Unit test: disk write failure (read-only path) does not panic or affect in-memory state

## 9. Update http-broker handler for new poll_messages signature

- [ ] 9.1 Update the `messages` handler in `src/broker/server.rs` to parse the `since` query parameter (default to `0` if absent, return 400 if non-numeric)
- [ ] 9.2 Pass `since` to `poll_messages(&state, &agent_id, since)`
- [ ] 9.3 Serialize response as `{"messages": [...], "last_seq": N}`
- [ ] 9.4 Update the integration test in `tests/broker.rs` for the new response shape
- [ ] 9.5 Add integration test: `GET /messages/feat-x?since=3` returns only newer messages
- [ ] 9.6 Add integration test: `GET /messages/feat-x?since=abc` returns 400

## 10. Quality gates

- [ ] 10.1 `cargo fmt` clean
- [ ] 10.2 `cargo clippy --all-targets -- -D warnings` clean
- [ ] 10.3 `cargo test` — all unit and integration tests pass
- [ ] 10.4 `cargo doc --no-deps` builds without warnings
- [ ] 10.5 `just check` — full pipeline green

## 11. Handoff readiness

- [ ] 11.1 Confirm `delivery.rs` function signatures match the spec exactly (no signature changes from http-broker stubs except the new `since` parameter on `poll_messages`)
- [ ] 11.2 Confirm `src/broker/server.rs` has minimal changes — only the `since` query param parsing and new response shape
- [ ] 11.3 Confirm `src/broker/messages.rs` changes are purely additive (two new methods, no existing code touched)
- [ ] 11.4 Confirm `src/broker/mod.rs` changes are additive (new fields in `BrokerStateInner`, no public API signature changes)
- [ ] 11.5 Confirm no changes outside `src/broker/` and `tests/broker.rs`
- [ ] 11.6 Commit with message: `feat(broker): implement message delivery, cursor polling, and log flush`
