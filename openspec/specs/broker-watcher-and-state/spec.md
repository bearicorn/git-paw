# broker-watcher-and-state Specification

## Purpose

This capability keeps the broker's `/status` roster and per-agent state honest as worktrees change. It covers the broker-internal filesystem watcher that polls each mapped worktree's `git status --porcelain` and auto-publishes `agent.status`; roster hygiene (a row appears only once a pane actually publishes `agent.status`, and its CLI column is filled authoritatively from the launcher-known value rather than a self-reported guess); republishing an agent's status from `committed` back to `working` on post-commit file writes within a configurable TTL; and terminal-status stickiness in `update_agent_record` so a completed agent cannot be downgraded by later inferred activity.

It also folds in the supervisor-introspection surface — optional `phase` and `detail` fields on `agent.status`, the documented supervisor phase taxonomy, and how the dashboard supervisor row and the MCP `get_session_status` tool surface them — and the `agent.advanced-main` broker event the supervisor publishes after every successful merge to main, together with the supervisor and coding-agent skill guidance (arrival on the normal poll, no auto-rebase) and its automatic dashboard broker-log rendering.

## Requirements

### Requirement: Roster populated only from agent.status publishers

The broker SHALL populate the `/status` agent roster only from
agents that publish `agent.status`. A roster row SHALL appear
only once a pane has actually published — a pane whose CLI is
known/seeded but which has not yet published SHALL NOT show a
row (no phantom row for an unstarted or aborted pane, supervisor
included). The system SHALL NOT create or update a roster entry
from the `from` or `target` identity fields of
`agent.feedback`, `agent.question`, or `agent.verified`
messages. Those messages SHALL still be routed and stored.

#### Scenario: Feedback from a non-agent identity creates no roster row

- **GIVEN** a running broker with N agent.status publishers
  registered
- **WHEN** an `agent.feedback` is published with
  `payload.from = "human"` (and `agent_id` of an existing
  publisher or the supervisor)
- **THEN** the roster SHALL still contain exactly the N status
  publishers (plus supervisor) — no `"human"` row is created

#### Scenario: Question/verified identities create no roster rows

- **WHEN** `agent.question` or `agent.verified` messages carry
  `from`/`target`/`verified_by` identities
- **THEN** none of those identities SHALL appear as roster rows
  unless they independently publish `agent.status`

#### Scenario: Feedback is still delivered

- **WHEN** an `agent.feedback` is published targeting an agent
- **THEN** the message SHALL still be routed/stored and
  retrievable by that agent's poll (delivery unaffected by the
  roster gating)

#### Scenario: Seeded-but-unpublished pane shows no row

- **GIVEN** the broker has a known/seeded CLI for a pane (from a
  `WatchTarget` or the supervisor seed) that has not yet
  published any `agent.status`
- **THEN** the `/status` roster SHALL contain no row for that
  pane — the row appears only once the pane publishes, so an
  unstarted or aborted launch leaves no phantom row

### Requirement: Agent CLI populated in the roster

The CLI map SHALL be pre-filled authoritatively at launch from
the value git-paw used to start each pane — coding agents from
their `WatchTarget` (the per-repo session JSON,
`.git-paw/sessions/paw-<project>.json`) at broker start, and the
supervisor (which is not a filesystem watch target) from the
launcher-resolved `[supervisor].cli` falling back to
`default_cli` via the broker-state seed. When a pane publishes
and its roster row appears, that row's CLI column SHALL render
the pre-filled value. The bundled skills SHALL NOT require
agents to self-report their CLI — they would only be guessing.

#### Scenario: Published agent's row carries the seeded CLI

- **GIVEN** a `cli = "claude-oss"` session (the CLI map seeded
  from watch targets at broker start)
- **WHEN** a coding agent publishes an `agent.status` (with no
  `cli` field of its own)
- **THEN** its roster row SHALL show `cli = "claude-oss"`,
  resolved from the authoritative seed, not blank and not a
  self-reported value

#### Scenario: Supervisor CLI seeded authoritatively from config

- **GIVEN** a `[supervisor].cli = "claude-oss"` (or `default_cli`)
  session, the CLI map seeded for `supervisor` at broker start
- **WHEN** the supervisor publishes its bootstrap `agent.status`
  (which carries no `cli` field)
- **THEN** the roster's `supervisor` row SHALL show
  `cli = "claude-oss"` from the seed — without the supervisor
  self-reporting its CLI

#### Scenario: Authoritative seed wins over a wrong self-report

