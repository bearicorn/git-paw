## Context

The broker is git-paw's coordination bus.

- **Workers.** Each worker has one agent id: the slugified worktree branch. Workers publish
  `agent.status`, `agent.artifact`, `agent.blocked`, `agent.question` and `agent.intent` to
  `POST /publish`, through `broker.sh` or the MCP publish tools.
- **The root session.** Its agent id is `"orchestrator"`, the root's wire identity since v0.21.0
  and its only accepted spelling since v1.0.0. It publishes `agent.verified`, `agent.feedback`, `agent.answer` and
  `agent.advanced-main`.
- **The broker itself.** It emits `orchestrator.verify-now`, `agent.learning`, and the conflict
  detector's `agent.feedback` and `agent.question` in-process.
- **Routing.** `delivery::route_message` fills per-agent inboxes. Clients read them by cursor with
  `GET /messages/{id}?since=N`. `GET /log` and `GET /status` serve the whole log and the roster.

This document is the design anchor for the whole v1.2.0 A2A cycle, which has five other changes:
`a2a-task-lifecycle`, `a2a-agent-card`, `a2a-legacy-coexistence`, `a2a-task-streaming` and
`a2a-migration-guide`. The listener's `Host`/`Origin` guard (`broker-listener-origin-guard`) was
moved into v0.17.0, so this cycle builds on it. The Sources section and the mapping table below apply to all of them.

## Sources

Every A2A fact in this cycle comes from these primary sources, all read on 2026-10-18.

- **A2A Protocol Specification, latest released version `1.0.0`.**
  <https://a2a-protocol.org/latest/specification/>. The normative text and the normative proto are
  at tag `v1.0.1`:
  - <https://github.com/a2aproject/A2A/blob/v1.0.1/docs/specification.md>
  - <https://github.com/a2aproject/A2A/blob/v1.0.1/specification/a2a.proto>, package `lf.a2a.v1`.
    Per §1.4, the proto is the single authoritative definition of the data objects.
- **A2A release notes.** <https://github.com/a2aproject/A2A/releases>:
  - v1.0.0, 2026-03-12: the first stable release.
  - v1.0.1, 2026-05-28: a patch with HTTP-binding media-type and transcoding-error fixes. Per §3.6,
    patch versions never affect protocol compatibility, so the protocol version is `1.0`.
  - v0.3.0, 2025-07-30: renamed the well-known card URI from `agent.json` to `agent-card.json`.
- **Release announcement.**
  <https://a2a-protocol.org/latest/blog/2026/03/12/a2a-protocol-ships-v10-production-ready-standard-for-agent-to-agent-communication/>
- **Official Rust SDK.** <https://github.com/a2aproject/a2a-rs>, licensed Apache-2.0. It is
  published as `a2a-lf` (types, 0.4.2, 2026-10-16), `a2a-server-lf` (0.5.3) and `a2a-client-lf`
  (0.2.9), all with MSRV 1.85, per the crates.io API. The SDK is still 0.x and broke its minor
  version as recently as 2026-09-30. `a2a-lf` depends on `base64`, `chrono`, `serde`, `serde_json`,
  `thiserror` and `uuid`. `a2a-server-lf` also pulls in `a2a-pb`, `reqwest`, `tower-http`,
  `tracing` and `tokio-stream`.
- **MCP transports**, for `a2a-migration-guide`'s decision to drop the MCP HTTP item:
  - <https://modelcontextprotocol.io/specification/2026-07-28/basic/transports>
  - <https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http>
    (revision 2026-07-28).
  - The `rmcp` crate metadata: <https://crates.io/crates/rmcp>.

**Facts from the spec that this cycle relies on:**

- **Version negotiation (§3.6).** The protocol version is `Major.Minor`. Clients send `A2A-Version`
  as a header or query parameter. An empty value means `0.3`. A server answers an unsupported
  version with `VersionNotSupportedError` (`-32009`).
- **Bindings (§5, §9–§11).** JSON-RPC 2.0, gRPC and HTTP+JSON are all standard bindings. None is
  mandatory. An agent declares the bindings it supports in `supportedInterfaces`, and all of them
  must be functionally equivalent (§5.1). The JSON-RPC binding uses `application/json`,
  PascalCase method names, and SSE for streaming.
- **Discovery (§8.2, §14.3).** The card is at `https://{server_domain}/.well-known/agent-card.json`,
  so there is one card per origin.
- **Task states (proto `TaskState`).**
  - In progress: `SUBMITTED`, `WORKING`.
  - Interrupted: `INPUT_REQUIRED`, `AUTH_REQUIRED`.
  - Terminal: `COMPLETED`, `FAILED`, `CANCELED`, `REJECTED`.
  - A message sent to a terminal task is refused with `UnsupportedOperationError` (§3.1.1).
  - Task ids are server-generated (§3.4.2).
