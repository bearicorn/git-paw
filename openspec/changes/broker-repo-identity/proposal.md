## Why

Two repositories using git-paw on one machine silently share a broker, and the second one
is wrecked without an error.

Every repository defaults to `[broker] port = 9119`. Before binding, `start_broker_with`
probes `GET /status`, and classifies the listener as a git-paw broker purely on the body
containing `"git_paw": true`. `/status` reports no repository identity, so a broker for
*any* repository matches. On a match the caller gets `BrokerHandle::reattached(url, state)`
— a handle wrapping the caller's own fresh, empty state, with no watchers started, no
conflict detector and no flush thread. The `watch_targets` and `conflict` arguments are
silently dropped.

The effect for the second repository: its agents publish into the first repository's broker
(their `GIT_PAW_BROKER_URL` is the shared port), while its dashboard renders an empty state
that nothing ever updates. The first repository's broker, meanwhile, receives a stream of
messages from agents it does not know. It is the same blast-radius class as the
cross-agent misrouting fixed earlier in the project: work is reported to the wrong place,
silently.

The log makes it worse. Every launch path sets `broker_log_path` to
`<session_state_dir>/broker.log` — one file for all repositories — and `purge` deletes that
file. Purging repository B deletes the log repository A's live broker is still writing.

The existing *Stale broker detection* requirement mandates the reattach on the marker alone,
so this is a spec change, not just a fix.

## What Changes

- `GET /status` reports `repo_root`, the canonical path of the repository the broker serves.
- The stale-broker probe reattaches only to a broker for the caller's own repository. A
  git-paw broker for a different repository — or one that does not report a repository —
  is refused with an error naming the port, and the other repository when known, and
  telling the user to set a distinct `[broker] port`.
- The broker log becomes per-session: `<session_state_dir>/<session_name>.broker.log`.

Deliberately not included: automatically choosing a free port. That would mean persisting a
dynamic port into session state and every place the URL is rendered (boot blocks,
environment, helpers). A loud, actionable error is the small fix; automatic port allocation
can follow if the error proves common.

## Capabilities

### New Capabilities

<!-- none -->

### Modified Capabilities

- `broker-runtime`: *Stale broker detection* (reattach requires matching repository),
  *GET /status returns broker and agent state* (adds `repo_root`), and *Dashboard subcommand
  starts broker and dashboard* (per-session log path).

## Impact

- `src/broker/server.rs` — `/status` body gains `repo_root`.
- `src/broker/mod.rs` — `probe_existing_broker` parses `repo_root` and compares it;
  `ProbeResult` gains a variant for "git-paw broker, different or unknown repository";
  `BrokerError` gains a variant for the refusal. `BrokerState` must know its repository.
- `src/commands/start.rs`, `src/commands/supervisor.rs`, `src/main.rs` — the four places that
  build `<session_state_dir>/broker.log`; the purge cleanup that deletes it.
- Messages from a new broker to an old one are unaffected; an old broker without `repo_root`
  is treated as unknown and refused, which is the safe direction.
- **Delta coordination**: `broker-runtime` is not touched by any other in-flight change.
