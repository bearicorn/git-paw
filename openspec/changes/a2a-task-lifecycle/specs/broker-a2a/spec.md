## ADDED Requirements

### Requirement: A2A configuration section

The system SHALL parse an optional `[broker.a2a]` table in `.git-paw/config.toml` into the broker configuration. The table SHALL hold one boolean field, `enabled`, which SHALL default to `false` when the field or the table is absent. A configuration without the table SHALL load with the A2A face disabled and every other setting unchanged. The table SHALL round-trip through save and load.

#### Scenario: Absent table means disabled

- **GIVEN** a config with `[broker] enabled = true` and no `[broker.a2a]` table
- **WHEN** the config is loaded
- **THEN** the A2A `enabled` setting SHALL be `false`

#### Scenario: Enabled table parses and round-trips

- **GIVEN** a config with `[broker.a2a]` containing `enabled = true`
- **WHEN** the config is loaded, saved and loaded again
- **THEN** the A2A `enabled` setting SHALL be `true` both times

### Requirement: The A2A face is served only when enabled on a loopback bind

The broker host SHALL serve the A2A face only when both of these hold: the session's broker is running with `[broker.a2a] enabled = true`, and `[broker] bind` is a loopback address. The A2A face means the A2A routes defined by this capability.

When `enabled = true` but the bind is not a loopback address, the host SHALL NOT serve the A2A face. It SHALL print a warning on stderr at start that names the bind address and states that A2A is served only on loopback. The legacy endpoints SHALL still be served.

When the A2A face is not served, every A2A route SHALL answer `404`. Every legacy endpoint SHALL respond exactly as it does when this capability is absent.

#### Scenario: Disabled face answers 404

- **GIVEN** a running broker with `[broker.a2a]` absent
- **WHEN** `POST /a2a` is sent
- **THEN** the response status SHALL be `404`

#### Scenario: Non-loopback bind refuses to serve A2A and warns

- **GIVEN** `[broker] bind = "0.0.0.0"` and `[broker.a2a] enabled = true`
- **WHEN** the broker host starts
- **THEN** stderr SHALL contain a warning naming `0.0.0.0` and stating that A2A is served only on loopback
- **AND** `POST /a2a` SHALL answer `404`
- **AND** `GET /status` SHALL answer `200`

#### Scenario: Enabled loopback broker serves the face

- **GIVEN** `[broker.a2a] enabled = true` on the default bind
- **WHEN** an authenticated `GetTask` for an existing task is posted to `/a2a`
- **THEN** the response SHALL be a JSON-RPC result carrying the task

### Requirement: JSON-RPC 2.0 binding at /a2a

The A2A face SHALL implement the A2A 1.0 JSON-RPC binding at `POST /a2a`. A request whose `Content-Type` is not `application/json` SHALL receive HTTP `415`. Every request SHALL be a single JSON-RPC 2.0 request object with `jsonrpc` equal to `"2.0"`, an `id`, and a PascalCase A2A method name.

Errors that are not authentication failures SHALL be returned as JSON-RPC error objects with HTTP status `200`. They SHALL use these codes:

- `-32700` for unparseable JSON;
- `-32600` for an invalid request object, which includes a missing `id` and a JSON array batch;
- `-32601` for an unknown method;
- `-32602` for invalid parameters;
- the A2A codes `-32001` through `-32009` for A2A errors.

Every A2A error SHALL carry, in `error.data`, a `google.rpc.ErrorInfo` with domain `a2a-protocol.org` and the A2A error name as its reason, for example `TASK_NOT_FOUND`.

#### Scenario: Wrong content type is refused

- **WHEN** an authenticated request is posted to `/a2a` with `Content-Type: text/plain`
- **THEN** the response status SHALL be `415`

#### Scenario: Batch request is invalid

- **WHEN** an authenticated JSON array of two requests is posted to `/a2a`
- **THEN** the response SHALL be a single JSON-RPC error with code `-32600`

#### Scenario: Unknown method

