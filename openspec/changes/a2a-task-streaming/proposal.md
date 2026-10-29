## Why

The v0.17.0 dogfood (GP-33) showed that a turn-based orchestrator polling every 20 seconds misses
transitions. v0.18.0 answered that on the legacy face with a cursor-based event read surface
(`orchestrator-read-surface`). `a2a-task-lifecycle` gives an A2A client only polling, through
`GetTask` and `ListTasks`.

A2A 1.0 defines the event-driven surface:

- `SubscribeToTask` and `SendStreamingMessage` stream a task's status and artifact updates as
  Server-Sent Events (§3.1.2, §3.1.6, §3.5.2, §9.4.2);
- the agent declares `capabilities.streaming: true`.

**Gated on a dependency decision.** axum's `Sse` response needs a `futures` `Stream`. The approved
dependency set has no crate that provides the trait or a tokio-to-`Stream` adapter.
`futures-util` (MIT or Apache-2.0) is already in `Cargo.lock` as a dependency of axum, so adding it
as a direct dependency adds no new crate to the build. It still needs explicit approval under
AGENTS.md. Do not implement this change until the user approves; if they decline, drop the change
and streaming stays `false`.

## What Changes

- **`SubscribeToTask`.**
  - It answers `200 text/event-stream`. Each SSE `data:` line is a JSON-RPC response whose `result`
    is an A2A `StreamResponse`.
  - The first event is the current `Task`. The events after it are `statusUpdate` and
    `artifactUpdate`, in broker order. The stream closes after the event that makes the task
    terminal.
  - Subscribing to a terminal task returns `-32004`, and an unknown task returns `-32001`. Both are
    plain JSON-RPC errors, not streams.
- **`SendStreamingMessage`.** It validates and delivers like `SendMessage`, then streams the task
  the same way.
- **Fan-out.** Several subscribers to one task receive identical sequences, and closing one stream
  does not affect the others (§3.5.2).
- **No silent gaps.** A subscriber that falls behind the fan-out buffer has its stream closed. It
  never receives a sequence with a gap. A2A §3.7 already warns clients that reconnects can miss
  messages. Re-subscribing returns a fresh `Task` snapshot first.
- **The Agent Card** declares `capabilities.streaming: true`. The `UnsupportedOperationError`
  answers from `a2a-task-lifecycle` for these two methods no longer apply.
- **Authentication, version negotiation and the listener guard** apply as they do for every other
  A2A method.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `broker-a2a`: adds requirements for streaming subscription, streaming send, fan-out and
  lagging-subscriber handling, and for the card advertising streaming.

## Impact

- **Code:**
  - `src/broker/a2a/stream.rs`: a `tokio::sync::broadcast` fan-out fed by the registry fold, a
    per-subscriber task filter, SSE framing with axum's `Sse`, and a keep-alive comment on idle
    streams.
  - `card.rs`: `streaming = true`.
  - `rpc.rs`: dispatch for the two methods.
- **Dependencies:** **`futures-util` as a direct dependency, pending user approval.** It is already
  in `Cargo.lock` through axum, adds no new crate, and must still pass `cargo deny`.
- **Enum-variant ripple:** none.
- **Cross-version dependencies:**
  - Requires `a2a-task-lifecycle` and `a2a-agent-card`.
  - Complements, and does not replace, the cursor-based event read surface v0.18.0 shipped for
    GP-33 in `orchestrator-read-surface`: the `GET /log?since=&wait=` long poll with its `epoch`,
    the MCP `wait_for_events` tool, and `sweep.sh events`. This change gives the A2A face the same
    event-driven reads.
- **Security / safety:**
  - The streams are read-only and credential-gated.
  - The fan-out buffer is bounded, so a stalled client cannot grow broker memory.
  - Each stream ends at task termination or when its client disconnects.