- **GIVEN** the broker has seeded the supervisor's CLI as
  `claude-oss` from config
- **WHEN** the supervisor self-reports a different CLI in its
  `agent.status` (e.g. `cli = "claude"`, a wrong guess from the
  bootstrap placeholder)
- **THEN** the roster SHALL keep the seeded `claude-oss` — a
  self-reported CLI fills the map only when no authoritative
  value was seeded, so a guess never clobbers the launcher-known
  value

#### Scenario: Broker resolves CLI from session JSON when status omits it

- **GIVEN** an agent whose `agent.status` payload has no `cli`
  field, but the per-repo session JSON lists its `cli`
- **WHEN** the roster entry is rendered
- **THEN** the broker SHALL resolve the CLI from the session
  JSON so the row is not blank

#### Scenario: Dashboard CLI column populated for all agents

- **GIVEN** a session with multiple coding agents
- **WHEN** the dashboard renders the agent table
- **THEN** every agent row's CLI column SHALL be populated
  (not just the supervisor row)

#### Scenario: Unknown CLI shows a placeholder, not blank

- **GIVEN** an agent whose CLI cannot be resolved from status
  or session JSON
- **THEN** the CLI column SHALL show a documented
  "unknown" placeholder rather than an empty string

### Requirement: Phantom rows do not survive a broker restart

The broker roster is in-memory; the system SHALL NOT persist
phantom rows. A broker restart SHALL produce a roster built
solely from fresh `agent.status` publishers.

#### Scenario: Restart clears any pre-existing phantom

- **GIVEN** a broker that (under old behaviour) had a phantom
  row
- **WHEN** the broker restarts and agents re-register via
  `agent.status`
- **THEN** the new roster SHALL contain only the real
  status-publishing agents

### Requirement: Watch worktree git state for changes

The broker process SHALL poll each worktree at a fixed interval using `git status --porcelain` and auto-publish `agent.status` when the set of reported paths differs from the previous tick. The `modified_files` field SHALL contain the paths currently reported by git status.

The poll interval SHALL be 2 seconds. The watcher SHALL NOT publish when the snapshot is unchanged from the previous tick.

#### Scenario: File edit triggers status publish

- **GIVEN** a running broker watching worktree `/path/to/git-paw-feat-x`
- **WHEN** a file at `src/lib.rs` is modified in that worktree
- **THEN** within 3 seconds the broker publishes `agent.status` for `feat-x` with `modified_files` containing `src/lib.rs`

#### Scenario: Multiple rapid edits are collapsed into one publish

- **GIVEN** a running broker watching a worktree
- **WHEN** 5 files are modified within a single poll interval
- **THEN** a single `agent.status` is published with all 5 files in `modified_files`

#### Scenario: Build artifacts are excluded via gitignore

- **GIVEN** a running broker watching a worktree whose `.gitignore` lists `target/` and `node_modules/`
- **WHEN** files change in `target/` or `node_modules/`
- **THEN** no `agent.status` is published for those changes

#### Scenario: Unchanged state does not re-publish

- **GIVEN** a running broker that has just published an `agent.status` for a worktree
- **WHEN** the next poll tick runs and `git status --porcelain` output is byte-identical to the previous tick
- **THEN** no new `agent.status` is published

#### Scenario: Watcher stops when broker stops

- **GIVEN** a running broker with active watchers
- **WHEN** the `BrokerHandle` is dropped
- **THEN** all watcher tasks stop within one poll interval

### Requirement: Worktree-to-agent mapping

The broker SHALL accept a list of `WatchTarget { agent_id, worktree_path }` at startup. Each watcher task SHALL publish status for the `agent_id` of its assigned `WatchTarget`.

#### Scenario: Events map to correct agent

- **GIVEN** two watch targets: `feat-a` at `/wt-a/` and `feat-b` at `/wt-b/`
- **WHEN** a file changes in `/wt-a/src/lib.rs`
- **THEN** the status is published for agent `feat-a`, not `feat-b`

### Requirement: Watcher republishes working on post-commit file writes

The filesystem watcher SHALL transition an agent's state from
`committed` back to `working` when it observes a file
modification inside the agent's worktree within a configurable
TTL window after the `agent.artifact status: "committed"`
event. The default TTL SHALL be 60 seconds.

#### Scenario: File write within TTL republishes working

- **GIVEN** an agent that just published `agent.artifact
  status: "committed"` 10 seconds ago
- **WHEN** the watcher observes a file modification inside
  the agent's worktree