- **WHEN** an authenticated request with method `FrobnicateTask` is posted
- **THEN** the response SHALL be a JSON-RPC error with code `-32601`

#### Scenario: Unknown task carries ErrorInfo

- **WHEN** an authenticated `GetTask` names a task id that does not exist
- **THEN** the response SHALL be a JSON-RPC error with code `-32001`
- **AND** `error.data` SHALL contain an object with `@type` `type.googleapis.com/google.rpc.ErrorInfo`, reason `TASK_NOT_FOUND` and domain `a2a-protocol.org`

### Requirement: Every A2A request is authenticated with the session credential

Every request to `/a2a` SHALL carry `Authorization: Bearer <credential>`, where `<credential>` is the session's root-authority credential. The broker SHALL compare it in constant time before parsing the method or the parameters.

A request with a missing or non-matching credential SHALL receive HTTP `401` with a `WWW-Authenticate: Bearer` header and a JSON-RPC error with code `-32000`. The error SHALL carry a `google.rpc.ErrorInfo` with reason `UNAUTHENTICATED` and domain `git-paw`. The request SHALL change no broker state.

When the session has no credential, every A2A request SHALL be refused in the same way. The credential SHALL NOT appear in any A2A response, log line or diagnostic.

#### Scenario: Missing credential is refused before dispatch

- **WHEN** a `SendMessage` with valid parameters targeting worker `feat-x`'s task is posted without an `Authorization` header
- **THEN** the response status SHALL be `401` with a `WWW-Authenticate: Bearer` header
- **AND** the JSON-RPC error code SHALL be `-32000` with reason `UNAUTHENTICATED`
- **AND** `GET /messages/feat-x` SHALL contain no new message

#### Scenario: Wrong credential is refused

- **WHEN** a `GetTask` is posted with `Authorization: Bearer wrong`
- **THEN** the response status SHALL be `401`

#### Scenario: Session without a credential refuses everything

- **GIVEN** a broker state with no authority credential
- **WHEN** any A2A request is posted, with or without an `Authorization` header
- **THEN** the response status SHALL be `401`

#### Scenario: Credential never echoed

- **GIVEN** an accepted, authenticated `SendMessage`
- **WHEN** the response body, `/log` and the broker host's stderr are inspected
- **THEN** none SHALL contain the credential value

### Requirement: A2A protocol version negotiation

The A2A face SHALL read the requested protocol version from the `A2A-Version` header or, when the header is absent, from the `A2A-Version` query parameter. It SHALL compare only the `Major.Minor` part of the value.

A value of `1.0` SHALL be accepted. A missing or empty value SHALL be interpreted as `0.3`, as A2A requires. Any value other than `1.0` SHALL be refused with JSON-RPC error `-32009` (`VersionNotSupportedError`), and the request SHALL change no state.

#### Scenario: Version 1.0 is accepted

- **WHEN** an authenticated `ListTasks` is posted with `A2A-Version: 1.0`
- **THEN** the response SHALL be a JSON-RPC result

#### Scenario: Missing version means 0.3 and is refused

- **WHEN** an authenticated `ListTasks` is posted with no `A2A-Version` header or parameter
- **THEN** the response SHALL be a JSON-RPC error with code `-32009`

#### Scenario: Query parameter is honoured

- **WHEN** an authenticated `ListTasks` is posted to `/a2a?A2A-Version=1.0` without the header
- **THEN** the response SHALL be a JSON-RPC result

### Requirement: GetTask returns a worker's task

`GetTask` SHALL return the task with the requested `id`, mapped as the `broker-a2a` task model defines. It SHALL return error `-32001` when no such task exists.

`historyLength` SHALL be applied as follows:

- when it is absent, the full history SHALL be returned;
- when it is `0`, the `history` field SHALL be omitted;
- when it is a positive number, at most that many most recent history messages SHALL be returned.

#### Scenario: History length limits the returned messages

- **GIVEN** a task whose history holds four messages
- **WHEN** `GetTask` is called with `historyLength = 2`
- **THEN** the returned task SHALL carry exactly the two most recent history messages

