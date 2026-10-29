## Context

The dashboard is MVU at the tip this change was written against:

- `src/dashboard.rs` holds `Model { rows, status_line, broker_log, quit, panel_height }`, the
  messages `Msg::{Key, Snapshot, BrokerIngest}`, and a pure `update`.
- `src/dashboard/view.rs::render` draws four segments: the title, a stateless agent `Table`, the
  status line, and the broker log.
- The roster code is `AgentRow`, `format_agent_rows`, `arrange_with_supervisor_pinned`,
  `status_symbol` and `format_status_line`. `format_age` is also used for ages.
- `src/dashboard/broker_log.rs` holds the bounded ring buffer, the chips, the overlay, and
  `collapse_status_runs`, which collapses heartbeats only when they are adjacent across all
  agents.

After v0.18 the dashboard is a thin HTTP client of the `__broker` host. It reads
`GET /status` (reachability, the wave indicator) and `GET /log?since=<seq>` (the event cursor
from `orchestrator-read-surface`). It holds no `BrokerState`.

The dashboard pane is created whenever the broker is enabled:

- `commands/start.rs` creates it on the bare and spec paths and records `dashboard_pane =
  Some(0)`.
- `commands/supervisor.rs` creates it on the coordinated path.
- `commands/recover.rs` creates it when the session recorded a broker.
- `restart_from_pause` creates it when `broker_port.is_some()`.
- `Session::agent_pane_offset` derives the first agent pane from `mode` and `broker_port`. v0.18
  `pane-less-session-parity` adds `Session.layout` and moves the offset to `dashboard_pane`.

Usage (v0.19 `worker-usage-budget`) is computed when it is asked for, not stored:

- a provider usage facet (`CliProvider::usage` → `Known{…} | Unknown(reason)`) reads the report
  of a CLI session that git-paw launched;
- `status --json` exposes each worktree's `usage` and a workers-only `usage_total`;
- the drive loop keeps its one-shots' session ids **in memory** for the run and checks
  `budget_max_tokens` (input plus output) and `budget_max_cost` (`[drive]` after the v0.21
  vocabulary split).

Nothing outside the drive-loop process can see the one-shot ids today.

`git paw status` (`commands/status.rs`) prints `branch → cli (path)` per worktree. It shows no
broker status, phase or pane. So the roster has **no** human substitute today, although
MILESTONE's decomposition table says it does.

## Goals / Non-Goals

**Goals:**

- The dashboard is off by default. Turning it on needs no other setting, and existing setups
  keep it.
- The dashboard is a usage-and-events view. The roster has a real home in `git paw status`.
- One source of usage figures shared by the dashboard, `status --json` and the drive loop.
- A pure view model that can be tested without a terminal.

**Non-Goals:**

- A price table or any cost git-paw computes itself (v0.19 non-goal).
- Enforcing caps in the dashboard. Only the drive loop enforces them.
- A public `git paw dashboard` command (see Open Questions).
- Tabs, or a configurable size for the dashboard row.
- Changing what the broker log shows: chips, overlay, row format and activity phrase stay.

## Decisions

### D1 — Opt-in resolution, and the dashboard requires the broker

The dashboard is enabled for a launch when the launch has `--dashboard`, otherwise when the
merged config says `[dashboard] enabled = true`, and is off by default. It is created only for
a session that runs the broker host.

`--dashboard` without a broker fails before anything is created, because the user asked
explicitly. A config-enabled dashboard in a bare session without a broker silently adds no pane,
because that key is a standing preference and a notice on every start would be noise. The docs
state this.

The key lives in the existing `[dashboard]` table. The vocabulary split does not touch
`[dashboard]`: `default_mode` covers modes and `[drive]` covers the drive loop.

Alternatives considered:
- `default_mode`-style top-level key `dashboard = true`: rejected. Every other dashboard key is
  already in `[dashboard]`.
- On by default for coordinated sessions only: rejected by the user decision.

### D2 — Back-compat goes through the config version, not a heuristic at start

"Absent means off" is a change of meaning, and the config schema version exists for exactly
that. The v1→v2 step that `supervisor-vocabulary-split` adds also sets `[dashboard] enabled =
true` when a version-1 document:

- sets `[broker] enabled = true`. In v0.18–v0.20 the dashboard exists exactly when the broker
  is enabled (`supervisor-pane-removal`).
