## Context

`server::router` (`src/broker/server.rs`) mounts `/publish`, `/watch`, `/messages/{agent_id}`,
`/status` and `/log`. It has no request-level checks beyond content type and payload validation,
and no authentication: any client that reaches the port can publish any `BrokerMessage` variant.
`start_broker_with` (`src/broker/mod.rs`) binds `[broker] bind:port`, which defaults to
`127.0.0.1:9119`, and is called by `git paw __dashboard` (the dashboard pane process), which hosts
the broker in-process.

Every in-tree client addresses the broker by the URL `http://<bind>:<port>` (`BrokerConfig::url()`,
or `discover_broker_url` in `assets/scripts/_paw_common.sh`), which with the default bind is
`127.0.0.1`:

- `broker.sh`, `sweep.sh` and the post-commit git hook (`assets/hooks/post-commit.sh`) use `curl`,
  which sends `Host: <bind>:<port>`;
- `publish_to_broker_http`, `register_watch_target_http`, `fetch_log_entries_over_http`,
  `fetch_log_over_http` (`src/broker/publish.rs`), `probe_existing_broker` (`src/broker/mod.rs`),
  `fetch_status_agents_over_http` (`src/coordination/inventory.rs`) and `fetch_status_over_http`
  (`src/supervisor/poll.rs`) write raw HTTP/1.1 with `Host: <addr>` taken from the URL;
- the MCP server reaches the broker through `publish_to_broker_http` and
  `fetch_log_entries_over_http`, with a URL from `broker_url_from_session` (`src/mcp/mod.rs`), which
  maps a `0.0.0.0` bind to `127.0.0.1`;
- the dashboard reads the in-process `BrokerState` directly; its poll loop uses
  `fetch_status_over_http`.

None of these sends an `Origin` header. `docs-fetch.sh` talks only to the documentation site, never
to the broker.

## Goals / Non-Goals

**Goals:**

- Stop browsers from reaching the broker, whether by cross-origin requests or by DNS rebinding.
- Leave every non-browser loopback client unaffected, including clients using tunnels and
  port-forwards.
- Use one guard for the whole listener, so any route added later inherits it.

**Non-Goals:**

- Authenticating loopback clients. Any local process can still reach the broker; a credential for
  the verdict verbs is separate work.
- TLS, and any support for safe LAN exposure.
- CORS. No browser access is wanted at all.

## Decisions

### D1: Check the host part of `Host` only, not the port

A rebinding page sends `Host: attacker.example:<port>`. The hostname alone identifies the attack. A
tunnel such as `ssh -L 19119:127.0.0.1:9119` presents `Host: 127.0.0.1:19119`, and checking the port
would refuse it for no gain.

The accepted host names are:

- `localhost`, compared ASCII case-insensitively;
- any IPv4 literal in `127.0.0.0/8`;
- `[::1]`.

### D2: Reject the `null` origin; allow a missing `Origin`

Browsers send `Origin` on every non-GET request, including same-origin requests made after
rebinding. A rebinding `POST /publish` therefore fails the Origin check even before the Host check
applies. Sandboxed iframes and `file://` pages send `Origin: null`, which is refused. Non-browser
clients send no `Origin`, and that stays accepted.

### D3: Non-loopback bind gets an Origin-only check and a warning

With `bind = "0.0.0.0"` or a LAN address, the set of legitimate `Host` values is whatever names the
operator's network uses (the bundled helpers themselves send `Host: 0.0.0.0:<port>` in that case).
git-paw cannot enumerate them without new dependencies. In that configuration the broker is already
reachable from the network without authentication, and that is the larger exposure. The honest
response has three parts:

- keep Origin validation, which still stops browsers;
- skip Host validation;
- warn at start on stderr from `start_broker_with`, naming the bind address and pointing at the
  security-posture chapter.

### D4: One middleware, one pure decision

`decide_request_origin(host: Option<&str>, origin: Option<&str>, bind_is_loopback: bool)` returns
`Ok` or a typed rejection, and the middleware maps the rejection to `403`. `is_loopback_bind(&str)`
decides `bind_is_loopback` from `[broker] bind` (an IP literal that is loopback, or `localhost`).
The decision is pure and tested with a table. The layer wraps the router, so it runs before both
routing and the `404`/`405` fallbacks; a request that passes it is routed exactly as before. The
loopback flag reaches the layer through `router` (a parameter, or a field on `BrokerState`).

*Alternative rejected:* checking inside each handler. A route added later would have to remember to
call the check.

## Risks / Trade-offs

- **A reverse proxy forwards the original external `Host`.**
  → It gets `403`. The documented migration is to forward a loopback `Host`, or to address the
  broker directly.
- **A request with no `Host` header.** HTTP/1.0 clients may omit it, and HTTP/1.1 requires it.
  → On a loopback bind, a missing `Host` is refused as `forbidden host`. Every in-tree client sends
  one.
- **A behaviour change on the broker's HTTP surface.**
  → It is called out as **BREAKING** in the release notes and changelog, and it is limited to
  browser-shaped and foreign-host requests. The regression test over every in-tree client is the
  guarantee that git-paw's own traffic is unaffected.

## Migration Plan

1. Add the decision function, `is_loopback_bind`, and their unit table.
2. Add the middleware layer, plus router tests that show in-tree clients unaffected.
3. Add the start-time warning on a non-loopback bind.
4. Update the docs and the release notes.

To roll back, revert. No state is persisted.

## Open Questions

- Should a non-loopback bind be refused outright, rather than warned about? *Recommendation:* warn
  for now, because refusing would break existing LAN setups that the operator chose deliberately.
  Revisit if an authenticated remote path is introduced.
