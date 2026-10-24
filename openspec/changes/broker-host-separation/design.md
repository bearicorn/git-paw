## Context

`cmd_dashboard()` (`src/main.rs`, ~300 lines) is the broker's host. It builds `BrokerState` with the
log path, verify-on-commit nudge, seeded orchestrator CLI, role-gating roster and learnings
aggregator; calls `start_broker_with` with the session's watch targets and conflict config; installs
the SIGHUP handler; then runs the TUI, which reads `Arc<BrokerState>` directly. (v0.17.0
`classifier-retirement` deleted the attended auto-approve thread that used to be spawned here.) It refuses to run without `$TMUX`. Pause kills the
dashboard pane *to stop the broker*; stop relies on the pane's SIGHUP to flush the log.

Two shipped patterns inform the split: `src/mcp/` holds no state (`RepoContext::for_root` resolves
`broker_url` from the saved `Session`; tools talk HTTP and degrade to empty documents), and the drive
loop already consumes the broker over HTTP from another process. `GET /log` already accepts a
`since` cursor (`broker-runtime`, "GET /log returns the full broker message log");
`orchestrator-read-surface`, archived before this change, adds the bounded wait and the
cursor-holding empty response to it and to `broker::publish::fetch_log_entries_over_http`, and points
the drive loop's observer at the cursor.

The v0.17 change `broker-repo-identity` makes the log per-session
(`<session_state_dir>/<session_name>.broker.log`) and makes the stale-broker probe refuse another
repository's broker; `stop-without-confirmation` removes the stop prompt and `--force`.

DS-3: v0.17.0 `supervisor-boot-wiring` adds `WorktreeEntry.spec_id` and seeds the aggregator from
it (`seed_from_session`) where `cmd_dashboard` builds the broker. A hot-added agent is not mapped:
`/watch` carries no spec id, and the running aggregator was seeded at start.

## Goals / Non-Goals

**Goals:**
- The broker outlives, and is independent of, any view.
- The dashboard is a stateless HTTP client that can run anywhere and degrades gracefully.
- Start / recover / pause / stop / restart keyed on the broker host, with a fallback for sessions
  started before the split.
- Pane indices and existing persisted fields untouched.
- Decide, with a recommendation, where the host process lives.
- Carry the agent → spec seeding into the aggregator the host builds, and map hot-added agents
  (DS-3 remainder).
- Keep the receipt-after-panes order (failed start preserves state) without starving the host of
  session data.
- `git paw status` reports the broker from the host's liveness (PP-27).

**Non-Goals:**
- Whether the TUI survives, tabs/scrolling, a `[dashboard] enabled` switch — the dashboard-survival
  question stays open and downstream of this split.
- A daemon with pidfiles, or behavior when tmux is absent.
- New endpoints or `BrokerMessage` variants (the long-poll read surface is `orchestrator-read-surface`).
- The authority credential (`broker-verdict-authority` adds it to the host).

## Decisions

### D1 — Own process (`git paw __broker`) in a detached service surface — not a daemon yet

The drive loop cannot host the broker (it runs only under `--unattended`; a manual session still
needs a broker). The choice is (A) a hidden subcommand in a detached surface of the session, or (B) a
long-lived session-host daemon (`setsid`, pidfile) owning the broker and, when unattended, the drive
loop. **Recommendation: (A).** It reuses every lifecycle mechanism that works today —
`kill-session` delivers SIGHUP → `BrokerHandle` drop → flush; orphan detection (`getppid()==1`) and
tty-hangup exit apply unchanged; no pidfile or stale-pid recovery. The surface is named and outside
window 0, so persisted pane indices are untouched. (B) is the daemon a PTY backend would need;
building it now pre-pays for an undecided outcome. The **process boundary is identical** under both,
so moving to (B) later is a relocation, not a rewrite. *Cost of (A):* the broker still dies with the
tmux server, as today; detached-unattended survivability belongs to the tmux spike.