- **THEN** the watcher SHALL publish `agent.status:
  working` so dashboard + MCP consumers reflect the agent's
  continued activity

#### Scenario: File write after TTL does NOT republish

- **GIVEN** the same agent 5 minutes after its `committed`
  event
- **WHEN** the watcher observes a file modification
- **THEN** the watcher SHALL NOT auto-republish `working`
  (the agent is considered settled; only an explicit
  `agent.status` publish from the agent itself transitions
  out of `committed`)

#### Scenario: Multiple writes within TTL republish only once

- **GIVEN** an agent within its post-commit TTL window
- **WHEN** the watcher observes a burst of file
  modifications (e.g. ten files in two seconds)
- **THEN** the watcher SHALL publish `agent.status:
  working` exactly once for that burst (rate-limited),
  preserving v0.5.0's watcher rate-limit semantics

### Requirement: TTL configurable via broker.watcher config

The system SHALL accept
`[broker.watcher].republish_working_ttl_seconds` as a numeric
config field defaulting to `60`. Values less than 5 SHALL be
clamped to 5 (matching the v0.5.0 auto-approve threshold
floor pattern) with a stderr warning. Values 0 SHALL be
treated as "disable the auto-republish behaviour."

#### Scenario: Default TTL is 60 seconds

- **GIVEN** no `[broker.watcher]` section in config
- **WHEN** the watcher initialises
- **THEN** the configured TTL SHALL resolve to 60 seconds

#### Scenario: TTL of 0 disables auto-republish

- **GIVEN** `[broker.watcher].republish_working_ttl_seconds
  = 0`
- **WHEN** a post-commit write fires
- **THEN** the watcher SHALL NOT publish a synthetic
  `working` status; v0.5.0 behaviour is preserved

### Requirement: Dashboard accepts committed → working transition

The dashboard state machine SHALL accept `working` as a
valid transition out of `committed` for the supervisor row
and all agent rows. The dashboard SHALL re-render the
agent's row accordingly when the transition fires.

#### Scenario: Dashboard re-renders on the transition

- **GIVEN** an agent row currently displaying `committed`
- **WHEN** a new `agent.status: working` message arrives
  for that agent
- **THEN** the next dashboard frame SHALL show the row as
  `working` (the previous v0.5.0 behaviour of locking on
  `committed` SHALL NOT apply)

### Requirement: Behavioural opt-out preserves v0.5.0 model

The system SHALL provide a behavioural opt-out that restores
v0.5.0's "committed is terminal until explicit republish"
semantics exactly. Setting
`[broker.watcher].republish_working_ttl_seconds = 0` SHALL
disable the auto-republish behaviour entirely.

#### Scenario: Opt-out produces v0.5.0 behaviour

- **GIVEN** the TTL configured to 0
- **WHEN** an agent commits and then continues editing
- **THEN** the watcher SHALL NOT republish `working`; the
  dashboard SHALL display `committed` until the agent
  itself publishes a new `agent.status` (matching v0.5.0
  byte-for-byte)

### Requirement: Terminal status helper

The system SHALL expose an `is_terminal_status` helper that returns `true` for the four terminal status strings (`"done"`, `"verified"`, `"blocked"`, `"committed"`) and `false` for any other status string.

#### Scenario: Helper recognizes terminal statuses

- **WHEN** `is_terminal_status` is invoked with `"done"`, `"verified"`, `"blocked"`, or `"committed"`
- **THEN** the result is `true`

#### Scenario: Helper rejects non-terminal statuses

- **WHEN** `is_terminal_status` is invoked with any string other than the four terminal statuses (e.g. `"working"`, `"idle"`, `"error"`, `""`)
- **THEN** the result is `false`

### Requirement: Terminal status is sticky in update_agent_record

When `update_agent_record` updates an agent record's status, the system SHALL preserve a terminal current status by only overwriting it when the incoming status is also a terminal status. Specifically, the record's status SHALL be updated when (the current status is NOT terminal) OR (the new status IS terminal); otherwise the existing status SHALL remain unchanged. This protection SHALL apply uniformly regardless of which message variant (`agent.status`, `agent.artifact`, `agent.blocked`, etc.) triggered the call, and the protection SHALL silently retain the terminal status without raising an error. The watcher's automatic status updates (e.g. inferring `"working"` from filesystem activity) SHALL therefore NOT downgrade an agent that has already reached a terminal status.

#### Scenario: Terminal status is not overwritten by non-terminal status

