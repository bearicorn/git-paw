## 1. Configuration and serving conditions

- [ ] 1.1 Add `A2aConfig { enabled: bool }` under `BrokerConfig`, with a serde default so an absent table means `false`. Write config tests for "Absent table means disabled" and "Enabled table parses and round-trips". Add a commented `[broker.a2a]` stanza to the init template, as `core-init` *Init generates default config.toml* requires.
- [ ] 1.2 In the `__broker` host, compute `a2a_served = enabled && is_loopback_bind(bind)` (the helper comes from v0.17.0's `broker-listener-origin-guard`). Print the non-loopback warning, and mount `POST /a2a` only when the face is served.
- [ ] 1.3 Write tests for "Disabled face answers 404", "Non-loopback bind refuses to serve A2A and warns" and "Enabled loopback broker serves the face". Update the `broker-runtime` route tests for the modified *Unknown routes return 404*: "Live watch registration route is documented" and "A2A route is unknown while the face is not served".

## 2. Task registry on the publish path

- [ ] 2.1 `BrokerState` gains a `TaskRegistry`, holding records keyed by agent and generation, cancel markers, and the accepted `messageId`s per task. It also gains a `tokio::sync::watch` change counter. When the face is served, `delivery::publish_message` folds `task_events_from(msg)` via `broker::tasks::apply`, and watch-target registration opens tasks. The lock discipline is unchanged: no guard is held across an `.await`.
- [ ] 2.2 Write unit tests showing that the registry fold equals the pure projection for the same events, and that a disabled face performs no fold.

## 3. JSON-RPC entry layer

- [ ] 3.1 `src/broker/a2a/rpc.rs`: content-type check (`415`); parse one request object; return `-32700`, `-32600` (missing `id`, batch array), `-32601` or `-32602`; build every A2A error with its `ErrorInfo` (`a2a-protocol.org`). Write tests for "Wrong content type is refused", "Batch request is invalid", "Unknown method" and "Unknown task carries ErrorInfo".
- [ ] 3.2 `src/broker/a2a/auth.rs`: a pure `authorize(header, credential) -> AuthDecision` using an in-tree constant-time byte compare. It runs before parsing. A failure answers `401` with `WWW-Authenticate: Bearer` and `-32000`/`UNAUTHENTICATED`. Write tests for "Missing credential is refused before dispatch", "Wrong credential is refused", "Session without a credential refuses everything" and "Credential never echoed" (inspect the response body, `/log` and captured stderr).
- [ ] 3.3 `src/broker/a2a/version.rs`: a pure `negotiate(header, query) -> Result<Version, VersionError>` comparing `Major.Minor`, where empty means `0.3`. Write tests for "Version 1.0 is accepted", "Missing version means 0.3 and is refused" and "Query parameter is honoured".

## 4. Operations

- [ ] 4.1 `GetTask`, with `historyLength` semantics. Write tests for "History length limits the returned messages" and "History length zero omits history".
- [ ] 4.2 `ListTasks`, per design D9:
  - sort newest first;
  - filters for `contextId`, `status` and `statusTimestampAfter`;
  - `pageSize` defaulting to 50, capped at 100, with `-32602` for zero or less;
  - an opaque cursor;
  - `nextPageToken` that is always present;
  - artifacts omitted by default.

  Write tests for "Newest first, paginated", "Status filter" and "Artifacts omitted by default".
- [ ] 4.3 `SendMessage`, per design D5:
  - every validation in the refusal table;
  - mapping to `agent.answer`, `agent.feedback` or `agent.verified` from `"orchestrator"`;
  - publishing on the blocking pool via `delivery::publish_message`;
  - `messageId` idempotency.

  Write tests for "Plain text becomes an answer to the worker", "Feedback verdict becomes agent.feedback with one error per part", "Verified verdict completes the task", "Verdict without activating the extension is refused", "No task id means no new work", "Message to a completed task is refused", "File part is refused" and "Retried messageId is not delivered twice".
- [ ] 4.4 Implement the blocking wait on the watch counter until the task is `INPUT_REQUIRED`, `COMPLETED` or `CANCELED`, plus the `returnImmediately` path. Write tests for "Default call returns when the worker next needs input" (drive the worker events from the test after the call starts) and "returnImmediately does not wait".
- [ ] 4.5 `CancelTask`, per design D7. Write tests for "Cancel marks the task and notifies the worker" (assert the worktree and branch still exist using a temp repo fixture), "Cancelling a completed task fails" and "Repeat cancel is idempotent".
- [ ] 4.6 Implement the unsupported operations: push config returns `-32003`, extended card returns `-32004`, and the streaming methods return `-32004` while not served. Write tests for "Push configuration is not supported", "Extended card is not supported" and "Streaming refused while not served".

## 5. Authority and cross-face end-to-end tests

- [ ] 5.1 In `tests/a2a_e2e.rs`, use a live broker host with a known injected credential. Test "Worker without the credential cannot complete its own task" and "Legacy gate unchanged with A2A enabled".
- [ ] 5.2 Using a live session fixture with a real worktree, test "Legacy publish is visible over A2A" (`broker.sh question`, then `GetTask`) and "A2A guidance is visible to the worker's helper" (`SendMessage`, then `broker.sh poll`). Also assert that the conflict detector and learnings observe an A2A-originated `agent.feedback`.

## 6. Docs

- [ ] 6.1 `docs/src/user-guide/security-posture.md`: add the A2A section. Extend `tests/security_posture_docs.rs` with "Posture chapter states the A2A privileges and limits".
- [ ] 6.2 `docs/src/configuration/`: add `[broker.a2a] enabled`, covering its default, the loopback-only rule and the warning. Add the A2A layer to `docs/src/architecture.md`, covering the broker host routes and the entry, core and wire split. The user chapter and the migration guide are in `a2a-migration-guide`.
- [ ] 6.3 Record `POST /a2a` and `[broker.a2a] enabled` in the major-1 contract manifest (`core-stability-contract`; see `a2a-legacy-coexistence`). Run `mdbook build docs/`.

## 7. Gates

- [ ] 7.1 Map every scenario to a named test. Run `just check` and `just deny` (no new dependency), and check the full suite against the merge-base.
- [ ] 7.2 Record the security review:
  - every A2A path authenticates before parsing;
  - the compare is constant-time;
  - the credential never appears in responses, logs or argv;
  - loopback-only serving;
  - no process, worktree or branch effect from any A2A operation;
  - the legacy `/publish` verdict gate is unchanged.
