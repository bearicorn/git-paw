# supervisor-introspection Specification

## Purpose
Adds optional `phase` and `detail` fields to the `agent.status` broker message and a documented supervisor phase taxonomy, so the supervisor emits structured progress that the dashboard and the MCP `get_session_status` tool surface on the supervisor row.

## Requirements
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