- **GIVEN** an agent record with `status = "done"`
- **WHEN** `update_agent_record` is invoked with a new status of `"working"`
- **THEN** the stored status remains `"done"`
- **AND** no error is returned

#### Scenario: Terminal status can be overwritten by another terminal status

- **GIVEN** an agent record with `status = "done"`
- **WHEN** `update_agent_record` is invoked with a new status of `"verified"`
- **THEN** the stored status becomes `"verified"`

#### Scenario: Non-terminal status can be overwritten by terminal status

- **GIVEN** an agent record with `status = "working"`
- **WHEN** `update_agent_record` is invoked with a new status of `"done"`
- **THEN** the stored status becomes `"done"`

#### Scenario: All terminal statuses are protected from non-terminal overwrites

- **GIVEN** agent records with each of the four terminal statuses (`"done"`, `"verified"`, `"blocked"`, `"committed"`)
- **WHEN** each record receives an update attempt with `status = "working"`
- **THEN** every record retains its original terminal status

#### Scenario: Watcher cannot downgrade a terminal status

- **GIVEN** an agent record with `status = "committed"` reached via an `agent.artifact` message
- **WHEN** the filesystem watcher subsequently calls `update_agent_record` with `status = "working"` because of file activity in the worktree
- **THEN** the stored status remains `"committed"`

#### Scenario: Artifact message with non-terminal status does not downgrade

- **GIVEN** an agent record with `status = "verified"`
- **WHEN** an `agent.artifact` message arrives carrying `status = "working"` and is routed through `update_agent_record`
- **THEN** the stored status remains `"verified"`

### Requirement: Optional phase and detail fields on agent.status

The `agent.status` broker message variant SHALL accept two
additional optional fields: `phase` (string, open enum) and
`detail` (free-form JSON object). The system SHALL omit both
fields from serialised messages when their values are
unset, preserving v0.5.0 wire compatibility. The broker SHALL
NOT validate the set of `phase` values; consumers SHALL
treat unknown values gracefully.

#### Scenario: Status without phase round-trips unchanged

- **GIVEN** a v0.5.0-shape `agent.status` message with no
  phase or detail fields set
- **WHEN** the broker accepts, stores, and re-emits the message
- **THEN** the round-tripped JSON SHALL be byte-equivalent to
  the v0.5.0 payload (no extra null fields appear)

#### Scenario: Status with phase and detail accepted

- **WHEN** an agent publishes an `agent.status` with `phase =
  "audit"` and `detail = { branch: "feat/x", audit_step:
  "tests" }`
- **THEN** the broker SHALL accept and route the message,
  preserving both fields

#### Scenario: Unknown phase value accepted

- **WHEN** an agent publishes an `agent.status` with `phase =
  "future_value_not_in_v0_6_0_taxonomy"`
- **THEN** the broker SHALL accept the message without
  validation error

### Requirement: Supervisor phase taxonomy

The bundled supervisor skill SHALL document a phase taxonomy
covering at least: `sweep`, `audit`, `merge`, `feedback`,
`intent_watch`, `learnings`, `idle`. Each phase SHALL have a
documented `detail` shape so the supervisor LLM emits
consistent structured data across sessions.

The skill SHALL deliver every phase-tagged `agent.status` — including
the boot self-register, each documented phase transition, and the
`checkpoint` emission — through the bundled `sweep.sh status-publish`
helper (`--phase <phase>` plus, when the taxonomy specifies a detail body,
`--detail '<json-object>'`), NOT through a raw `curl …/publish` call. The
skill's phase-taxonomy examples SHALL show the `sweep.sh status-publish`
form so the documented taxonomy reaches the broker by the least-privilege,
by-path helper grant rather than a broad curl allowlist.

#### Scenario: Taxonomy table documents all seven phases

- **WHEN** the bundled supervisor.md is inspected
- **THEN** the introspection section SHALL contain a table
  listing at least the seven phase values with their
  documented detail field names

#### Scenario: Audit phase detail names the five gates

- **WHEN** the audit phase's detail documentation is read
- **THEN** the detail's `audit_step` field SHALL enumerate
  the v0.5.0 five gates (tests, spec, docs, security,
  regression)

#### Scenario: Phase emission examples use the helper, not raw curl

- **WHEN** the introspection section's phase-emission examples are read
- **THEN** each `agent.status` emission example SHALL invoke
  `sweep.sh status-publish` with `--phase` (and `--detail` where the
  taxonomy specifies a detail body)
- **AND** no example SHALL emit an `agent.status` via a raw
  `curl …/publish` call

