# approval-pattern-surfacing Specification

## Purpose
Records every command the auto-approve loop forwards for a manual human decision to a per-session JSONL log, and provides the `git paw approvals` subcommand to aggregate those patterns by frequency with a promotion-target hint (project allowlist vs bundled preset candidate). First-seen patterns can also emit a `permission_pattern` learning record, surfacing which prompts recur so they can be promoted into the allowlist. The whole channel is opt-out via config.

## Requirements
### Requirement: Manual-decision log file

The system SHALL append one JSON object per line to
`.git-paw/sessions/<session>.manual-approvals.jsonl` for each
command the auto-approve poll loop forwards to the human for a
manual decision (a prompt the bundled preset / worktree
classifier did NOT match). Each entry SHALL include `timestamp`
(ISO 8601 UTC), `agent_id`, `pattern` (the captured command),
and `first_seen` (boolean). The system SHALL NOT log
auto-approved (preset-matched) commands.

The supervisor poll loop cannot observe the in-pane Yes/No
keystroke (it runs outside the agent's CLI process), so it
records the prompts it forwards for a manual decision — the
honest, observable signal. The recorded semantic is "a command
that required a manual decision", a superset of approvals (see
design D2).

#### Scenario: A forwarded prompt appends a JSONL line

- **GIVEN** an active session whose poll loop detects a prompt
  the preset does not match
- **WHEN** the loop forwards it to the human for a manual
  decision
- **THEN** the system SHALL append one JSON line to the
  session's manual-decision log with the required fields
  populated

#### Scenario: Auto-approved commands are not logged

- **GIVEN** a poll loop that matches a command against the
  bundled DEV_ALLOWLIST_PRESET (the `Approved` branch)
- **WHEN** the command is auto-approved
- **THEN** the system SHALL NOT append a manual-decision log
  entry

#### Scenario: first_seen is true exactly once per pattern per session

- **GIVEN** a session in which the same pattern is forwarded
  twice in succession
- **WHEN** both forwards are logged
- **THEN** the first log entry SHALL carry `first_seen: true`
  and the second SHALL carry `first_seen: false`

### Requirement: Best-effort log writes

The log writes SHALL be best-effort and SHALL NOT block the
sweep helper. The system SHALL emit a stderr warning when a
log write fails (disk full, permission denied) and SHALL
continue the sweep without panic.

#### Scenario: Disk-full failure does not crash the sweep

- **GIVEN** a filesystem at full capacity
- **WHEN** the sweep helper attempts to append a log entry
- **THEN** the write SHALL fail gracefully, a stderr warning
  SHALL be emitted, and the sweep helper SHALL continue
  operating

### Requirement: git paw approvals subcommand

The system SHALL provide a `git paw approvals` subcommand
that reads the manual-approval log, aggregates by pattern,
and reports the result. The subcommand SHALL accept
`--session <NAME>` (default: active session), `--limit <N>`
(default unlimited), and `--json` (alternative to the text
table).

#### Scenario: Subcommand lists patterns by frequency

- **GIVEN** a session whose log contains patterns approved
  with varying counts
- **WHEN** the user runs `git paw approvals`
- **THEN** the output SHALL list each pattern with its
  count, sorted descending by count

#### Scenario: --json emits structured output

- **WHEN** the user runs `git paw approvals --json`
- **THEN** the output SHALL be a JSON object with a top-level
  `session` field and an `approvals` array where each entry
  carries `pattern`, `count`, `suggested_target`,
  `first_seen`, and `last_seen`

#### Scenario: --session targets a named session

- **WHEN** the user runs `git paw approvals --session
  paw-other`
- **THEN** the subcommand SHALL read
  `.git-paw/sessions/paw-other.manual-approvals.jsonl` and
  aggregate from it, ignoring the active session's log

#### Scenario: --limit caps the output

- **WHEN** the user runs `git paw approvals --limit 5`
  against a log with more than 5 distinct patterns
- **THEN** the output SHALL contain at most 5 patterns, the
  top 5 by count

#### Scenario: No log file produces an empty result

- **GIVEN** an active session with no manual-approvals log
  file (none yet written)
- **WHEN** the user runs `git paw approvals`
- **THEN** the subcommand SHALL produce an empty result
  without error (text: "no manual approvals recorded"; JSON:
  `{ "session": "...", "approvals": [] }`)

### Requirement: Promotion-target heuristic

The aggregator SHALL classify each pattern with a suggested
promotion target. Patterns matching project-specific
heuristics (paths starting with `./`, paths under the
worktree root, contains the project or branch name) SHALL be
suggested for the `project allowlist`. Other patterns SHALL
be suggested as `bundled preset candidate`. The suggestion
SHALL be presented as a hint, not a rule.

#### Scenario: Project-specific path suggested for project allowlist

- **GIVEN** the pattern `./scripts/deploy-staging.sh`
- **WHEN** the aggregator classifies it
- **THEN** the suggested target SHALL be `project allowlist`

#### Scenario: Generic command suggested for bundled preset

- **GIVEN** the pattern `make integration-test`
- **WHEN** the aggregator classifies it
- **THEN** the suggested target SHALL be `bundled preset
  candidate`

### Requirement: Learnings dispatch on first_seen

When `[supervisor] learnings = true`, the system SHALL emit an
`agent.learning` record with
`category = "permission_pattern"` for each first-seen
manual-decision pattern. The system SHALL NOT emit additional
learning records for subsequent sightings of the same pattern
within the same session.

#### Scenario: First-seen pattern emits one learning record

- **GIVEN** `learnings = true` and a brand-new pattern
  approval
- **WHEN** the sweep records the approval
- **THEN** the system SHALL emit one `agent.learning` record
  with `category = "permission_pattern"` and a body
  containing the pattern and current count

#### Scenario: Repeat sightings do not emit duplicate learnings

- **GIVEN** the same pattern approved twice in the same
  session
- **WHEN** both approvals are recorded
- **THEN** the system SHALL emit exactly one learning record
  (from the first sighting), not two

#### Scenario: Learnings disabled means no broker emission

- **GIVEN** `learnings = false` (or no learnings config)
- **WHEN** a first-seen pattern is approved
- **THEN** the system SHALL append to the JSONL log but
  SHALL NOT emit any `agent.learning` record

### Requirement: Opt-out via config

The system SHALL accept
`[supervisor].manual_approvals_log` as a boolean config field
(default `true`). When the field is `false`, the system
SHALL NOT write to the log file AND SHALL NOT emit
learnings records derived from manual approvals.

#### Scenario: Opt-out suppresses log writes

- **GIVEN** `[supervisor].manual_approvals_log = false`
- **WHEN** the poll loop forwards a prompt for a manual
  decision
- **THEN** the system SHALL NOT append to the JSONL log AND
  SHALL NOT emit any `permission_pattern` learning record

#### Scenario: Subcommand still reads existing logs after opt-out

- **GIVEN** an existing log file written under a previous
  setting AND the opt-out now in effect
- **WHEN** the user runs `git paw approvals`
- **THEN** the subcommand SHALL still aggregate from the
  existing file (the opt-out affects writes, not reads)

