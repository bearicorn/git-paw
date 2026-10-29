## Why

A2A reached its first stable release, protocol version `1.0` (v1.0.0, 2026-03-12; patch v1.0.1,
2026-05-28). git-paw v1.2.0 adopts it as the standard way for an outside agent to observe and steer
a session. Before any endpoint exists, the session's coordination state needs an A2A representation:
wire types for the A2A 1.0 data model, and a deterministic projection of each worker's broker events
into an A2A **Task**.

This is a seam, and it deliberately does **not** replace the broker bus. A2A defines a
client→server request model. It has no operation for an agent to publish its own status and no
broadcast primitive, and it states that Messages are not a reliable delivery channel (A2A §3.7).
git-paw's bus depends on all three: agent-originated heartbeats, broadcast intents, and cursor-based
inboxes. So the internal `BrokerMessage` model and the frozen legacy wire stay as they are. A2A
becomes an additional external face, translated at the `broker::wire` edge.

## What Changes

- **A2A 1.0 wire types** in `broker::wire::a2a`. These are the A2A objects git-paw emits or accepts:
  Task, TaskStatus, TaskState, Message, Role, Part, Artifact, AgentCard and its parts, the
  security-scheme subset, and the JSON-RPC 2.0 envelope and error object. Serialization follows
  ProtoJSON as A2A §5.5–§5.7 requires.
- **Task projection.** This is a pure reducer in the functional core, `broker::tasks`. Each worker
  assignment has one task. Coordination events fold into its A2A state. Only the authority-gated
  `agent.verified` variant can complete a task, and terminal tasks are final.
- **The git-paw coordination extension, v1.** An A2A extension URI and its metadata schema. It
  carries git-paw facts that A2A has no field for: the agent id, the status label, modified files,
  intents, and the verdict kind.
- **A mapper** from the core task record to the wire `Task`.
- **No HTTP surface and no behavior change.** Nothing serves these types yet. That comes in
  `a2a-task-lifecycle`, `a2a-agent-card` and `a2a-task-streaming`.

## Capabilities

### New Capabilities

- `broker-a2a`: the broker's A2A 1.0 protocol face. It covers the wire data model, the task
  projection from coordination events, the coordination extension, and, through later changes in
  this cycle, the JSON-RPC binding, the Agent Card, streaming and documentation.

### Modified Capabilities

<!-- None. -->

## Impact

- **Code (new):**
  - `src/broker/wire/a2a.rs`, or a submodule if `broker::wire` lands as a directory: the DTOs and
    the mapper.
  - `src/broker/tasks.rs`: the core `TaskState`, `TaskEvent` and `TaskRecord` types, plus the pure
    `apply` reducer and `task_events_from(&BrokerMessage)`.
  - Unit tables live next to each.
- **Code (touched):** `src/broker/mod.rs`, for the module declarations only.
- **Wire:** the legacy `BrokerMessage` JSON is unchanged. The A2A types are new and are not yet
  served.
- **Enum-variant ripple:** none. No `BrokerMessage` or `SpecBackendKind` variant is added or
  removed. `task_events_from` adds one new exhaustive `match` over `BrokerMessage`, so add it to the
  AGENTS.md §7 compile-forcing list as `src/broker/tasks.rs`.
- **Dependencies:** none. Serialization is `serde` and `serde_json`, timestamps are `chrono`, and
  ids are derived from existing sequence numbers. The official Rust SDK (`a2a-lf`) is a decision
  for the user. See design D7.
- **Cross-version dependencies:**
  - Builds on the v0.21.0 wire-DTO split ("Split wire DTOs from internals": the shared
    `broker::wire` module and its version fields). If that change lands under another name, this
    one rebases onto it. If it has not landed, this change creates `src/broker/wire/` itself.
  - Relies on v0.18.0 `broker-verdict-authority` for the rule that only the root session publishes
    `agent.verified` and `agent.feedback`. The projection's completion invariant assumes it.
  - Relies on the root's wire identity being `agent_id = "orchestrator"` and the broker's nudge
    type being `orchestrator.verify-now`: introduced by v0.21.0 `supervisor-vocabulary-split`, the
    only spellings accepted since v1.0.0 `remove-supervisor-vocabulary`.
  - Respects the v1.0.0 frozen wire.
- **Security / safety:** no new surface. The projection's invariant (a worker-published status label
  such as `"verified"` never completes a task) is the safety property later changes rely on.