- has a `[dashboard]` table. That table overlays the global one as a whole (`merged_with`), so
  without this rule a repo file that only tunes `[dashboard.broker_log]` would hide a global
  file's implied `enabled = true`.

`[supervisor] enabled` alone does not imply the dashboard. Since v0.18 such a session has no
broker host, and leaving it out keeps the vocabulary split's row-by-row "v1 path loads as v2
path" equality intact.

The rule is per document and pure. A version-2 file is read as written.

Two writers would lose the implication when they stamp a file, so they write `[dashboard] enabled
= true` explicitly in that case:
- `init`'s `version = 2` backfill, which by the vocabulary-split rule stamps a file that has
  `[broker] enabled = true` and no `[supervisor]`;
- the `add-cli` / `remove-cli` rewrite.

Alternatives considered:
- Off for everyone, plus a one-time notice: rejected. It breaks "existing configs keep it" and
  the "no breaking changes" theme of v0.21.
- Imply for every version-1 file: rejected. A global file holding only `[clis.*]` would turn the
  dashboard on in every new repository.

If `supervisor-vocabulary-split` did not land, this change would introduce version 2 itself.

### D3 — "Has a dashboard" is recorded, never re-derived from config

`dashboard_pane` is written only when a pane is created. A session *has a dashboard* when it
records `dashboard_pane`, or when it has no `layout` record (written before v0.18) and recorded a
broker. That is today's implicit rule, kept for old files.

