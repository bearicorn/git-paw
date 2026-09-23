## MODIFIED Requirements

### Requirement: Manual-decision log file

The system SHALL append one JSON object per line to
`.git-paw/sessions/<session>.manual-approvals.jsonl` for each
command an approver forwards to the human for a manual
decision (a prompt the bundled preset / worktree classifier did
NOT match). The approvers are the dashboard's auto-approve poll
loop (attended sessions) and the `--unattended` drive loop,
which escalates instead of forwarding; whichever is active for
the session SHALL record. Each entry SHALL include `timestamp`
(ISO 8601 UTC), `agent_id`, `pattern` (the captured command),
and `first_seen` (boolean). The system SHALL NOT log
auto-approved (preset-matched) commands.

The supervisor poll loop cannot observe the in-pane Yes/No
keystroke (it runs outside the agent's CLI process), so it
records the prompts it forwards for a manual decision — the
honest, observable signal. The recorded semantic is "a command
that required a manual decision", a superset of approvals (see
design D2).

The drive loop SHALL record one entry per escalation it emits:
a prompt suppressed by the loop's escalation dedup window SHALL
NOT be recorded again, and an escalation that is not a
permission prompt (a CLI-unavailability banner) SHALL NOT be
recorded. The `[supervisor].manual_approvals_log` opt-out and
the first-seen learnings dispatch SHALL apply to the drive loop
exactly as to the poll loop.

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

#### Scenario: A drive-loop escalation appends a JSONL line

- **GIVEN** an `--unattended` session whose drive loop finds a
  live prompt it classifies as danger or unknown
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

#### Scenario: Drive-loop auto-approvals are not logged

- **GIVEN** a drive loop that classifies a live prompt as safe
- **WHEN** it auto-approves the prompt
- **THEN** the system SHALL NOT append a manual-decision log
  entry

#### Scenario: The opt-out applies to the drive loop

- **GIVEN** `[supervisor].manual_approvals_log = false` and an
  `--unattended` session
- **WHEN** the drive loop escalates a prompt
- **THEN** the system SHALL NOT append to the JSONL log AND
  SHALL NOT emit any `permission_pattern` learning record