#### Scenario: History length zero omits history

- **WHEN** `GetTask` is called with `historyLength = 0`
- **THEN** the returned task SHALL have no `history` key

### Requirement: ListTasks lists the session's tasks with cursor pagination

`ListTasks` SHALL return the session's tasks sorted by status timestamp, newest first. It SHALL filter by `contextId`, `status` and `statusTimestampAfter` when they are given.

The page size SHALL be `pageSize`, which defaults to 50 and is capped at 100. A `pageSize` of zero or less SHALL be refused with `-32602`. The response SHALL always carry `tasks`, `pageSize` and `nextPageToken`. `nextPageToken` SHALL be an opaque cursor, and SHALL be the empty string on the last page.

When `includeArtifacts` is absent or `false`, each listed task SHALL omit the `artifacts` field. `historyLength` SHALL apply to each task as it does for `GetTask`.

#### Scenario: Newest first, paginated

- **GIVEN** three tasks whose status timestamps are 10:00, 10:05 and 10:10
- **WHEN** `ListTasks` is called with `pageSize = 2` and then with the returned `nextPageToken`
- **THEN** the first page SHALL hold the 10:10 and 10:05 tasks with a non-empty `nextPageToken`
- **AND** the second page SHALL hold the 10:00 task with `nextPageToken` equal to `""`

#### Scenario: Status filter

- **GIVEN** one `WORKING` task and one `INPUT_REQUIRED` task
- **WHEN** `ListTasks` is called with `status = "TASK_STATE_INPUT_REQUIRED"`
- **THEN** only the `INPUT_REQUIRED` task SHALL be listed

#### Scenario: Artifacts omitted by default

- **GIVEN** a task with one artifact
- **WHEN** `ListTasks` is called without `includeArtifacts`
- **THEN** the listed task SHALL have no `artifacts` key

### Requirement: SendMessage delivers guidance and verdicts to an existing worker task

`SendMessage` SHALL require `message.taskId` to name an existing task. The broker SHALL deliver the message to that task's worker by publishing an ordinary coordination message from the root identity `"orchestrator"`, through the same in-process publish path that `POST /publish` uses.

The message delivered SHALL depend on the coordination extension's `verdict`:

| Extension `verdict` | Message published |
|---|---|
| absent | `agent.answer`, whose `answer` is the text parts joined with newlines |
| `feedback` | `agent.feedback`, whose `errors` are the text parts, one error per part |
| `verified` | `agent.verified`, whose `message` is the joined text, or absent when there is no text |

A verdict SHALL be honoured only when the coordination extension URI is listed in `message.extensions`.

The broker SHALL refuse the following:

| Condition | Error |
|---|---|
| A missing `taskId` (v1.2.0 accepts no new tasks over A2A) | `-32004` |
| A `taskId` that names no task | `-32001` |
| A task in a terminal state | `-32004` |
| A part other than `text` | `-32005` |
| A `role` other than `ROLE_USER` | `-32602` |
| A `contextId` different from the task's | `-32602` |
| Verdict metadata without the extension URI in `message.extensions` | `-32602` |
| An unknown verdict value | `-32602` |
| A `feedback` verdict with no text part | `-32602` |

A refused request SHALL publish nothing.

A `messageId` that was already accepted for the same task SHALL be acknowledged with the task's current state and SHALL NOT be delivered again. The response SHALL be the task after delivery.

#### Scenario: Plain text becomes an answer to the worker

- **GIVEN** an `INPUT_REQUIRED` task for `feat-x`
- **WHEN** an authenticated `SendMessage` with `returnImmediately = true` carries the text "use the v2 parser" for that task
- **THEN** `GET /messages/feat-x` SHALL contain an `agent.answer` with `from = "orchestrator"` and `answer = "use the v2 parser"`
- **AND** the returned task SHALL be `WORKING`

#### Scenario: Feedback verdict becomes agent.feedback with one error per part

