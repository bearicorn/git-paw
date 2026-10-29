## Context

The A2A facts below are cited from `a2a-wire-model/design.md`, section "Sources" (A2A 1.0, proto
`lf.a2a.v1` at tag v1.0.1). The relevant ones are:

- the JSON-RPC binding (§9);
- the method set and error codes (§5.3, §5.4);
- capability validation (§3.3.4);
- blocking `SendMessage` semantics (§3.2.2);
- terminal-state refusal (§3.1.1);
- authentication (§7.3–§7.5);
- data scoping (§13.1).

On the git-paw side, by the time this lands:

- the broker runs in its own `__broker` host process (v0.18.0 `broker-host-separation`);
- `/publish` refuses `agent.verified` and `agent.feedback` without the root credential
  `X-Paw-Authority` (v0.18.0 `broker-verdict-authority`);
- the credential lives only in the `0600` user-data `Session` file and the hidden
  `git paw __authority`;
- the listener refuses browser origins (v0.17.0's `broker-listener-origin-guard`).

## Goals / Non-Goals

**Goals:**

- A spec-conformant A2A 1.0 JSON-RPC server for the session's worker tasks.
- Least privilege: root credential only, loopback only, and no process control.
- One coordination state. The A2A face writes through the same publish path the legacy face uses.

**Non-Goals:**

- New-task creation, which would mean spawning workers.
- Push notifications.
- Extended cards.
- gRPC or HTTP+JSON bindings.
- TLS or remote access. These belong to v1.4.0.
- Streaming, which is in `a2a-task-streaming`.
- Changing workers' transport. Workers keep `broker.sh` and MCP publish.

## Decisions

### D1: Opt-in layer, served only on loopback

`[broker.a2a] enabled = false` is the default, per the v1.0.0 layer-isolation rule: A2A is layer 7
and installs only its own surface. The `__broker` host reads the setting at start.

On a non-loopback `bind`, the face is not served. The host warns and points at the
security-posture chapter. A2A §7.1 requires TLS for networked deployments. Serving plain HTTP to a
network would be non-conformant and unsafe.

*Alternative rejected:* on by default. A new authenticated surface on every v1.2.0 session would
contradict layer isolation, and nothing needs it on by default.

### D2: JSON-RPC only

JSON-RPC is a single route with PascalCase methods and the same SSE as MCP-style tooling. The
HTTP+JSON binding needs AIP-style paths with colons (`/tasks/{id}:cancel`). gRPC needs `tonic` and
`prost`, which are not approved. A2A requires no particular binding, only functional equivalence
across the bindings an agent declares (§5.1), and git-paw declares one.

JSON-RPC 2.0 batch arrays are not supported. They return `-32600`, because A2A defines no batch
semantics. A request without an `id` is invalid, because every A2A method returns a result (§9.3).

### D3: Authentication reuses the root credential as a bearer token

- **Scheme.** `HTTPAuthSecurityScheme { scheme: "Bearer" }`. It is the most widely supported
  scheme, and the card declares it.
- **Credential.** The same value v0.18.0 mints. The legacy face keeps its `X-Paw-Authority`
  header, and each face uses its own conventional header. A client obtains the credential out of
  band (A2A §7.3): the orchestrator runs `git paw __authority` at the repository root.
- **Comparison.** Constant time.
- **Failure.** HTTP `401` with `WWW-Authenticate: Bearer` (§7.4 SHOULD), and a JSON-RPC error with
  code `-32000`. That code is in JSON-RPC's implementation-defined server-error range and outside
  A2A's reserved `-32001`…`-32099` block. The error carries `google.rpc.ErrorInfo` with reason
  `UNAUTHENTICATED` and domain `git-paw`.
- **No credential.** A session that has no credential fails closed.

**Every operation requires the credential, reads included.** The legacy `/status` and `/log`
remain unauthenticated for local tools, so this costs no capability. Starting strict is the
semver-safe direction, because relaxing a rule later is additive. The authorization model
(§13.1 SHOULD document it) is that the credential holder sees and steers every task in the session.

### D4: No new tasks over A2A in v1.2.0

A `SendMessage` without a `taskId` asks the server to start new work. For git-paw that means
creating a worktree and launching an auto-approved coding agent with prompt text that came over
HTTP. The v0.18.0 MCP write tools already offer this to the root, behind the same credential.
Duplicating it on a network protocol before TLS and remote trust exist (v1.4.0) widens the attack
surface for no v1.2.0 consumer. The call returns `UnsupportedOperationError` with ErrorInfo reason
`UNSUPPORTED_OPERATION` and a detail naming `git paw add`.

v1.4.0 can add a delegation skill additively, without changing this change's wire.

### D5: SendMessage reuses the legacy publish path

The handler maps the incoming message to a `BrokerMessage` from the root identity `"orchestrator"`,
which v0.18.0 keeps as the wire identity:

| Extension `verdict` | Resulting message |
|---|---|
| None | `agent.answer`, with the text parts joined by newlines |
| `feedback` | `agent.feedback`, with one error per text part; at least one is required |
| `verified` | `agent.verified`, with the joined text as the optional message |

It then publishes through `delivery::publish_message` on the blocking pool, exactly like
`/publish`. Workers, `/messages`, `/log`, the dashboard, the conflict detector and learnings
therefore all see an ordinary message, and the projection folds it like any other.

**Validation:**

- A verdict needs the extension URI in `message.extensions`. Verdict metadata without the URI gets
  `-32602`, so that a client's mistake is never downgraded to plain guidance.
- Only `text` parts are accepted. `raw`, `url` and `data` parts get `-32005`, because workers read
  text.
- `role` must be `ROLE_USER`.
- A `contextId` that does not match the task gets `-32602` (§3.4.3).
- `messageId` is remembered per task, and a repeat is acknowledged without being delivered again
  (§3.3.1). This protects against client retries doubling feedback.

### D6: Blocking by default, as the spec requires

Without `returnImmediately`, the response is withheld until the task is interrupted
(`INPUT_REQUIRED`) or terminal (§3.2.2 MUST). For an orchestrator this means "tell the worker,
return when it asks or finishes". The wait uses a per-registry `tokio::sync::watch` that is bumped
on every fold. There is no server-side cap, because the spec says MUST wait. The docs recommend
`returnImmediately: true` plus polling for long work.

### D7: Cancel records state; it does not control processes

`CancelTask` on a non-terminal task sets `CANCELED` in the registry. It also delivers an
`agent.answer` from `"orchestrator"` that tells the worker the orchestrator cancelled the task and it
should stop, commit nothing further, and stand by. A repeat cancel returns the same task and sends
no second notice. Cancelling a `COMPLETED` task returns `-32002`.

Process and worktree teardown stays with `git paw remove`, which is the existing authority. This
keeps an A2A caller from deleting work.

### D8: Unsupported optional capabilities are refused exactly as §3.3.4 specifies

| Operation | Error |
|---|---|
| The four push-config operations | `-32003` |
| `GetExtendedAgentCard` | `-32004` |
| `SendStreamingMessage`, `SubscribeToTask` | `-32004` while streaming is not served |

Every A2A error carries ErrorInfo with domain `a2a-protocol.org` and the reason in A2A's naming,
for example `TASK_NOT_FOUND`. §9.5 makes this optional for JSON-RPC, and it costs nothing.

### D9: Pagination and listing

- Results are sorted by status timestamp, newest first (§3.1.4 MUST).
- `pageSize` defaults to 50 and is capped at 100. A value of zero or less gets `-32602`.
- `pageToken` is an opaque cursor that encodes the last returned task's timestamp and id.
- `nextPageToken` is always present, and is `""` on the last page.
- `includeArtifacts` defaults to false, in which case the `artifacts` field is omitted.
- Filters: `contextId`, `status` and `statusTimestampAfter`.

## Risks / Trade-offs

- **A blocking `SendMessage` holds an HTTP connection for as long as a worker runs.**
  → Spec-mandated. Documented, with `returnImmediately` and polling recommended. Tokio handles
  idle connections cheaply.
- **A2A clients need the hidden `__authority` command.**
  → Acceptable while the orchestrator, which runs at the root, is the client. A visible connection
  command is an open question for v1.4.0's remote clients.
- **The A2A face and the legacy face drift apart.**
  → Both go through one publish path. The cross-face end-to-end tests are part of this change.
- **Guidance text reaches agents with execution power.**
  → Same trust as the root's existing paths. It is credential-gated and loopback-only, and the
  posture chapter states it.

## Migration Plan

1. Config field, serving conditions, and the router mount (disabled means unchanged).
2. Registry fold on the publish path, and the notifier.
3. JSON-RPC parse and dispatch, auth, version.
4. Operations and the error map.
5. Cross-face end-to-end tests.
6. Docs.

To roll back, set `enabled = false`, or revert. Nothing is persisted.

## Open Questions

- **A visible credential hand-off for third-party A2A clients**, for example
  `git paw a2a connect`, which would print the URL and the credential to a TTY.
  *Recommendation:* defer to v1.4.0, together with remote access.
- **Should reads (`GetTask`, `ListTasks`) be open on loopback, like the legacy `/status`?**
  *Recommendation:* no, stay strict. Relaxing later is additive.