- **Messages (§3.7).** Messages are communication and Artifacts carry results. Messages "MUST NOT
  be considered a reliable delivery mechanism", and a client that reconnects to a stream may miss
  status messages.
- **Extensions (§4.6).** An extension is identified by a URI. A breaking change needs a new URI.
  An unknown extension is ignored unless the card marks it `required`.
- **Security (§7, §13).** Schemes are declared in the card and credentials are obtained out of
  band. The server authenticates every request. §7.1 requires HTTPS only for production deployments
  over a network.

**Inconsistencies inside the spec, resolved by following the proto per §1.4:**

- §5.3 and §11.3 give `POST /tasks/{id}:subscribe`, but the proto's HTTP annotation is `GET`.
- The §8.5 sample card uses `security`, but the proto field is `security_requirements`, which is
  `securityRequirements` in JSON.

Neither affects this cycle, because git-paw serves only the JSON-RPC binding (see
`a2a-task-lifecycle`).

## Goals / Non-Goals

**Goals:**

- A2A 1.0 wire types that serialize exactly as ProtoJSON specifies, kept separate from the
  internal model.
- A pure, table-tested projection from coordination events to A2A task state, with the
  completion-authority invariant built in.
- One coordination extension URI and a stable metadata schema.

**Non-Goals:**

- Serving anything. The endpoint, card and streaming arrive in later changes.
- Changing `BrokerMessage`, delivery routing, the legacy endpoints, `broker.sh`, `sweep.sh` or the
  MCP tools.
- gRPC or HTTP+JSON bindings, Agent Card signing (JWS), and push notifications.

## Decisions

### D1: A2A is an external face over the bus, not a replacement for it

The MILESTONE item "swap custom JSON for the standardised A2A protocol" does not survive contact
with the stable spec, for three reasons:

1. A2A has no agent-originated publish operation. An A2A server reports progress through task
   updates that it emits itself, and the only client→server write is `SendMessage`.
2. A2A has no broadcast or pub/sub. Streams are per task.
3. Messages are explicitly unreliable (§3.7). git-paw's verdicts and feedback rely on durable,
   cursor-read inboxes.

Encoding every bus event as `SendMessage` plus extension metadata would be custom JSON inside an A2A
envelope. No third-party client could interpret it, and it would break the frozen v1.0 wire along
with `sweep.sh`, the drive loop, the dashboard and MCP.

The workers' way of reporting to the broker is what A2A SDKs call the *agent executor* side. The
specification leaves it to the implementation. git-paw's existing bus plays that role. A2A is the
standard face that the broker host presents to outside clients.

*Alternative rejected:* migrate the bus wholesale. That costs a breaking change on every bundled
helper, and the protocol gains nothing in return.

### D2: Concept mapping

Columns: the git-paw concept, its A2A construct, and the disposition (native, extension, or
internal).

| git-paw concept | A2A construct | Disposition |
|---|---|---|
| Session broker host | The A2A server, with one Agent Card per broker origin | Native |
| Root session / external orchestrator | An A2A client holding the session credential | Native (security scheme) |
| Worker agent (`agent_id` = branch slug) | A **Task**, one per worker assignment. The agent id goes in task metadata | Native task plus extension. A worker is not an A2A server, so it gets no card of its own |
| Wave / broker host lifetime | `contextId`, shared by every task | Native |
| `agent.status` heartbeat (label, message, activity, phase, modified files) | The task's current status message (`ROLE_AGENT`), the state per D4, and the label, phase and files in extension metadata | Native plus extension |
| `agent.artifact` `committed` / `done` | A task `Artifact` with a JSON data part. State is `WORKING` for `committed`, `INPUT_REQUIRED` (awaiting verdict) for `done` | Native |
| `agent.question` (from a worker) | `INPUT_REQUIRED`, with the question as the status message and in history | Native |
| `agent.blocked` (`needs`, `from`) | `INPUT_REQUIRED`, with `needs` and `from` in extension metadata | Native plus extension |
| `agent.answer` (root to worker) | A `ROLE_USER` history message. The task goes to `WORKING` | Native. A2A `SendMessage` produces it |
| `agent.feedback` (root or conflict detector to worker) | A `ROLE_USER` history message, with verdict `feedback` in extension metadata. The task goes to `WORKING` | Native plus extension |
| `agent.verified` (root, authority-gated) | `COMPLETED`, which is terminal | Native plus extension. This is the only path to completion |
| `agent.intent` | No state change. Unexpired intents go in task extension metadata | Extension |
| Verdict authority (v0.18.0) | The security scheme (§7.4) plus server authorization (§7.5) | Native |
| `agent.advanced-main` | A session-level broadcast with no task to attach to | Internal (legacy `/messages`, `/log`, MCP) |
| `agent.learning` | Output of the session-level aggregator | Internal |
| `orchestrator.verify-now` | A nudge to the root. On the A2A face, the `done`/`committed` artifact update is that signal | Internal |
| Conflict-detector question (`agent_id = "orchestrator"`) | A session-level escalation with no worker task | Internal (open question) |
| Inbox cursors (`/messages?since`), `/log?since` | `GetTask` and `ListTasks` polling, and later `SubscribeToTask`. These are lossy across reconnects, per §3.7 | Native, but weaker. The legacy cursor stays the reliable channel |
| `/status` roster | `ListTasks` | Native |
| `POST /watch` (filesystem watch-target registration) | None. The launcher does this internally | Internal. Registration still opens a task (D3) |
| Escalated permission prompt (drive loop) | `AUTH_REQUIRED` would fit exactly (§7.6 names "human approval before a destructive action"), but no per-worker broker event exists | Deferred (open question) |
| `git paw remove` | `CANCELED` would fit, but `remove` does not tell the broker | Deferred (open question) |
| Push notifications | — | Declined. §13.2 SSRF guidance rejects loopback webhooks, and the broker has no outbound HTTP client |

