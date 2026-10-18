## Why

The v0.17 dogfood ran a relocation rehearsal and produced two findings about how an external
orchestrator *reads* a session (GP-33):

- **A turn-based reader that polls pane state misses events.** The external orchestrator sampled
  panes every 20 s and silently missed a permission prompt: the agent answered one and hit the next
  inside the same window, so both samples read "prompt" and no transition fired. The in-process loop
  has no such failure because it keys on command identity. The orchestrator's read surface must
  therefore be *event-driven* — a cursor over the broker's log that it can block on — not
  capture-diff polling.
- **A stalled wave is invisible.** A dogfood session sat on an unanswered prompt for ~13 days with
  every pane alive and the broker reporting each agent `working`. Per-agent heartbeats do not reveal
  that nothing has *progressed*; a wave-level staleness signal is needed.

At the tip, `GET /log` already accepts a `since` cursor, but no client uses it: the drive loop's
`HttpLogObserver` (via `broker::publish::fetch_log_entries_over_http`), the MCP intent/conflict
readers and `sweep.sh` (`/log?since=0`) all fetch the whole log and filter client-side, so every poll
is O(n) in log length; and nothing can *wait* for the next event. `/status` reports per-agent rows
only.

Archive position 3 of 20 for v0.18: the broker read side lands on the readers v0.17 ships, before the
host split (`broker-host-separation`, position 13).

## What Changes