### D2 — The dashboard copies the MCP server's shape

`BrokerClient` (`src/broker/client.rs`) consolidates the hand-rolled HTTP helpers with bounded
timeouts: `publish_to_broker_http`, `register_watch_target_http`, `fetch_log_entries_over_http` (with
the cursor and wait `orchestrator-read-surface` added) and `fetch_log_over_http`,
`drive::fetch_status_over_http`, `inventory::fetch_status_agents_over_http` and the stale-broker
probe. `__dashboard` resolves the URL from the saved session's `broker_port` / `broker_bind` — not
through `RepoContext::for_root`, whose `broker_url_from_session` returns `None` unless the session is
`Active`, while restart-from-pause creates the dashboard before flipping `Paused` to `Active` — and
retries while no receipt is saved yet (a fresh launch saves it after the panes exist). It polls
`/status` and `/log?since=<seq>` on its 800–1000 ms idle cadence and feeds `BrokerLog` by cursor. An
empty `/log` holds `last_seq` at the cursor, so a restarted host (sequence numbers start over) is
detected from `/status` `uptime_seconds` going down, and the panel then re-reads from 0 without
duplicating what it holds. No `BrokerState`, no `BrokerHandle`, no `$TMUX` gate. A failed poll flips a
status-line indicator and keeps the last snapshot; it is never an exit condition.

### D3 — Start ordering and readiness

With a separate host, `start` does: open the session (env, dashboard pane 0, agent panes — one
terminal-port step, `open_session`) → spawn the `broker` service with the session name and the
per-session log path on argv → wait (≤10 s) for `/status` with the `git_paw` marker and this
repository's `repo_root` → register each agent through `POST /watch` (agent id, worktree, CLI, spec id)
→ save the receipt → readiness gate → inject boot prompts. A host that never answers (or exits first)
fails the launch with the port named and leaves session state for recovery.

*Amended 2026-10-28 against the merged terminal port:* the earlier order spawned the host before any
pane existed. A service surface is a surface *of a session*, and the port's `open_session` creates the
session together with its dashboard and agent surfaces (the launch argv fixtures pin that), so the host
starts right after the session opens instead. Nothing is lost: agent CLIs do nothing broker-facing until
their boot prompt, which still waits for `/status`; the dashboard renders degraded for the moment until
the host answers.

The receipt cannot move ahead of the host: v0.17 saves it only after the panes exist
(`cmd_start`, `launch_spec_session`, `cmd_supervisor`) and released *A failed start preserves persisted session
state* requires that order. v0.17's dashboard races the receipt (`cmd_dashboard` documents it); a
host that must answer before the receipt exists would lose that race every time — log path falling back
to `git-paw.broker.log` (`session::resolve_broker_log_path`), no watch targets, no `spec_id` seeding. So the launcher
hands the host what it needs instead (design call 3, shared with `broker-verdict-authority`, which adds
the credential on stdin): session name and log path on argv (not secret), and each agent through
`/watch` once its pane exists — the path `git paw add` already uses. Recovery and restart-from-pause
have a saved receipt, so the host seeds from it as before. The opsx role-gating roster needs nothing
from the receipt for correctness: the root's entry comes from `repo_root`, and an unmatched worktree is
a violation exactly as a matched coding worktree is.

### D4 — Pause keeps the dashboard

Because the dashboard is stateless, `pause` stops only the host; the dashboard shows "broker
unreachable" and reconnects when `start` recreates the host. Restart-from-pause recreates the
dashboard pane only if none is running.

### D5 — Fallback for sessions started before the split

A session started by an earlier binary and still alive after upgrade has no `broker` surface; its
dashboard pane *is* the broker. `pause` detects the missing surface and falls back to killing the
dashboard pane at `dashboard_pane`. `stop` is unaffected (kill-session).

### D6 — No approver moves with the broker