### Requirement: Supervisor emission cadence

The bundled supervisor skill SHALL teach the supervisor LLM
to emit an `agent.status` on every phase transition AND at
most once per ~30 seconds while remaining in the same phase.
The supervisor SHALL NOT emit per-micro-action status spam.
On entering `idle`, the supervisor SHALL emit one status and
stop further updates until the next active phase.

#### Scenario: Cadence rules documented in skill prose

- **WHEN** the introspection section of supervisor.md is read
- **THEN** the cadence rules SHALL appear explicitly:
  emit on phase transition, rate-limit to ~30s within the
  same phase, single-emit on entering idle

### Requirement: Dashboard surfaces supervisor phase

The dashboard agent table SHALL render the `phase` field next
to the summary on the supervisor row only. When `phase` is
absent or unrecognised, the dashboard SHALL fall back to the
v0.5.0 summary-only rendering. Non-supervisor agent rows SHALL
render exactly as in v0.5.0 regardless of whether `phase` is
present on their status.

#### Scenario: Supervisor row shows phase when present

- **GIVEN** an active session whose supervisor has published
  `phase = "audit"`
- **WHEN** the dashboard renders the agent table
- **THEN** the supervisor row SHALL include `audit` (or its
  documented label) alongside the summary

#### Scenario: Supervisor row falls back when phase absent

- **GIVEN** an active session whose supervisor has not
  published a `phase` field
- **WHEN** the dashboard renders the agent table
- **THEN** the supervisor row SHALL render as it did in
  v0.5.0 (status + summary only)

#### Scenario: Non-supervisor agent rows unchanged

- **GIVEN** a coding agent that has published an
  `agent.status` with a phase field set
- **WHEN** the dashboard renders the agent table
- **THEN** that agent's row SHALL render as it did in
  v0.5.0 — the phase field SHALL be ignored for non-supervisor
  rows

### Requirement: MCP get_session_status includes introspection

The MCP `get_session_status()` tool from [[mcp-server]] SHALL
populate `phase` and `detail` for the supervisor sub-record
from the latest supervisor `agent.status` message. The fields
SHALL be omitted (or null) when the supervisor has not
emitted them in the current session.

#### Scenario: MCP response surfaces supervisor phase

- **GIVEN** an active session whose supervisor has emitted
  `phase = "merge"` with detail
- **WHEN** an MCP client calls `get_session_status()`
- **THEN** the supervisor sub-record SHALL include
  `phase: "merge"` and the detail object

#### Scenario: MCP response degrades gracefully

- **GIVEN** an active session whose supervisor has not emitted
  any phase
- **WHEN** an MCP client calls `get_session_status()`
- **THEN** the supervisor sub-record SHALL have `phase` and
  `detail` either absent or null, with no error

### Requirement: checkpoint phase shared with stream-timeout-recovery

The system SHALL reuse the `phase` field for the checkpoint
emission defined by [[supervisor-stream-timeout-recovery]].
That emission SHALL use `phase = "checkpoint"` with detail
fields documented by that change. The introspection skill
prose SHALL acknowledge `checkpoint` as a valid phase value.

#### Scenario: Checkpoint emission uses phase = checkpoint

- **WHEN** the supervisor performs a stream-timeout-recovery
  pre-action checkpoint per [[supervisor-stream-timeout-recovery]]
- **THEN** the emitted `agent.status` SHALL set `phase =
  "checkpoint"` and SHALL include the checkpoint's documented
  detail fields

### Requirement: Stack-agnostic phrasing

The new supervisor-skill section SHALL pass the no-language-
leak audit from [[lang-agnostic-assets]]. The section SHALL
NOT use Rust-specific or any other stack-specific language in
its prose or examples.

#### Scenario: No-leak audit passes after the section lands

- **WHEN** the no-leak audit runs against the updated
  supervisor.md
- **THEN** the audit SHALL pass on the rendered skill across
  all supported spec backends


### Requirement: agent.advanced-main broker variant

The broker SHALL accept and route an `agent.advanced-main`
message variant. Each message SHALL carry the fields `from`,
`merged_branch`, `new_main_sha`, `base`, `merged_at`, and an
optional `summary`. The broker SHALL NOT validate the SHA's
existence or shape beyond its presence as a string.

#### Scenario: Broker accepts a well-formed advanced-main message

- **WHEN** the supervisor publishes an `agent.advanced-main`
  message with all required fields populated
- **THEN** the broker SHALL accept the message and SHALL
  include it in subsequent `/messages/<branch_id>` poll
  responses for every registered agent