- **WHEN** an authenticated `SendMessage` targets `feat-x`'s task with the extension URI in `extensions`, metadata `verdict = "feedback"` and text parts "test a fails" and "lint b"
- **THEN** `GET /messages/feat-x` SHALL contain an `agent.feedback` with `from = "orchestrator"` and `errors = ["test a fails", "lint b"]`

#### Scenario: Verified verdict completes the task

- **GIVEN** an `INPUT_REQUIRED` task for `feat-x` awaiting its verdict
- **WHEN** an authenticated `SendMessage` carries the extension URI and `verdict = "verified"`
- **THEN** an `agent.verified` for `feat-x` with `verified_by = "orchestrator"` SHALL be in `/log`
- **AND** the returned task SHALL be `COMPLETED`

#### Scenario: Verdict without activating the extension is refused

- **WHEN** a `SendMessage` carries `verdict = "verified"` metadata but its `extensions` list does not contain the extension URI
- **THEN** the response SHALL be a JSON-RPC error with code `-32602`
- **AND** nothing SHALL be published

#### Scenario: No task id means no new work

- **WHEN** an authenticated `SendMessage` carries a message without `taskId`
- **THEN** the response SHALL be a JSON-RPC error with code `-32004`
- **AND** no worktree, agent or task SHALL be created

#### Scenario: Message to a completed task is refused

- **GIVEN** a `COMPLETED` task
- **WHEN** an authenticated `SendMessage` targets it
- **THEN** the response SHALL be a JSON-RPC error with code `-32004`

#### Scenario: File part is refused

- **WHEN** a `SendMessage` carries a part with a `url` member
- **THEN** the response SHALL be a JSON-RPC error with code `-32005`

#### Scenario: Retried messageId is not delivered twice

- **WHEN** the same `SendMessage` with `messageId = "m-1"` is posted twice for `feat-x`'s task
- **THEN** `GET /messages/feat-x` SHALL contain exactly one corresponding `agent.answer`

### Requirement: SendMessage waits for an interrupted or terminal state by default

Unless the request's `configuration.returnImmediately` is `true`, `SendMessage` SHALL withhold its response until the task reaches `INPUT_REQUIRED`, `COMPLETED` or `CANCELED`. It SHALL then return the task in that state. With `returnImmediately = true`, it SHALL return immediately after delivery with the task's current state.

#### Scenario: Default call returns when the worker next needs input

- **GIVEN** an `INPUT_REQUIRED` task for `feat-x`
- **WHEN** an authenticated `SendMessage` without `returnImmediately` delivers guidance
- **AND** `feat-x` then publishes `agent.status` (`working`) and afterwards `agent.artifact` (`done`)
- **THEN** the response SHALL arrive only after the artifact
- **AND** it SHALL carry the task in `INPUT_REQUIRED`

#### Scenario: returnImmediately does not wait

- **WHEN** a `SendMessage` with `configuration.returnImmediately = true` delivers guidance to a task
- **THEN** the response SHALL arrive without any further worker event, with the task in `WORKING`

### Requirement: CancelTask records cancellation without process control

`CancelTask` SHALL behave as follows:

- On a task in a non-terminal state, it SHALL set the task to `CANCELED`. It SHALL deliver to the worker an `agent.answer` from `"orchestrator"`, which tells the worker that the orchestrator cancelled the task and that it should stop and stand by. It SHALL return the cancelled task.
- On a task that is already `CANCELED`, it SHALL return the task unchanged and deliver nothing.
- On a `COMPLETED` task, it SHALL return error `-32002`.
- When no task has the given `id`, it SHALL return error `-32001`.

`CancelTask` SHALL NOT stop any process, close any agent surface, or remove or modify any worktree or branch.

#### Scenario: Cancel marks the task and notifies the worker

- **GIVEN** a `WORKING` task for `feat-x`
- **WHEN** an authenticated `CancelTask` names it
- **THEN** the returned task SHALL be `CANCELED`
- **AND** `GET /messages/feat-x` SHALL contain an `agent.answer` from `"orchestrator"` telling it to stop and stand by
- **AND** the worker's worktree and branch SHALL still exist