**Honest fit assessment.** A2A's task model fits the delegation edge well: the orchestrator and a
worker's assignment, its progress, questions, results and verdict. It fits the peer coordination
bus poorly. Intents, main-advanced broadcasts, learnings and detector escalations have no
task-shaped home. They stay internal in v1.2.0, and the coordination extension leaves room for them
later.

### D3: One task per worker assignment, with server-generated identifiers

- **When a task opens.** The first time the broker learns of a worker agent id: a watch-target
  registration (at start, or via `POST /watch`), or the first message that names the id as sender
  or target. The root identity `"orchestrator"` never gets a task.
- **Task id.** Built from the context nonce, the agent id and a generation counter. It is opaque to
  clients, and unique within the context.
- **`contextId`.** Derived from the session name and the broker host's start instant. One context
  covers one broker host lifetime. Broker state is not reloaded across a host restart, so a restart
  starts a new context. That is documented, not hidden.
- **`messageId` on broker-derived messages.** Derived from the broker sequence number. A client's
  own `messageId` is preserved.

*Alternatives rejected:*

- A per-agent card that addresses the worker as a server. Workers do not execute A2A requests; see
  `a2a-agent-card`.
- Client-created tasks. A2A forbids client-provided task ids for new tasks, and v1.2.0 does not let
  an A2A client spawn workers (`a2a-task-lifecycle` D4).

### D4: State rules, as a pure reducer

The reducer is `apply(record, event, at) -> record`. It is fed in broker sequence order.

