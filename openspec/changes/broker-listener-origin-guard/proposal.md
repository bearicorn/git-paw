## Why

The broker is an HTTP server (axum, `src/broker/server.rs`) on the operator's loopback interface,
bound to `127.0.0.1:<[broker] port>` (default `9119`) by `start_broker_with` in the process that
runs the dashboard pane (`git paw __dashboard`). It authenticates nothing: `POST /publish` accepts
every message variant — including `agent.answer`, `agent.feedback` and `agent.verified` — from any
client that can reach the port. A web page that the operator opens in a browser can reach it too,
through DNS rebinding: the attacker's hostname is re-resolved to `127.0.0.1`, so the page is
"same-origin" with `http://<attacker-host>:9119`. From there the page can:

- read `GET /log`, which is the whole session's coordination traffic;
- post an `agent.answer`, `agent.feedback`, `agent.verified`, `agent.question` or `agent.intent`
  into the bus.

The text of an `agent.answer` or `agent.feedback` is read as authoritative guidance by coding agents
that run with auto-approved execution. A rebinding attack is therefore remote prompt injection into
agents that have shell access.

The MCP Streamable HTTP transport specification requires local servers to validate `Origin` for
exactly this reason. The same reasoning applies to git-paw's broker. Because the guard wraps the
whole listener, any route added to the broker later inherits it.

## What Changes

- **Host validation.** Every request is checked before routing. When the broker is bound to a
  loopback address (the default, `127.0.0.1`), the host part of the `Host` header must be
  `localhost`, an IPv4 address in `127.0.0.0/8`, or `[::1]`. The port is not checked, so SSH tunnels
  and local port-forwards keep working.
- **Origin validation.** If an `Origin` header is present, its host must also be loopback. The
  serialized origin `null` is refused. A request with no `Origin` header, which is what every
  non-browser client sends, is unaffected.
- **Rejections** answer `403` with `{"error":"forbidden host"}` or `{"error":"forbidden origin"}`
  and change no broker state.
- **Non-loopback bind.** When `[broker] bind` is set to a non-loopback address, the broker cannot
  know its public names, so Host validation is skipped. Origin validation still applies. The process
  that starts the broker prints a warning at start that the broker is reachable from the network
  without authentication.
- **BREAKING — for browser-shaped and foreign-host clients only.** Two kinds of request are now
  refused with `403`:
  - a browser request carrying a non-loopback (or `null`) `Origin`;
  - a request that reaches a loopback-bound broker with a non-loopback `Host` (a DNS-rebinding page,
    or a reverse proxy forwarding an external `Host`).

  No in-tree client is affected. Every one addresses the broker by its configured URL
  `http://<bind>:<port>` and sends no `Origin`: the bundled `broker.sh` and `sweep.sh` helpers and
  the post-commit git hook (`curl`), the coordination skill's rendered `curl` examples, the Rust
  HTTP clients in `src/broker/publish.rs`, `src/broker/mod.rs` (the stale-broker probe),
  `src/coordination/inventory.rs` and `src/supervisor/poll.rs`, the MCP server (which reaches the
  broker through those clients), and the dashboard (in-process state plus the poll loop's
  `/status` client). `docs-fetch.sh` does not talk to the broker. **Migration:** address the
  broker by a loopback name; a reverse proxy must forward a loopback `Host` header. The release
  notes and changelog carry this as a breaking change.

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `broker-runtime`: adds requirements for Host and Origin validation on every request, and for the
  start-time warning on a non-loopback bind.
- `core-security-posture`: the posture chapter documents the guard and what it does not cover.

## Impact

- **Code:**
  - `src/broker/server.rs`: one `axum::middleware::from_fn` layer on the whole `router`, so every
    route (`/publish`, `/watch`, `/messages/{agent_id}`, `/status`, `/log`) and the `404`/`405`
    fallbacks sit behind it. It calls a pure
    `decide_request_origin(host, origin, bind_is_loopback) -> Result<(), OriginRejection>`; a small
    `is_loopback_bind(&str)` helper decides whether the configured bind is loopback.
  - `src/broker/mod.rs`, in `start_broker_with`: decide whether `config.bind` is loopback via
    `is_loopback_bind`, hand that to the router, and print the warning on a non-loopback bind.
- **Wire:** a new `403` response shape. Every other response is unchanged.
- **Tests:**
  - A unit table for the decision function and `is_loopback_bind`.
  - `oneshot` router tests in `src/broker/server.rs` for each scenario.
  - An integration test in `tests/broker_integration.rs` over real TCP with `curl`-style headers.
  - A regression test showing that every in-tree broker client (listed above) is still accepted.
- **Docs:**
  - `docs/src/user-guide/security-posture.md`, with a new "Browser exposure" section.
  - `docs/src/user-guide/coordination.md`, for the 403 shape.
  - `docs/src/configuration/README.md`, for the `[broker] bind` field's warning.
  - CHANGELOG / release notes: a **BREAKING** entry (the changelog is generated by git-cliff, so
    it comes from the commit's `!` + `BREAKING CHANGE:` footer).
- **Enum-variant ripple:** none (`BrokerMessage` is untouched).
- **Dependencies:** none. `axum::middleware` is part of axum's default features.
- **Cross-change dependencies:**
  - `broker-repo-identity` (v0.17.0, also `broker-runtime`) MODIFIES *Stale broker detection*,
    *GET /status returns broker and agent state* and *Dashboard subcommand starts broker and
    dashboard*; this change only ADDs requirements, so the deltas do not collide. Its repo-aware
    probe still sends `Host: <bind>:<port>` (loopback by default) and no `Origin`, so it passes the
    guard; whichever lands second keeps the probe regression test green.
  - Independent of every other pending change.