#### Scenario: Cancelling a completed task fails

- **GIVEN** a `COMPLETED` task
- **WHEN** `CancelTask` names it
- **THEN** the response SHALL be a JSON-RPC error with code `-32002`

#### Scenario: Repeat cancel is idempotent

- **GIVEN** a task already `CANCELED` by `CancelTask`
- **WHEN** `CancelTask` names it again
- **THEN** the task SHALL be returned as `CANCELED`
- **AND** no second `agent.answer` SHALL be delivered

### Requirement: Unsupported optional A2A operations return the specified errors

The A2A face SHALL refuse the operations for capabilities it does not declare:

- `CreateTaskPushNotificationConfig`, `GetTaskPushNotificationConfig`, `ListTaskPushNotificationConfigs` and `DeleteTaskPushNotificationConfig` SHALL return `-32003` (`PushNotificationNotSupportedError`).
- `GetExtendedAgentCard` SHALL return `-32004` (`UnsupportedOperationError`).
- While streaming is not served, `SendStreamingMessage` and `SubscribeToTask` SHALL return `-32004`.

None of these SHALL change any state.

#### Scenario: Push configuration is not supported

- **WHEN** an authenticated `CreateTaskPushNotificationConfig` is posted
- **THEN** the response SHALL be a JSON-RPC error with code `-32003`

#### Scenario: Extended card is not supported

- **WHEN** an authenticated `GetExtendedAgentCard` is posted
- **THEN** the response SHALL be a JSON-RPC error with code `-32004`

#### Scenario: Streaming refused while not served

- **GIVEN** a broker that does not serve streaming
- **WHEN** an authenticated `SubscribeToTask` is posted
- **THEN** the response SHALL be a JSON-RPC error with code `-32004`

### Requirement: Verdict authority holds on the A2A face

No request without the session's root-authority credential SHALL cause an `agent.verified` or an `agent.feedback` to be published through the A2A face, or cause any task to reach `COMPLETED`. Enabling the A2A face SHALL NOT change which callers `POST /publish` accepts for those two message types.

#### Scenario: Worker without the credential cannot complete its own task

- **GIVEN** a `WORKING` task for `feat-x` and a client without the credential
- **WHEN** the client posts a `SendMessage` with `verdict = "verified"` for that task
- **THEN** the response status SHALL be `401`
- **AND** the task SHALL remain `WORKING`
- **AND** `/log` SHALL contain no `agent.verified` for `feat-x`

#### Scenario: Legacy gate unchanged with A2A enabled

- **GIVEN** `[broker.a2a] enabled = true`
- **WHEN** `POST /publish` receives `agent.verified` without the `X-Paw-Authority` header
- **THEN** the response status SHALL be `403`, exactly as with A2A disabled

### Requirement: Legacy and A2A faces share one coordination state

The A2A face and the legacy endpoints SHALL read and write one coordination state:

- A message accepted by `POST /publish` SHALL be reflected by `GetTask` and `ListTasks` in the same broker host.
- A message delivered through `SendMessage` or `CancelTask` SHALL appear in the worker's `GET /messages/:agent_id` inbox and in `GET /log` as the corresponding ordinary coordination message.
- That message SHALL be observed by the conflict detector and the learnings aggregator like any other.

#### Scenario: Legacy publish is visible over A2A

- **GIVEN** `[broker.a2a] enabled = true`
- **WHEN** `broker.sh question "which schema?"` is run in worktree `feat-x`
- **THEN** an authenticated `GetTask` for `feat-x`'s task SHALL return state `INPUT_REQUIRED`, with a last history message whose text is "which schema?"

#### Scenario: A2A guidance is visible to the worker's helper

- **WHEN** an authenticated `SendMessage` delivers "use the v2 parser" to `feat-x`'s task
- **THEN** `broker.sh poll` run in worktree `feat-x` SHALL print an `agent.answer` from `orchestrator` containing "use the v2 parser"
