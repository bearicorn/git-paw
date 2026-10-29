## Why

An A2A client starts from the Agent Card. The card tells the client where the server is, which
binding and protocol version to use, how to authenticate, and what the server can do. Without one,
the A2A face from `a2a-task-lifecycle` exists but a standard client cannot find or configure itself
against it.

The MILESTONE assumed two things about cards that the stable spec does not support:

- **The path.** It assumed `/.well-known/agent.json`. A2A v0.3.0 renamed the well-known URI to
  `/.well-known/agent-card.json` (spec §8.2; IANA registration §14.3).
- **One card per agent.** The well-known URI identifies one card per origin. Workers are not A2A
  servers: they never execute A2A requests, because the broker does that on their behalf.

## What Changes

- **`GET /.well-known/agent-card.json`** on the broker origin. It is served only while the A2A
  face is served, and needs no authentication: card discovery is public, as §8.2 and §14.3 intend.
  The listener's Host/Origin guard still applies.
- **One card for the session broker.** The card carries:
  - the name `git-paw session <session name>`, and the git-paw version as the card `version`;
  - one `supportedInterfaces` entry: `{url: <broker_url>/a2a, protocolBinding: "JSONRPC",
    protocolVersion: "1.0"}`;
  - capabilities: `streaming` (true only when `a2a-task-streaming` is served),
    `pushNotifications: false` and `extendedAgentCard: false`, plus the coordination extension v1
    with `required: false`;
  - an HTTP Bearer security scheme and a security requirement for it;
  - default input and output modes `text/plain` and `application/json`.
- **Three capability skills, not per-agent skills.** The skills are `observe-workers`,
  `steer-worker` and `record-verdict`. Workers are addressed as tasks, which a client discovers
  with `ListTasks`. The card does not change when agents are added or removed.
- **Caching.** The response carries `Cache-Control: max-age=60` and an `ETag` derived from the
  card content. `If-None-Match` gets a `304` (§8.6).
- **No secrets in the card.** It carries no credential, no worktree path and no agent id.
- MILESTONE's "formal skill declarations replacing curl templates" is corrected. The card's skills
  describe the session's A2A capabilities for A2A clients. The worker-facing coordination skill and
  `broker.sh` are a different audience and stay, per `a2a-legacy-coexistence`.

## Capabilities

### New Capabilities

<!-- None. `broker-a2a` is introduced by `a2a-wire-model`. -->

### Modified Capabilities

- `broker-a2a`: adds the requirements for the card route, the card content, the capability skills
  and card caching.

## Impact

- **Code:**
  - `src/broker/a2a/card.rs`: the pure function `build_agent_card(view) -> AgentCard`, built from
    the session name, broker URL, git-paw version and whether streaming is served.
  - The route handler with ETag and `If-None-Match` handling.
  - The router mounts the route only while the face is served.
- **Wire:** one new public JSON document.
- **Enum-variant ripple:** none.
- **Dependencies:** none. The ETag is a hash from the standard library over the serialized card.
- **Cross-version dependencies:**
  - Requires `a2a-wire-model` (the card DTOs), `a2a-task-lifecycle` (the serving conditions, and
    the interface the card advertises) and v0.17.0's `broker-listener-origin-guard`.
  - Relies on v0.17.0 `broker-repo-identity`. Each repository's broker has its own port, so each
    session has its own origin, and therefore its own well-known card.
  - Leaves room for v1.4.0. A catalog of session cards across repositories can come from the
    v1.1.0 registry (`multi-repo-registry`), and remote access needs a public URL in
    `supportedInterfaces`. Neither changes this card's shape.
- **Security / safety:** a public card on loopback reveals the session name and capabilities, and
  nothing more. `GET /status` already reveals more than that, with no authentication.
