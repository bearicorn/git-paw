## Context

After the relocation the orchestrator is an external, turn-based session. It needs to learn what
happened since it last looked, cheaply, and to learn when nothing has happened for too long. The
broker already keeps an ordered `message_log` with sequence numbers and `GET /log?since=N` filters
it (`delivery::full_log`), but every client fetches from 0 and filters itself, and there is no way
to block for the next entry. Per-agent rows in `/status` carry `last_seen`, which a repeated
`working` heartbeat keeps fresh, so "everyone is working" can coexist with a wave that has not moved
in days (the 13-day dogfood stall).

## Goals / Non-Goals

**Goals:**
- An event-driven read: block on the log cursor with a bounded wait.
- Every in-tree reader that consumes the log incrementally requests only new entries.
- A wave-level "no progress since T" signal, surfaced wherever a human or orchestrator looks.
- Entry points for both kinds of orchestrator: MCP-attached and shell-driven.

**Non-Goals:**
- Server push (SSE / WebSocket) — a long-poll over the existing endpoint covers a turn-based reader
  with no new transport.
- A `logs --follow` CLI (that belongs to the dashboard-survival decision downstream).
- Acting on staleness (nudging, escalating) — the drive loop's heartbeat and the orchestrator's
  judgment own reactions; this change only makes the condition visible.

## Decisions

### D1 — Long-poll on `/log`, bounded

`wait` (seconds, capped at 60) holds the request until an entry with `seq > since` is appended or the
wait elapses. The publish path signals a `tokio::sync::Notify` after appending; the handler checks
the log, awaits the notification or the timeout, and re-checks — never holding the state lock across
`.await` (`clippy::await_holding_lock` stays clean). Response shape is unchanged
(`{entries, last_seq}`), so existing clients and `sweep.sh` parsing keep working. One value changes
(design call 4): v0.17 computes `last_seq` as the maximum over the returned entries and reports `0`
when there are none (`server.rs:484`), so an empty answer rewinds the caller's cursor. An empty
response now reports `last_seq = since`, with or without `wait` — a cursor that rewinds to 0 is a bug
for every reader, not only for a waiter. A response with entries is unchanged.
*Alternative rejected:* SSE — a new transport and client code for a reader that acts once per turn.

### D2 — Progress is a branch tip moving or a wave phase advancing

Progress means the work moved: a branch tip moving or a wave phase advancing. The broker records
`last_progress_at` when it accepts an `agent.status` whose status or phase differs from that agent's
previous record, an `agent.artifact`, `agent.verified`, `agent.feedback` or `agent.advanced-main`,
and when the watcher observes that an agent's worktree `HEAD` commit changed (the watcher already
runs git in each worktree every tick; reading `HEAD` beside `git status` is one more call). Nothing
else is progress. In particular, under `classifier-retirement` the drive loop escalates every live
permission prompt as an `agent.question`, re-escalates it after each dedup window, and records an
`agent.learning` for it; all three keep arriving while a wave sits on an unanswered prompt, which is
exactly the stall this signal exists to show, so none of them counts. Uncommitted file churn and a
repeated status do not count either. The classification is one function next to the delivery
routing, so a future `BrokerMessage` variant is classified where its routing is decided. The drive loop's
heartbeat uses the same definition (`unattended-heartbeat-progress`, later in this release).
*Alternative rejected:* deriving staleness from `last_seen` — the heartbeat keeps it fresh by
design, which is exactly how the stall hid. *Alternative rejected:* any message other than a repeated
status — the drive loop's own escalations and learnings would keep a stuck wave looking alive.

### D3 — Threshold and surfaces

`[broker] wave_stall_minutes` (default 30, `0` disables the flag) sets when `stalled` turns true;
`idle_seconds` is always reported. `/status` carries `wave {last_progress_unix_secs, idle_seconds,
stalled, stall_threshold_seconds}`; `git paw status` prints one line ("wave stalled — no progress for
…") when stalled; the dashboard status line shows a badge (computed from the in-process `BrokerState`
it renders at v0.17; from `/status` once `broker-host-separation` makes it an HTTP client); MCP
`get_session_status` includes the object. 30 minutes sits just above the drive loop's 25-minute heartbeat, so an unattended run exits
on its heartbeat before an attended reader sees "stalled" for the same quiet period.

### D4 — Two entry points, one contract

`wait_for_events(since, timeout_seconds)` (MCP, read-only, timeout capped at the server's 60 s) and
`sweep.sh events <since> [wait]` both call the long-poll and return `{entries, last_seq, wave}`. The
helper prints JSON so a shell-driven orchestrator parses it with the tools it already uses.
Both degrade gracefully with no live broker (empty entries, `no_session`).

### D6 — `setup-cli` grants `sweep.sh events` read-only

v0.17's `cli-setup-consent` grants, on consent, an exact-path allow entry for each `sweep.sh`
subcommand that sends no keystrokes and publishes no verdict, and requires every subcommand to be
granted or listed as never granted, so a new verb must be placed. `events` only reads `/log` and
`/status`, so it is granted (`Bash(.git-paw/scripts/sweep.sh events *)`), making twelve allow
entries. The `cli-setup` delta copies v0.17's requirement text and changes only the list and the
count. git-paw never rewrites recorded grants itself: for a user who applied the allowlist before
this release, doctor's setup check reports the applied entries as out of date, and the user re-runs
`git paw setup-cli claude --undo` then `--allowlist` to pick up the entry.

### D5 — In-tree readers adopt the cursor

The cursor and wait go into the reader v0.17 already shares, `broker::publish::fetch_log_entries_over_http`
(optional `since` and `wait`). `HttpLogObserver` passes its own cursor as `since` (the client-side
filter stays as a guard). `sweep.sh`'s merge-order read keeps `since=0` because it needs the whole
history, and the MCP intent/conflict reconstructions need the full log by definition, so they call
the same reader with `since = 0` and no wait. The dashboard is not a reader here: at v0.17 it renders
`BrokerState` in-process. `broker-host-separation` (later in the archive order) moves this reader into
its `BrokerClient` and makes the thin dashboard poll by cursor.

## Risks / Trade-offs

- [Long-poll ties up a handler task] → bounded wait, capped; axum tasks are cheap; loopback only.
- [Progress misclassified] → unit table over every variant; a misclassification only shifts when the
  flag turns on, never what any component does.
- [An agent editing for a long time without committing reads as stalled] → accepted: uncommitted
  churn is not progress by definition; the threshold is configurable and the flag is informational.
- [A truly idle but healthy wave reads as stalled] → that is the point; the threshold is
  configurable and the flag is informational.

## Migration Plan

1. Broker: notify on append, `wait` param, progress tracking, `wave` in `/status`, config key.
2. Readers: `fetch_log_entries_over_http` cursor + wait; drive-loop observer cursor.
3. Surfaces: `git paw status`, dashboard badge, MCP tool + session status, `sweep.sh events`, and
   its read-only grant in the setup-cli grant table.
4. Docs.

Rollback: revert; every addition is optional on the wire.

## Open Questions

- Should the drive loop publish an escalation when the wave goes stale under `--unattended`?
  Recommendation: no — its heartbeat exit already re-engages the human; revisit if dogfood shows
  heartbeat exits being missed.