`Session::agent_pane_offset` becomes `dashboard_pane + 1` when the session has a dashboard and 0
otherwise. A session that records neither `dashboard_pane` nor a layout keeps v0.18
`supervisor-pane-removal`'s earlier (pre-relocation) rules, so a live supervised session from an
older binary still resolves its agents from pane 2. Restart from pause, recovery, `add` / `remove` re-tiling, status, liveness and the
keystroke senders (the drive loop's sweep, and `sweep.sh approve` through the discovery roster)
all read this rule. Since v0.17.0 no git-paw component approves prompts on its own; these senders
only nudge, submit buffered input, or run the explicit approve.

A session started before v0.21 keeps its pane. The existing precedent agrees: recovery already
uses the original broker config, not the current config.

No new field is needed: v0.18's `layout` record tells old files from new ones.

### D4 — Usage comes from the shared query; events come from the broker cursor

The dashboard calls the same library query that `status --json` uses. That query takes the
session's worktree entries and recorded invocations, the provider usage facet and the config
caps, and returns a usage snapshot. It runs on a reader thread every 10 s, and the snapshot
reaches the loop as a message. Transcript reads are file I/O of unbounded size, so they must not
run on the input path. The draw loop drains the channel each wake-up, which is at most 1 s.

Events keep the v0.18 cursor read (`/log?since=<last_seq>`) on each tick, never the whole log.
The long-poll `wait` stays for the external orchestrator, because a blocking read in the draw
loop would stall input and the loop redraws on its tick anyway.

Alternatives considered:
- A `GET /usage` broker endpoint: rejected. It is a new wire contract just before the freeze,
  and it computes in a second process what `status --json` computes in-process. The dashboard
  resolves its session from the repository anyway (v0.18 D2), so it is always local.
- Long-poll on a thread: deferred. It adds a wake pipe for no visible gain at a 1 s cadence.

This changes v0.18's "data comes only from the broker client" rule. The dashboard still holds no
broker state and writes nothing.

### D5 — Orchestrator invocations are persisted

v0.19 keeps one-shot session ids in drive-loop memory, which no other process can read. The
drive loop now appends `{cli, cli_session_id?}` to an additive `Session` list, through the
`session::store` lock from `session-state-locking`, at spawn time.

`status --json` gains `orchestrator_usage`, which is `null` when there are no invocations.
`usage_total` keeps its v0.19 meaning (workers only). The dashboard's "session total" adds the
two, which matches the drive loop's cap scope.

Alternatives considered:
- Derive the invocations from the broker log's outcome statuses: rejected. They are lost when
  the host restarts, and they carry no session id.
- A separate ledger file: rejected. It is one more file to lock and reconcile.

### D6 — Pure view model (functional core)

`fn view_model(model: &Model) -> DashboardView` computes everything the dashboard shows:
- the usage rows, sorted by agent id;
- the tier subtotals (tier = the worker's CLI id, the `[clis.*]` entry it was launched with; the
  CLI column is the provider family from `cli-provider-record`);
- the orchestrator row;
- the totals with the unknown count;
- the cap progress (`used / cap`, percent, exceeded at ≥ 100%; the cost cap is "not evaluable"
  when no source reports cost);
- the visible log rows (filter, then per-agent collapse);
- the status line and the focus and selection markers.

The current time comes in through `Msg::Tick { now }`, so the function reads no clock. `render`
only lays out a `DashboardView`. The aggregation (subtotals, totals, cap progress) is a pure
`usage` module shared with `status --json` and the drive-loop summary, so the three cannot
disagree.

### D7 — Focus and scrolling, carried from `dashboard-tabbed-roster` D1

`Focus { Usage, Log }`, starting on `Log`, so every existing log key is unchanged. `Tab` toggles
focus. The usage worker rows scroll through a `TableState`, with the orchestrator and total rows
pinned below them. Hiding the log with `l` moves focus to the usage panel.

The draft's opt-in tabbed layout is dropped: with no roster, `l` already gives the usage panel
the full height.

### D8 — Per-agent heartbeat collapse, carried from `dashboard-tabbed-roster` D3

An agent's identical heartbeats collapse even when other agents' messages come in between. The
merged row sits at its latest heartbeat. This is the same rule as v0.22
`replay-semantic-timeline` ("no other message from that agent in between"), so the shared
heartbeat key it moves to `src/broker/summary.rs` serves both.

### D9 — `git paw status` shows the roster

When the broker answers, `status` joins `/status` with the live pane map. The join already
exists as `coordination::inventory::build_inventory`, routed through the terminal port after
v0.18. Each worktree line gets ` — <phase or status>, <age>, pane <n>`, and `--json` gets an
`agent` object. The broker request has the client's bounded timeout. When the broker does not
answer, the output is exactly what it was before.

### D10 — Layout share unchanged

When the dashboard is on it keeps the 50% top row (`supervisor-pane-removal`,
`configurable-agent-grid`). When it is off, the agent rows use `100 / agent_rows`. That is the
no-top-row geometry `pane-less-session-parity` D2 already defines for broker-less sessions, now
keyed on "has a dashboard".

## Risks / Trade-offs

- [An operator upgrades and expects the old pane on a new repo] → version-1 configs keep it,
  `init`-generated version-2 configs show `# enabled = true` with a comment, and the changelog
  and the dashboard chapter lead with the change.
- [Usage is unknown for every worker until v0.19's source is verified] → rows say `unknown`
  with the reason, and totals are labelled "known usage". The event log keeps the view useful.
- [Large transcripts make the 10 s read expensive] → the read runs on its own thread at a fixed
  cadence. An incremental parser is a v0.19 implementation detail behind the facet.
- [The roster removal breaks dashboard tests] → they move to `git paw status` tests and to the
  view-model tests. The removed public functions take their tests with them.
- [Human `git paw status` output changes] → the change is additive, appears only with a live
  broker, and is not a contract. The "unchanged human output" clauses of `worker-usage-budget` and
  `uniform-json-output` cover only their own fields.
- [Many MODIFIED requirements written on unarchived text] → each one names its source change in
  the proposal, and task 0.1 checks the headers before apply.

## Migration Plan

1. Seam: introduce the `DashboardView` / `view_model` split with today's roster content. This
   step changes no behaviour.
2. Session rule ("has a dashboard", offsets) and recorded invocations. Old session files load
   unchanged.
3. Config `enabled`, the v1 implication, the stamp/rewrite paths, and `--dashboard`. Launch,
   recovery and restart follow the rule.
4. Content swap: the usage panel replaces the roster, focus, per-agent collapse, `status`
   roster.
5. Docs.

Rollback: revert. The new session and config fields are additive and ignored by older binaries,
which would then show a dashboard again for broker sessions.

## Open Questions

- **A public way to open the view on demand** (`git paw dashboard`, running the existing thin
  client in any terminal). Recommendation: not now, because it adds to the CLI surface just
  before the freeze. Revisit after dogfooding.
- **`--no-dashboard`** for a single launch against a config that enables it. Recommendation:
  not now. The config key covers it.
- **Dashboard row share** when enabled (50% kept). Recommendation: keep it until dogfooding
  shows otherwise.
- **The v0.19 usage source is still "to verify"** (Claude transcript location and shape). This
  change consumes the facet and does not depend on the answer.
