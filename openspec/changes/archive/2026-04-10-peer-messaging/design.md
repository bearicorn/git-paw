## Context

This change is pure logic — no new I/O, no new dependencies, no new modules. It replaces three stub function bodies in `src/broker/delivery.rs` with real implementations that manipulate `BrokerStateInner` fields. The HTTP layer (`server.rs`) is frozen; the message types (`messages.rs`) are frozen; the `BrokerState` wrapper API is frozen. This change only operates through existing access patterns — calling public methods on `BrokerState` and manipulating `BrokerStateInner` fields via the write lock.

The delivery logic is the core of v0.3.0's coordination feature. It determines how agents discover each other's work, how blocked requests find their target, and how the dashboard gets its data. Despite being ~100-150 lines of code, the routing rules must be precisely correct because agents rely on them for coordination.

## Goals / Non-Goals

**Goals:**

- Implement the three delivery functions with correct routing per message type
- Make all delivery logic unit-testable without an HTTP server or tokio runtime
- Ensure the dashboard sees agent records immediately after a publish
- Keep lock hold times minimal (microseconds, not milliseconds)

**Non-Goals:**

- Message persistence (messages are in-memory only; lost on broker restart)
- Message TTL or expiration (messages stay in the queue until polled)
- Message ordering guarantees beyond insertion order (FIFO within a single agent's inbox)
- Rate limiting or backpressure (agents can publish as fast as they want)
- Deduplication (same message published twice results in two deliveries)
- Acknowledgment (poll drains the queue; no ack/nack)
- v0.4 message types (`agent.verified`, `agent.feedback`) — those are added when the supervisor lands

## Decisions

### Decision 1: Routing rules are per-variant, not configurable

The routing is hardcoded in a match expression:

```rust
pub(crate) fn publish_message(state: &BrokerState, msg: BrokerMessage) {
    let mut inner = state.write();

    // Always update the sender's agent record
    update_agent_record(&mut inner, &msg);

    match &msg {
        BrokerMessage::Status { .. } => {
            // Status updates are informational — no routing needed.
            // Dashboard reads them via agent_status_snapshot.
        }
        BrokerMessage::Artifact { agent_id, .. } => {
            // Broadcast to every other agent's inbox
            for (id, queue) in inner.queues.iter_mut() {
                if id != agent_id {
                    queue.push_back(msg.clone());
                }
            }
        }
        BrokerMessage::Blocked { payload, .. } => {
            // Targeted delivery to the agent named in payload.from
            if let Some(queue) = inner.queues.get_mut(&payload.from) {
                queue.push_back(msg.clone());
            }
        }
    }
}
```

**Why:**
- Three message types, three routing rules. A lookup table or configuration layer would be overengineering.
- The routing rules map directly to the v0.3.0 coordination model in MILESTONE.md: status is observed (dashboard), artifacts are broadcast (all peers need to know), blocked is targeted (only the agent that can help).
- Adding v0.4's `agent.verified` and `agent.feedback` will be two more match arms — trivial.

**Alternatives considered:**
- *Router trait with pluggable strategies.* Premature abstraction for 3 variants. Rejected.
- *All messages broadcast to all agents.* Would flood agents with irrelevant status updates. Rejected.

### Decision 2: Cursor-based polling (non-destructive reads)

```rust
pub(crate) fn poll_messages(state: &BrokerState, agent_id: &str, since: u64) -> (Vec<BrokerMessage>, u64) {
    let inner = state.read();
    let Some(queue) = inner.queues.get(agent_id) else {
        return (Vec::new(), 0);
    };
    let messages: Vec<_> = queue.iter()
        .filter(|(seq, _)| *seq > since)
        .map(|(_, msg)| msg.clone())
        .collect();
    let last_seq = queue.iter().last().map(|(seq, _)| *seq).unwrap_or(0);
    (messages, last_seq)
}
```

Each message in the queue is stored with an auto-incrementing sequence number (`u64`). The `since` parameter filters to messages strictly newer than that sequence. The response includes `last_seq` — the highest sequence in the result — which the agent passes as `since` on its next poll.

**Why:**
- **Lossless.** No messages are ever deleted. If an agent crashes between polling and processing, it re-polls with the same `since` and gets the same messages.
- **Idempotent.** Repeated polls with the same `since` return the same results, making the system robust against retries.
- **No ack complexity.** Agents track their own cursor (the `last_seq` from the previous response). No server-side read tracking per agent.
- **Memory bounded in practice.** Sessions run for minutes to hours with 3-10 agents publishing ~1 message/minute. Worst case: ~500 messages × ~200 bytes ≈ 100KB. Trivially small for in-memory storage that resets every session.
- **Read lock only.** Polling takes a read lock (not write), so multiple agents can poll concurrently without contention.

**Alternatives considered:**
- *Drain on read (consume-on-read).* Simpler but lossy — crashed agents lose messages forever. Rejected.
- *Ack per message (POST /ack/:id).* Adds a new endpoint, message IDs, ack tracking, and re-delivery logic. Rejected — cursor-based achieves the same lossless guarantee with no new endpoints.
- *Keep messages, mark as read.* Server-side read tracking per agent. More state, more complexity. Rejected — cursor-based offloads tracking to the agent.

### Decision 3: Agent records are created lazily on first publish

When `publish_message` is called with an `agent_id` not yet in `inner.agents`, a new `AgentRecord` is created automatically. There is no "register agent" step.

```rust
fn update_agent_record(inner: &mut BrokerStateInner, msg: &BrokerMessage) {
    let agent_id = msg.agent_id();
    let record = inner.agents
        .entry(agent_id.to_string())
        .or_insert_with(|| AgentRecord::new(agent_id));
    record.last_seen = Instant::now();
    record.status = msg.status_label().to_string();
    record.last_message = Some(msg.clone());

    // Also ensure the agent has an inbox
    inner.queues
        .entry(agent_id.to_string())
        .or_insert_with(VecDeque::new);
}
```

**Why:**
- Agents are launched by tmux; git-paw doesn't know exactly when each one starts. Lazy creation means the agent appears in the dashboard the moment it publishes its first status.
- No registration API to call, no timing dependencies between tmux pane launch and broker readiness.
- Polling by an unknown agent_id returns empty (no error) — the agent may simply not have any messages yet.

**Alternatives considered:**
- *Pre-register agents at session launch.* Requires `broker-integration` to call a registration API before launching panes, creating a startup ordering dependency. Rejected.

### Decision 4: Artifact broadcast creates inboxes lazily too

When broadcasting an `agent.artifact`, the sender loops over `inner.queues`. But what if other agents haven't published yet and don't have inboxes? The broadcast skips them — they'll get future artifacts once they're known, but they'll miss this one.

**Is this a problem?** Not really. The coordination model is:
1. Agent A starts, publishes `agent.status` (creates its record + inbox)
2. Agent B starts, publishes `agent.status` (creates its record + inbox)
3. Agent A finishes, publishes `agent.artifact` (broadcasts to B's inbox)

If agent B is slow to start and A finishes before B's first status, B misses A's artifact. But B will eventually check `/status` and see A is "done" — the dashboard shows all agent states regardless of whether B received the broadcast.

**Mitigation:** Document this as a known limitation. If it matters in practice, v0.4's supervisor can handle it (the supervisor polls `/status` and can resend artifacts to late-joining agents).

### Decision 5: `agent_status_snapshot` clones records to minimize lock hold time

```rust
pub(crate) fn agent_status_snapshot(state: &BrokerState) -> Vec<AgentStatusEntry> {
    let inner = state.read();
    inner.agents.values()
        .map(|r| AgentStatusEntry::from(r))
        .collect()
}
```

Takes a read lock, iterates agents, clones into `AgentStatusEntry` values, releases the lock. The dashboard (and the `/status` HTTP handler) receive an owned snapshot that can be rendered/serialized without holding any lock.

**Why:**
- The dashboard holds this data for the entire frame render (~1ms). Holding a read lock for 1ms would block any publish write. Cloning 10-20 agent records takes <1μs and eliminates the contention.
- `AgentStatusEntry` is a small struct (~5 fields, all `String` or `Instant`). Cloning is cheap.

### Decision 6: `BrokerMessage` needs a helper method for extracting `agent_id`

The three delivery functions all need to extract `agent_id` from the enum. Rather than matching in each function, add a helper on `BrokerMessage`:

```rust
impl BrokerMessage {
    pub fn agent_id(&self) -> &str { ... }
    pub fn status_label(&self) -> &str { ... }
}
```

**Where does this live?** In `src/broker/messages.rs`. This is technically a modification to a Wave 1 file. Two options:
- **(a)** Add the methods in `messages.rs` as part of this change. Small, additive, no signature changes to existing code.
- **(b)** Add them in `delivery.rs` via a private helper function that takes `&BrokerMessage` and matches.

**Choose (a)** — it's the Rust-idiomatic location (methods on the type), and it's a pure addition that doesn't touch any existing code in `messages.rs`. The Wave 1 implementing agent for `message-types` could even anticipate this and add the methods themselves.

### Decision 7: Broker message log (periodic background flush, plain text)

Since messages are cursor-based and never deleted, the in-memory message list is already a complete, ordered record of everything that happened in the session. A background flush thread periodically writes new entries to disk for traceability.

**Architecture:**

1. `publish_message` stores `(seq, timestamp, msg)` in a `Vec` inside `BrokerStateInner`. Zero disk I/O in the publish path.
2. `start_broker` spawns a `std::thread` (not tokio — avoids async I/O concerns) that runs a flush loop:

```rust
fn flush_loop(state: BrokerState, log_path: PathBuf, stop: Arc<AtomicBool>) {
    let mut last_flushed: u64 = 0;
    while !stop.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_secs(5));

        // Read new entries under a short-lived read lock
        let new_entries: Vec<_> = {
            let inner = state.read();
            inner.message_log.iter()
                .filter(|(seq, _, _)| *seq > last_flushed)
                .cloned()
                .collect()
        };

        if new_entries.is_empty() { continue; }

        // Append to file outside the lock
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log_path) {
            for (seq, ts, msg) in &new_entries {
                let _ = writeln!(f, "[{}] {} {}", seq, format_timestamp(*ts), msg);
            }
        }

        last_flushed = new_entries.last().map(|(seq, _, _)| *seq).unwrap_or(last_flushed);
    }

    // Final flush on shutdown (same logic, one last pass)
}
```

3. `BrokerHandle::drop` sets the `stop` flag and joins the flush thread, ensuring one final flush before exit.

**Log file location:** `<session_state_dir>/broker.log` — the session state directory is managed by `broker-integration` (Wave 2), which passes the path into `BrokerState` at construction. If no log path is configured (e.g. during tests), the flush thread is not spawned.

**Format:** plain text, one line per message, using the existing `Display` impl on `BrokerMessage`:

```
[1] 2026-04-10T14:30:00 [feat-errors] status: working (2 files modified)
[2] 2026-04-10T14:30:30 [feat-errors] artifact: done — exports: PawError, NotAGitRepo
[3] 2026-04-10T14:30:45 [feat-config] blocked: needs PawError from feat-errors
```

**Why plain text with `Display`:**
- Human-readable, `grep`-friendly, zero new serialization code
- `Display` impl already exists from `message-types`
- Matches the aesthetic of existing `git paw replay` (ANSI-stripped terminal output)
- If v0.4 supervisor learnings need structured data, the in-memory log is serde-serializable — a JSON export can be added later without changing the plain text log

**Why periodic flush (not per-publish):**
- Zero disk I/O in `publish_message` — the hot path stays fast
- The flush thread is `std::thread`, not tokio — no async worker thread blocking
- The read lock is held only to clone new entries (microseconds), same pattern as the dashboard
- File writes happen entirely outside any lock — no contention with publish or poll
- Multi-repo safe: each broker's flush thread writes to its own `broker.log`

**Worst case data loss:** ~5 seconds of messages on a hard crash (kill -9). Acceptable for an audit trail — the tmux session logs (via `pipe-pane`) still capture terminal output.

**Future integration:** `git paw replay` currently shows tmux session logs. `broker.log` is a second log stream that could be shown alongside or separately (`git paw replay --broker`). That integration is out of scope for this change.

## Risks / Trade-offs

- **Late-joining agents miss earlier broadcasts** → An agent that starts late won't see artifacts published before its inbox existed. **Mitigation:** the dashboard shows all agent states; the agent can check `/status` to discover completed peers. Document as a known limitation. The cursor model doesn't help here because the inbox itself doesn't exist yet when the broadcast happens.

- **Unbounded message retention** → Messages are never deleted within a session. With typical sessions (3-10 agents, minutes of runtime, messages every ~30s), total storage is ~100-500 messages × ~200 bytes ≈ 100KB. **Mitigation:** acceptable for v0.3.0. If v0.4+ sessions run for hours with many agents, add a max-retention or TTL then. Session restart clears all state.

- **Write lock contention under high publish rate** → If all 10 agents publish simultaneously every second, the write lock is contested 10 times/second. Each write holds the lock for <1μs (HashMap insert + VecDeque push). At 10/second, total lock-held time is ~10μs/second. Negligible. **Mitigation:** not a real risk at v0.3.0 scale.

- **Modifying `messages.rs`** → Adding helper methods to a Wave 1 file. Minimal risk since it's purely additive (no existing code changes). **Mitigation:** if the supervisor flags a concern during merge review, the methods can live as private helpers in `delivery.rs` instead.

- **Flush thread lifecycle** → The flush thread must be joined on shutdown to ensure the final flush completes. If `BrokerHandle::drop` panics or is skipped (e.g. `std::process::exit`), the last ~5 seconds of messages are lost. **Mitigation:** acceptable for an audit trail. The flush thread checks a `stop` flag every 5 seconds, so shutdown latency is at most 5 seconds. If faster shutdown is needed, use a `Condvar` to wake the thread immediately.

- **~5 seconds of data loss on crash** → Hard crash (kill -9, OOM) loses messages accumulated since the last flush. **Mitigation:** tmux session logs still capture terminal output. The broker log is supplementary, not the only record. Reducing the flush interval (e.g. to 2 seconds) trades CPU/disk for durability.

## Migration Plan

No migration. This change replaces stub bodies with real logic. The HTTP behavior changes from "publish returns 202 but nothing happens" to "publish returns 202 and messages are routed." No config changes, no new endpoints, no breaking changes.