- **Long-poll on the existing cursor.** `GET /log?since=<seq>&wait=<seconds>` returns as soon as an
  entry newer than `since` exists, or after the bounded wait (capped at 60 s) with no entries. An empty
  response holds `last_seq` at `since`, with or without `wait` (v0.17 reports `0`, rewinding every
  reader's cursor); otherwise a request without `wait` behaves exactly as today.
- **Readers use the cursor.** The existing `broker::publish::fetch_log_entries_over_http` gains the
  cursor and the wait; the drive loop's message observer and the bundled helper request only entries
  after the last sequence they have seen. The dashboard keeps reading the broker state in-process until
  `broker-host-separation` turns it into an HTTP client that polls by cursor.
- **Wave staleness.** The broker tracks the last time the wave *progressed* — a wave phase advancing
  (an agent's status or phase changing, an artifact, a verdict, feedback, an advanced main) or a
  branch tip moving (the watcher sees an agent's worktree `HEAD` change). Nothing else counts: not a
  repeated status, not uncommitted file churn, and not the drive loop's own escalations,
  re-escalations or learnings, which keep arriving while a wave sits on an unanswered prompt. `/status`
  reports a `wave` object (last progress time, idle seconds, a `stalled` flag against a configurable
  threshold, default 30 minutes). `git paw status` (human and `--json`), the dashboard status line, and MCP
  `get_session_status` surface it.
- **Orchestrator entry points.** A read-only MCP tool `wait_for_events(since, timeout_seconds)` and a
  bundled `sweep.sh events <since> [wait]` verb expose the long-poll, each returning the new entries,
  the new cursor and the wave status — so a waiter that times out still learns the wave is stalled.
  The orchestrator skill (`orchestrator-skill-export`) teaches this surface instead of pane polling.
- **`setup-cli` grants the new verb read-only.** v0.17's `git paw setup-cli claude --allowlist` grants
  exactly eleven helper entries and requires every `sweep.sh` subcommand to be granted or listed as
  never granted. `sweep.sh events` sends no keystroke and publishes nothing, so it joins the granted
  read-only `sweep.sh` subcommands (twelve entries).

## Capabilities

### New Capabilities

<!-- None. -->

### Modified Capabilities

- `broker-runtime`: long-poll on `/log`; an empty `/log` response holds the cursor; the drive loop
  requests only new entries; wave staleness in
  `/status`; `git paw status` surfaces a stalled wave; the stall threshold is configurable.
- `broker-dashboard`: the dashboard shows a stalled-wave indicator.
- `mcp-server`: `wait_for_events` read tool; `get_session_status` reports wave staleness.
- `broker-agent-helper`: `sweep.sh events` verb.
- `cli-setup`: the allowlist grant table grants `sweep.sh events` read-only (twelve allow entries).

## Impact

- **Predecessors (archive order):** position 3, after `supervisor-role-typing` (1) and
  `setup-cli-adopted-entries` (2). It no longer waits for `broker-host-separation`: the wait is served
  by the broker wherever it runs, and the cursor goes into the readers v0.17 ships (the drive loop's
  `HttpLogObserver`, `src/supervisor/drive.rs:2769`, and the MCP intent/conflict readers, all through
  `broker::publish::fetch_log_entries_over_http`; the dashboard reads `BrokerState` in-process). v0.17:
  `broker-repo-identity` MODIFIES *GET /status returns broker and agent state* (adds `repo_root`) —
  this change does not modify that requirement; the `wave` object is a separate ADDED requirement on
  the same response. `cli-setup-consent` creates the `cli-setup` capability and its requirement
  *Allowlist entries are path-scoped to git-paw's helper scripts*; this change's MODIFIED delta copies
  that requirement's v0.17 text and only adds `events`, so `cli-setup-consent` MUST archive first.
  `classifier-retirement` makes the drive loop escalate every live permission prompt (an
  `agent.question`) and record a learning for it; this change classifies both as not progress. The
  documented upgrade remedy (doctor reports the applied allowlist out of date → `setup-cli claude
  --undo` → `--allowlist`) only clears the warning once `setup-cli-adopted-entries` (position 2) fixes
  PP-22 (apply records pre-existing matching entries); that change does not modify *Allowlist entries
  are path-scoped to git-paw's helper scripts*, so this change's `cli-setup` delta stays on the
  released text.
- **Cross-version:** v0.20 `skill-package-deploy` moves the helper scripts and must move every
  recorded setup-cli grant, including the `sweep.sh events` entry added here. This release's
  `unattended-heartbeat-progress` (archive position 7) uses the same progress definition for the drive
  loop's heartbeat (branch tip moving or wave phase advancing; the drive loop's own re-escalations
  and learnings excluded).
- **Successors:** `broker-host-separation` (moves the cursor/wait reader into its `BrokerClient`; its
  thin dashboard polls by cursor and reads the `wave` object from `/status`), `agent-hook-and-tmux-guards`
  (also edits `src/cli_setup/facts.rs`), `drive-loop-functional-core` (observations arrive by cursor),
  `orchestrator-skill-export` (skill teaches the event surface).
- **Code:** `src/broker/server.rs` (`log` handler: `wait` param, bounded wait on a publish
  notification without holding a lock across `.await`; `status` handler: `wave`), `src/broker/mod.rs`
  + `delivery.rs` (last-progress tracking, progress classification, notify on append),
  `src/broker/watcher.rs` (branch-tip observation per worktree),
  `src/cli_setup/facts.rs` (grant table gains `sweep.sh events`),
  `src/broker/publish.rs` (`fetch_log_entries_over_http` gains `since` + `wait`),
  `src/supervisor/drive.rs` (`HttpLogObserver` passes its cursor), `src/commands/status.rs`,
  `src/dashboard/view.rs` (badge from the in-process `BrokerState`),
  `src/mcp/tools/{coordination,session}.rs` + `src/mcp/query/session.rs`, `src/mcp/server.rs`
  (`ALL_ALLOWED_TOOL_NAMES`), `src/config/broker.rs` (`wave_stall_minutes`),
  `assets/scripts/sweep.sh` (`events`; the `.git-paw/scripts/` copy is gitignored and written by
  `git paw init`, so there is no tracked copy).
- **Wire:** additive — new query parameter, new `/status` object; `BrokerMessage` unchanged.
- **Config:** optional `[broker] wave_stall_minutes` (default 30; `0` disables the flag).
- **Upgrade:** git-paw never rewrites recorded setup-cli grants. A user who applied
  `setup-cli claude --allowlist` under v0.17 sees doctor's setup check report the applied entries as
  out of date, and re-runs `git paw setup-cli claude --undo` then `--allowlist` to add the `events`
  entry. Until then `sweep.sh events` prompts as any ungranted command does.
- **Tests:** server tests (wait returns early on publish, times out empty with `last_seq = since`, cap
  enforced, an empty response without `wait` also holds `last_seq = since`); staleness classification unit tests (repeated identical status, uncommitted file
  churn, escalations, re-escalations and learnings are not progress; a moved branch tip is);
  setup-cli grant-table tests (twelve entries, `events` granted, coverage of every subcommand);
  status / dashboard / MCP / helper surfaces; drive-loop observer requests `since=<cursor>`.
- **Docs:** `docs/src/user-guide/coordination.md` (event surface), `user-guide/mcp.md`
  (`wait_for_events`), `user-guide/dashboard.md`, `cli-reference.md` (`status` wave line),
  configuration reference (`wave_stall_minutes`), `user-guide/cli-setup.md` and
  `user-guide/security-posture.md` (the grant table lists `sweep.sh events`).
- **Enum-variant ripple:** none; progress classification matches existing variants in one function
  (`src/broker/delivery.rs`), which is listed as a semantic-handling site for future variants.
- **Security / safety:** read-only; loopback-only as every endpoint; the wait is bounded so a client
  cannot pin a handler indefinitely; no new write path. The new setup-cli grant is path-scoped to one
  read-only subcommand; it sends no keystroke and publishes no verdict, and the existing tamper
  protection over `.git-paw/scripts/` covers it.
- **Dependencies:** none (tokio's existing sync primitives).

## Rebase note (2026-10-25): audit against released v0.17.0

- Moved to archive position 3, before `broker-host-separation`: task 2.1 adds the cursor and wait to
  the existing `broker::publish::fetch_log_entries_over_http` (drive loop `HttpLogObserver`, MCP
  intent/conflict readers) instead of a `BrokerClient` that does not exist yet; `broker-host-separation`
  absorbs that reader later.
- Dropped the "the dashboard client SHALL request only entries after…" clause: before the host split
  the dashboard reads `BrokerState` in-process; cursor polling belongs to `broker-host-separation`. The
  stalled-wave indicator reads the in-process wave state now and `/status` after the split.
- Design call 4: v0.17's `/log` reports `last_seq = 0` when nothing is new (`server.rs:484`), so the
  "times out empty → `last_seq = 7`" scenario was false. An empty response now holds `last_seq` at
  `since`, with or without `wait`; the "without `wait` unchanged" sentence is narrowed to match.
- Task 3.1 adds `wait_for_events` to the closed `ALL_ALLOWED_TOOL_NAMES` registry test.
- `.git-paw/scripts/sweep.sh` is gitignored (written by `git paw init`), not tracked.
- The upgrade remedy (doctor out of date → `setup-cli --undo` → `--allowlist`) depends on
  `setup-cli-adopted-entries` (PP-22); the `cli-setup` MODIFIED delta stays on the released text.
