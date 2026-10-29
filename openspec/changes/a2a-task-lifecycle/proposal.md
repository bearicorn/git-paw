## Why

`a2a-wire-model` gives each worker an A2A task. Nothing serves it yet. The orchestrator that
v0.18.0 moved outside the session currently steers workers through `sweep.sh` and MCP, and observes
them by polling `/status` and `/log`. A2A 1.0 is the standard interface for this kind of
delegation: list the tasks, read one, send it input, record the outcome, cancel it. Serving A2A
lets any A2A-capable orchestrator drive a git-paw session without git-paw-specific tooling.

The verdict authority from v0.18.0 must still hold, and the face must be least-privilege by
construction. That rules out unauthenticated reads, network exposure, and a remote client
spawning workers.

## What Changes

- **Opt-in `[broker.a2a] enabled`**, which defaults to `false`. A2A is layer 7 of the v1.0.0 layer
  model, so it is additive and isolated. When it is disabled, the broker's HTTP surface is unchanged.
- **Loopback only.** The A2A face is served only when the broker is bound to a loopback address.
  On any other bind it is not served, and the host warns at start. A2A §7.1 requires HTTPS for
  networked production deployments, and git-paw does not terminate TLS.
- **JSON-RPC 2.0 binding** at `POST <broker_url>/a2a`, declaring protocol version `1.0`. This is
  the only binding git-paw serves. gRPC and HTTP+JSON are optional in A2A and are not served.
- **Every A2A operation requires the session credential.** This is the root-authority credential
  from v0.18.0, sent as `Authorization: Bearer <credential>` and compared in constant time.
  Failures get `401` before dispatch and change nothing.
- **Version negotiation.** `A2A-Version: 1.0` is required. A missing value means `0.3` per the
  spec, which is refused with `VersionNotSupportedError` (`-32009`).
- **Operations:**
  - `GetTask` and `ListTasks`, with cursor pagination, filters, `historyLength` and
    `includeArtifacts`.
  - `SendMessage` to an **existing** worker task. Plain text becomes `agent.answer` guidance. A
    message that opts into the coordination extension carries a verdict: `feedback` becomes
    `agent.feedback`, and `verified` becomes `agent.verified`. Delivery uses the same in-process
    publish path as the legacy face, so workers see no difference. The call blocks by default until
    the task is interrupted or terminal; `returnImmediately` skips the wait. A repeated `messageId`
    is not delivered twice.
  - `CancelTask` marks the task `CANCELED` and tells the worker to stop and stand by. It never
    kills a process or removes a worktree; that stays with `git paw remove`.
  - Push-notification config operations return `PushNotificationNotSupportedError`.
    `GetExtendedAgentCard` returns `UnsupportedOperationError`. The streaming methods return
    `UnsupportedOperationError` until `a2a-task-streaming` is approved.
  - A `SendMessage` without a `taskId` returns `UnsupportedOperationError`. In v1.2.0 an A2A client
    cannot create work, so it cannot spawn agents.
- **One state, two faces.** The legacy endpoints are unchanged and fully supported. Anything
  published on either face is visible on both.
- `broker-runtime`'s route-table requirement, which already names the five legacy routes
  (`/watch` included), is extended so that it allows the A2A routes when the face is served.

## Capabilities

### New Capabilities

<!-- None. `broker-a2a` is introduced by `a2a-wire-model`. -->

### Modified Capabilities

- `broker-a2a`: adds the configuration section, the serving conditions, the JSON-RPC binding,
  authentication, version negotiation, the task operations and their errors, verdict authority on
  the A2A face, and cross-face consistency.
- `broker-runtime`: *Unknown routes return 404*, which keeps the five legacy routes and allows the
  A2A routes when they are served.
- `core-security-posture`: the posture chapter documents the A2A face's exposure and privileges.

## Impact

- **Code:**
  - New `src/broker/a2a/` holding the entry layer: `rpc.rs` (JSON-RPC parse and dispatch, error
    mapping), `auth.rs` (the pure bearer decision with a constant-time compare), `version.rs` (the
    pure negotiation) and `ops.rs` (the handlers).
  - `src/broker/mod.rs`: `BrokerState` gains a task registry that is folded on the publish path,
    and a change notifier (`tokio::sync::watch`).
  - `src/broker/delivery.rs`: `publish_message` folds `TaskEvent`s when the A2A face is served.
  - `src/broker/server.rs`: the router mounts `POST /a2a` only when served.
  - `src/config/broker.rs`: `A2aConfig { enabled }` with a serde default.
  - The init template gets a commented `[broker.a2a]` stanza.
- **Wire:** new routes and JSON-RPC shapes. The legacy `BrokerMessage` wire is byte-identical.
- **Enum-variant ripple:** none. No variant is added. A2A cancels live in the task registry, not
  in `BrokerMessage`.
- **Dependencies:** none. JSON-RPC is implemented with `serde_json`, waiting uses `tokio::sync`, and
  the constant-time compare is in-tree.
- **Cross-version dependencies:**
  - Requires `a2a-wire-model` and v0.17.0's `broker-listener-origin-guard`.
  - Requires v0.18.0 `broker-verdict-authority` for the credential (`Session.broker_authority`,
    `git paw __authority`) and the verdict rule.
  - Requires v0.18.0 `broker-host-separation`, because the A2A face is served by the `__broker`
    host process.
  - Uses v0.21.0's config-schema versioning (`config-schema-versioning`) for the additive
    `[broker.a2a]` table.
  - Respects the v1.0.0 layer-isolation rule.
  - `broker-runtime` *Unknown routes return 404* is not modified by any other pending change.
    v0.17.0 `broker-repo-identity` and `stop-without-confirmation`, and v0.18.0
    `broker-host-separation`, touch other `broker-runtime` requirements.
- **Security / safety:**
  - The face adds no privilege. Every operation needs the root credential that already authorizes
    verdicts.
  - It refuses network exposure.
  - It cannot spawn or kill processes.
  - Guidance text it delivers reaches agents with execution power. That is the same trust as the
    root's `sweep.sh` and MCP paths, and the posture chapter states it.
