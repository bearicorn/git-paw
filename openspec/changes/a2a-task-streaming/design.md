## Context

The A2A sources are listed in `a2a-wire-model/design.md`, section "Sources". The relevant parts are:

- §3.1.2 and §3.1.6: streams begin with the `Task` and close at a terminal state.
- §3.5.2: events are delivered in order, are broadcast to every active stream, and one stream
  closing does not affect the others.
- §9.4.2: in the JSON-RPC binding, each SSE `data:` line is a full JSON-RPC response carrying a
  `StreamResponse`.
- §3.3.4: when `capabilities.streaming` is false, these methods return `-32004`.

`a2a-task-lifecycle` folds events into the task registry and bumps a `watch` counter.

## Goals / Non-Goals

**Goals:**

- A spec-conformant SSE stream per task.
- Ordered, gap-free delivery, or an explicit close.
- Bounded memory.

**Non-Goals:**

- Resumable streams (`Last-Event-ID`). A2A 1.0 does not define them; a client re-subscribes
  instead.
- Push notifications.
- A session-wide stream, which is open in `a2a-wire-model`.

## Decisions

### D1: One bounded broadcast channel; each subscriber filters by task

Each registry fold emits `(task_id, StreamEvent)` on a `tokio::sync::broadcast` channel with a
fixed capacity, for example 1024 events. A subscriber filters for its own task id. On a `Lagged`
result, the subscriber's stream is closed, which follows §3.7. Closing is honest; skipping events
would hide a gap.

*Alternative rejected:* one channel per task, which costs more bookkeeping for no gain at session
scale.

### D2: The snapshot is taken under the same read as the subscription

The handler subscribes to the channel and then reads the task snapshot. Any event that is newer
than the snapshot's last applied sequence is delivered, and events already reflected in the
snapshot are dropped. The first event is therefore the current `Task`, with no lost and no
duplicated update (§3.1.6 rationale).

### D3: Close at terminal states only, in both methods

§3.1.2 requires the stream to close when the task becomes terminal. The REST binding's §11.7 also
mentions interrupted states, but git-paw serves only JSON-RPC, so it follows §3.1.2. An
orchestrator watching a worker therefore sees `INPUT_REQUIRED` arrive on the same stream and
answers through `SendMessage`.

### D4: Keep-alive comments

On an idle stream, the broker sends an SSE comment line (`:`) every 15 seconds so that proxies
and clients do not time out. Clients ignore comment lines. The interval is an internal constant,
and tests can override it.

### D5: The dependency

The only new direct dependency is `futures-util`, which provides `Stream` and `StreamExt` for
axum's `Sse`. It is already resolved in `Cargo.lock`.

Alternatives:

- **`tokio-stream`'s `BroadcastStream`.** Also not approved, and a new crate in the lock.
- **A hand-rolled `http_body::Body`.** Needs `http-body`, another unapproved direct dependency,
  for more code.

If the user declines `futures-util`, this change is dropped.

## Risks / Trade-offs

- **The connection count grows with subscribers.**
  → Credential-gated and loopback-only. The expected count is one orchestrator times a handful of
  workers.
- **A broker host restart drops streams and starts a new context.**
  → Clients re-list the tasks, as documented in `a2a-wire-model`.

## Migration Plan

This change is additive. With it reverted, the card returns to `streaming: false` and the two
methods return `-32004`.

## Open Questions

- Whether to approve `futures-util` as a direct dependency. **User decision.** The recommendation
  is to approve it.