#### Scenario: Missing required field is rejected

- **WHEN** a publish omits `merged_branch`, `new_main_sha`,
  `base`, or `merged_at`
- **THEN** the broker SHALL return a 400-class error
  identifying the missing field

#### Scenario: Optional summary is preserved when present

- **WHEN** the publisher includes a `summary` value
- **THEN** the routed message SHALL preserve the summary
  verbatim

### Requirement: Deterministic id for advanced-main events

The system SHALL produce a deterministic `id` for each
`agent.advanced-main` record using the same hashing pattern as
`agent.learning` from [[agent-learning-variant]]. The canonical
input SHALL include `merged_branch`, `new_main_sha`, `base`,
and the UTC hour bucket. Re-publishing the same merge within
the same hour SHALL produce an identical id.

#### Scenario: Same merge within the hour produces identical ids

- **WHEN** the supervisor publishes the same merge twice
  within a UTC hour
- **THEN** both broker messages SHALL carry identical `id`
  values

### Requirement: Supervisor publishes on merge to main

The bundled supervisor skill SHALL teach the LLM to publish an
`agent.advanced-main` event after every successful `git merge`
targeting `main` (or the configured `[git] main_branch`). The
skill SHALL include a concrete curl invocation example with
the message shape from this capability.

#### Scenario: Skill prose names the publish trigger explicitly

- **WHEN** the merge-orchestration section of supervisor.md is
  read
- **THEN** the prose SHALL include a publish step that fires
  immediately after a successful merge to main, with a
  concrete curl-to-`/publish` example

#### Scenario: Publish includes the resolved base name

- **WHEN** the skill prose shows the publish step
- **THEN** the `base` field SHALL be documented as the
  resolved `[git] main_branch` value, not hardcoded `"main"`

### Requirement: Agent skill teaches polling discipline

The bundled coordination skill SHALL include a "When main
advances" subsection teaching coding agents:
1. The event arrives on their normal
   `/messages/<branch_id>` poll
2. They SHALL NOT auto-rebase on receipt
3. The recommended decision process is fetch + inspect +
   decide
4. Any rebase action SHALL be preceded by a commit or stash
   to prevent loss

#### Scenario: Skill includes the four-step discipline

- **WHEN** the "When main advances" subsection is read
- **THEN** the prose SHALL contain the four items: polling
  source, no-auto-rebase rule, fetch+inspect+decide flow,
  and the commit-or-stash-first safety rule

#### Scenario: Skill explicitly forbids auto-rebase

- **WHEN** the polling-discipline subsection is read
- **THEN** the prose SHALL contain explicit "do not auto-
  rebase" language with a one-sentence safety rationale

### Requirement: Variant flows through dashboard automatically

The dashboard's [[dashboard-broker-log]] panel SHALL render
`agent.advanced-main` events without any code change to the
log panel — the existing watcher feed delivers the variant to
the ring buffer like any other message type. The filter-chip
bitmask SHALL gain a bit position for the new variant so
users can isolate the event stream.

#### Scenario: Advance event appears in the broker log

- **GIVEN** the dashboard's broker log panel is visible
- **WHEN** the supervisor publishes an `agent.advanced-main`
- **THEN** the event SHALL appear at the top of the panel
  within one frame tick, with the new variant's filter chip
  available in the header

### Requirement: Cross-reference with supervisor introspection

The publish trigger SHALL coordinate with
[[supervisor-introspection]] such that the `phase = "merge"`
status emitted before the merge and the `agent.advanced-main`
event emitted after a successful merge SHARE the
`merged_branch` value. This lets consumers correlate the two
events.

#### Scenario: Phase merge status and advance event share merged_branch

- **WHEN** the supervisor completes a successful merge of
  branch `feat/x`
- **THEN** the supervisor's preceding `phase = "merge"`
  status SHALL have `detail.branch == "feat/x"` and the
  resulting `agent.advanced-main` event SHALL have
  `merged_branch == "feat/x"`

### Requirement: Stack-agnostic phrasing

The new supervisor and coordination skill content SHALL pass
the no-language-leak audit from [[lang-agnostic-assets]]. The
content SHALL NOT use Rust-specific or any other stack-
specific language in its prose or examples.

#### Scenario: No-leak audit passes after the prose lands

- **WHEN** the no-leak audit runs against the updated
  supervisor.md and coordination.md
- **THEN** the audit SHALL pass on the rendered skills across
  all supported spec backends