| Event (on the worker's current task) | From a non-terminal state | Notes |
|---|---|---|
| Registration | Opens `SUBMITTED` | — |
| `agent.status`, any label | `SUBMITTED` → `WORKING`. `WORKING` stays `WORKING`. `INPUT_REQUIRED` is **unchanged** | A heartbeat cannot clear a pending question or an awaiting-verdict state. This mirrors the roster's terminal-status stickiness |
| `agent.artifact` `done` | → `INPUT_REQUIRED`, plus an artifact | "Awaiting verdict" |
| `agent.artifact`, any other label | → `WORKING`, plus an artifact | — |
| `agent.blocked`, `agent.question` (worker is the sender) | → `INPUT_REQUIRED`, plus a history message | — |
| `agent.intent` | `SUBMITTED` → `WORKING`, otherwise unchanged. Intents are updated | — |
| `agent.feedback`, `agent.answer` (worker is the target) | → `WORKING`, plus a `ROLE_USER` history message | — |
| `agent.verified` (worker is the target) | → `COMPLETED` | The **only** completion path |
| A2A cancel (registry event, not a `BrokerMessage`) | → `CANCELED` | From `a2a-task-lifecycle` |

On a terminal task:

- Worker-originated events are ignored by the projection. They stay in the broker log.
- A later `agent.feedback` or `agent.answer` opens generation n+1 in `WORKING`. Its first history
  message references the previous task id.
- A repeated `agent.verified` is a no-op.

`FAILED`, `REJECTED` and `AUTH_REQUIRED` are never produced in v1.2.0, because git-paw has no
explicit signal for them. See Open Questions.

### D5: The completion-authority invariant

The status *label* is untrusted, worker-supplied data. A worker can post
`agent.status {status: "verified"}` or `agent.artifact {status: "verified"}` on the legacy wire,
which v0.18.0 does not gate. The projection therefore keys completion on the `agent.verified`
**variant**, which v0.18.0 `broker-verdict-authority` restricts to the root credential, and never on
a label.

### D6: The coordination extension, v1

- **URI:** `https://bearicorn.github.io/git-paw/a2a/coordination/v1`. It is versioned in the path,
  as §4.6.3 requires. The page is published by `a2a-migration-guide`. The URI is git-paw's own
  identifier, not a consumer convention, so it is export-agnostic.
- **Metadata schema.** All metadata goes under the URI key, with camelCase field names:
  - `Task.metadata`: `agentId`, `generation`, `statusLabel?`, `phase?`, `modifiedFiles`,
    `intents[]` (`files`, `summary`, `validForSeconds`, `declaredAt`), and `previousTaskId?`.
  - Status and history message metadata: `event`, which is one of `status`, `artifact`, `blocked`,
    `question`, `intent`, `feedback`, `guidance`, `verdict` or `cancel`, plus event fields
    (`needs`, `from`).
  - Client message metadata: `verdict`, which is `verified` or `feedback`.
- **Not required.** The card marks the extension `required: false`, so a generic A2A client can
  observe tasks and send guidance. Only verdicts need it.

### D7: Hand-written DTOs on the existing serde stack, not the SDK

The A2A objects git-paw uses are small: about fifteen types, of which only Task, Message, Part,
Artifact and Card are non-trivial. The SDK's types crate (`a2a-lf`) is pre-1.0 and churns. It would
add `base64` and `uuid`, neither of which is approved, and the server crate pulls in a much larger
tree. Hand-written DTOs, checked against golden JSON fixtures derived from the v1.0.1 proto, cost
nothing in dependencies.

*Revisit when:* `a2a-lf` reaches 1.0. Adopting the types crate is then a mapper-only change.
**User decision:** approve `a2a-lf` or stay hand-written. The recommendation is to stay
hand-written for v1.2.0.

### D8: Where the code lives

| Layer | Contents |
|---|---|
| Wire | `broker::wire::a2a`: DTOs, ProtoJSON serde attributes, and `Task::from(&TaskRecord)`. No state |
| Core | `broker::tasks`: `TaskState`, `TaskEvent`, `TaskRecord`, `apply`, and `task_events_from(&BrokerMessage) -> Vec<TaskEvent>`. No serde-wire types, no axum, no clock. Time is passed in |
| Entry | Later changes: `broker::a2a`, which holds the JSON-RPC handlers and the card |

The reducer is folded incrementally on the publish path in `a2a-task-lifecycle`, so reads never
rescan the log.

## Risks / Trade-offs

- **Hand-written DTOs drift from the proto.**
  → Golden fixtures for every emitted object, validated field by field against the v1.0.1 proto
  (required fields, camelCase names, enum strings). Unknown input fields are ignored, as §5.7
  recommends.
- **Projection semantics are an interpretation.** Examples: `done` mapping to `INPUT_REQUIRED`, and
  heartbeats not clearing `INPUT_REQUIRED`.
  → Both are specified with scenarios and documented in the migration guide. Each is a single table
  row to change.
- **The DefaultHasher-style derivation used for ids is stable only per toolchain.**
  → Ids are opaque and scoped to one context, so cross-build stability is not needed.
- **A heartbeat cannot un-block a task.** A worker unblocked by a peer stays `INPUT_REQUIRED` until
  its next artifact or the root's guidance. This matches today's roster stickiness, and the
  migration guide documents it.

## Migration Plan

This change is additive and not served. To roll back, revert it; nothing persisted changes.

## Open Questions

- **`AUTH_REQUIRED` for escalated permission prompts.** It needs a per-worker broker event from the
  v0.18.0 drive loop's escalation hand-off. Today escalation outcomes are published as
  `agent.status` from `orchestrator` with no target worker.
  *Recommendation:* add the event in a later 1.x minor and map it then.
- **`CANCELED` on `git paw remove`.** It needs `remove` to notify the broker.
  *Recommendation:* a small 1.x follow-up.
- **`FAILED` when a worker CLI dies.** There is no explicit signal today. v0.17.0
  `cli-unavailable-detection-gaps` may provide one.
- **Session-level events on the A2A face** (`advanced-main`, detector escalations, learnings). The
  candidate is an extension-defined session task or stream. v1.4.0's orchestrator-to-orchestrator work
  is the natural point to decide this.
