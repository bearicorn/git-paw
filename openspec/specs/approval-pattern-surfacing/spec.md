# approval-pattern-surfacing Specification

## Purpose
Records every command the auto-approve loop forwards for a manual human decision to a per-session JSONL log, and provides the `git paw approvals` subcommand to aggregate those patterns by frequency with a promotion-target hint (project allowlist vs bundled preset candidate). First-seen patterns can also emit a `permission_pattern` learning record, surfacing which prompts recur so they can be promoted into the allowlist. The whole channel is opt-out via config.
## Requirements
### Requirement: Manual-decision log file

The system SHALL append one JSON object per line to
`.git-paw/sessions/<session>.manual-approvals.jsonl` for each
live permission prompt the `--unattended` drive loop escalates
for a human decision. Each entry SHALL include `timestamp`
(ISO 8601 UTC), `agent_id`, `pattern` (the prompted command),
and `first_seen` (boolean).

git-paw cannot observe the in-pane answer to a prompt (it runs
outside the agent's CLI process), so it records the prompts it
escalates for a manual decision — the honest, observable
signal. The recorded semantic is "a command that required a
manual decision", a superset of approvals.

The drive loop SHALL record one entry per escalation it emits.
A prompt suppressed by the loop's escalation dedup window SHALL
NOT be recorded again. An escalation that is not a permission
prompt (a CLI-unavailability banner, a wedged nudge, or a
correction-loop signal) SHALL NOT be recorded. The
`[supervisor].manual_approvals_log` opt-out and the first-seen
learnings dispatch SHALL apply to these records. Attended
sessions run no drive loop and therefore record no entries.

#### Scenario: A drive-loop escalation appends a JSONL line

- **GIVEN** an `--unattended` session whose drive loop finds a
  live permission prompt on an agent pane
- **WHEN** the loop escalates it
- **THEN** the system SHALL append one JSON line to the
  session's manual-decision log with the required fields
  populated

#### Scenario: A deduplicated drive-loop escalation is logged once

- **GIVEN** a drive loop that sees the same unresolved prompt
  on the same agent in consecutive sweeps within its dedup
  window
- **WHEN** both sweeps run
- **THEN** the system SHALL append exactly one manual-decision
  log entry for it

#### Scenario: Non-prompt escalations are not logged

- **GIVEN** a drive loop that records a `cli-unavailable`
  escalation for an agent whose pane shows a usage-limit banner
- **WHEN** the escalation is recorded
- **THEN** the system SHALL NOT append a manual-decision log
  entry for it

#### Scenario: first_seen is true exactly once per pattern per session

- **GIVEN** a session in which the same pattern is recorded
  twice
- **WHEN** both records are logged
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
promotion target in the agent CLI's own permission rules.
Patterns matching project-specific heuristics (paths starting
with `./`, paths under the worktree root, contains the project
or branch name) SHALL be suggested for the `project allowlist`,
the CLI's project-scoped rules. Other patterns SHALL be
suggested for the `user allowlist`, the CLI's user-scoped
rules. The suggestion SHALL be presented as a hint, not a rule:
acting on it is the user's choice, made in that CLI's own
settings.

#### Scenario: Project-specific path suggested for project allowlist

- **GIVEN** the pattern `./scripts/deploy-staging.sh`
- **WHEN** the aggregator classifies it
- **THEN** the suggested target SHALL be `project allowlist`

#### Scenario: Generic command suggested for the user allowlist

- **GIVEN** the pattern `make integration-test`
- **WHEN** the aggregator classifies it
- **THEN** the suggested target SHALL be `user allowlist`

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
learnings records derived from manual decisions.

#### Scenario: The opt-out applies to the drive loop

- **GIVEN** `[supervisor].manual_approvals_log = false` and an
  `--unattended` session
- **WHEN** the drive loop escalates a prompt
- **THEN** the system SHALL NOT append to the JSONL log AND
  SHALL NOT emit any `permission_pattern` learning record

#### Scenario: Subcommand still reads existing logs after opt-out

- **GIVEN** an existing log file written under a previous
  setting AND the opt-out now in effect
- **WHEN** the user runs `git paw approvals`
- **THEN** the subcommand SHALL still aggregate from the
  existing file (the opt-out affects writes, not reads)

### Requirement: Permission-pattern learning is extracted from the prompt slice

The `permission_pattern` learning SHALL derive its command pattern from the same
command slice the drive loop's escalation reports for the pending prompt, NOT from
surrounding pane text (an accept-edits diff line, a subsequent command's output). When
the slice does not yield a value that parses as a command, the system SHALL produce no
`permission_pattern` learning for that observation rather than record a spurious one.

#### Scenario: The pattern comes from the pending command, not adjacent text

- **GIVEN** a pane whose prompt slice holds the pending command and whose surrounding lines hold an accept-edits diff line and a later command's output
- **WHEN** a `permission_pattern` learning is built
- **THEN** its pattern SHALL be derived from the pending command in the prompt slice
- **AND** SHALL NOT be derived from the adjacent diff or output text

#### Scenario: A non-command slice yields no learning

- **GIVEN** a prompt slice whose extracted text does not parse as a command
- **WHEN** the learning extractor runs
- **THEN** it SHALL produce no `permission_pattern` learning for that observation