An earlier draft moved the attended auto-approve thread into `__broker`. v0.17.0
`classifier-retirement` deleted that thread (and `poll.rs`, `GIT_PAW_UNATTENDED`): no git-paw
process answers a permission prompt. The host therefore sends no keystrokes to any pane; prompt
detection and escalation stay in the `--unattended` drive loop, which runs in the foreground
`start` process, not in the host.

### D7 — The SIGHUP handler moves verbatim

`code-architecture` freezes the dashboard/`main.rs` SIGHUP `unsafe` path against *refactors*. This is
a specified change of location: the handler body moves into the host unchanged; the dashboard, which
no longer owns a `BrokerHandle`, keeps only its shutdown-flag path.

### D8 — Service surface is a port operation, not a tmux call

`spawn_service` / `stop_service` / `service_alive` join `TerminalBackend` ("run a service the human
does not watch"), so `start`, `recover`, `pause` never issue `new-window` / `kill-window` themselves.
tmux implements it as a detached named window; the fake records it.

### D9 — DS-3 remainder: seed at host start, map hot-added agents through `/watch`

The spec id is recorded at launch by v0.17.0 (`WorktreeEntry.spec_id`). When a saved receipt exists
(recovery, restart-from-pause) the host calls the aggregator's session seeding when it builds the
aggregator; on a fresh launch the launcher's `/watch` registrations carry the id (D3), and
`git paw add --from-spec` passes it the same way, so every agent is mapped. Agents with no spec stay unmapped,
and the aggregator's existing cross-spec / unknown handling applies. *Alternative rejected:* derive
the spec from the branch name — branch ↔ spec naming is a per-format convention, not a contract.

### D10 — `git paw status` reports the broker from the host's liveness (PP-27)

v0.17 prints "(running)" whenever the repository-scoped `/status` probe answers, so a process that
still answers after `git paw stop` reads as this session's running broker (PP-27). With a host surface
to ask, the broker line is "running" only when the probe answers *and* `service_alive` reports the
`broker` surface; a session started before the split (no surface) falls back to its terminal session's
liveness. A probe that answers with no live host is reported as a leftover, never as running.

## Risks / Trade-offs

- [Host vs first boot prompt race] → explicit bounded `/status` wait before any registration, receipt or prompt; failure is
  loud and preserves state.
- [Host vs receipt race] → the host never waits for the receipt on a fresh launch: name and log path on
  argv, agents through `/watch` (D3).
- [Stale data unnoticed during an outage] → indicator on every frame while a poll fails.
- [Log cursor after a host restart (sequence numbers restart)] → ingest by `(seq, timestamp)` identity
  with a restart detector; "no duplicates, history kept".
- [Hidden window surprises tmux users] → name it `broker`, document it; it shows the host's stderr.
- [Pre-split sessions] → pause fallback (D5).
- [Security] → the privileged process is headless and loopback-only; the visible pane holds nothing.

## Migration Plan

1. `BrokerClient`; `run_dashboard(client, shutdown)`; degraded state; cursor polling.
2. `__broker` extracted from `cmd_dashboard` (state build, lifecycle, SIGHUP/orphan exit).
3. `spawn_service` / `stop_service` / `service_alive` on the port (tmux + fake).
4. Launch paths: env → service (name + log path on argv) → `/status` wait → dashboard → agents →
   `/watch` registrations → receipt; recover, pause (+ fallback), restart-from-pause.
5. DS-3 remainder: host seeding from a saved receipt, `/watch` spec id field.
6. `git paw status` broker line from probe + host liveness (PP-27).
7. Docs; delete the `$TMUX` gate.

Rollback: revert; the additive `/watch` field is ignored by older binaries.

## Open Questions

- Should `__dashboard` accept an explicit `--broker-url` for a remote session? Recommendation: not in
  this change; the saved-session resolution covers the local case and MCP covers remote.
- Does the dashboard TUI survive the relocation? Deliberately not decided here (see Non-Goals); this
  split is what makes either answer cheap.
