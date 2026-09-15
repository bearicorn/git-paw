## ADDED Requirements

### Requirement: Worktree entry records the worker CLI session id

The `WorktreeEntry` struct SHALL include an optional field `cli_session_id: Option<String>` recording
the session identifier git-paw assigned to the worker's CLI at launch. The field SHALL be annotated
`#[serde(default, skip_serializing_if = "Option::is_none")]` so session files written before this
change load with `None` and an entry without an id serializes byte-identically to before.

#### Scenario: Assigned id round-trips

- **GIVEN** a `WorktreeEntry` with `cli_session_id = Some("3f2c…")` saved to disk
- **WHEN** the session is loaded back
- **THEN** `cli_session_id` SHALL be `Some("3f2c…")`

#### Scenario: Older session files load with None

- **GIVEN** a session JSON file with no `cli_session_id` field on its worktree entries
- **WHEN** the session is loaded
- **THEN** every entry's `cli_session_id` SHALL be `None`
- **AND** loading SHALL succeed without error

#### Scenario: Entry without an id serializes as before

- **GIVEN** a `WorktreeEntry` with `cli_session_id = None`
- **WHEN** the session is saved and the JSON inspected
- **THEN** the JSON SHALL NOT contain a `cli_session_id` key

### Requirement: Recovery relaunches a worker with its recorded session id

When `git paw start` recovers a stopped session, the system SHALL relaunch each worktree whose entry
carries a `cli_session_id` AND whose CLI resolves a resume template with that template's arguments
appended to the CLI launch command, so the worker's conversation context is restored. A worktree
whose entry has no id, or whose CLI resolves no resume template, SHALL be relaunched exactly as before
this change. Recovery SHALL NOT fail because a resume template is absent.

#### Scenario: Recovered Claude worker is resumed

- **GIVEN** a stopped session whose worktree entry for `feat/x` has `cli = "claude"` and
  `cli_session_id = Some("<uuid>")`, and no `[clis.claude]` override
- **WHEN** the session is recovered
- **THEN** the pane command for `feat/x` SHALL contain `--resume <uuid>`

#### Scenario: Entry without an id launches fresh

- **GIVEN** a stopped session whose worktree entry has `cli_session_id = None`
- **WHEN** the session is recovered
- **THEN** the pane command SHALL be identical to the pre-change recovery command for that entry

#### Scenario: CLI without a resume template launches fresh

- **GIVEN** a worktree entry with `cli = "my-agent"`, `cli_session_id = Some("<id>")`, and no
  `[clis.my-agent].resume_args`
- **WHEN** the session is recovered
- **THEN** the pane command SHALL contain no resume arguments
- **AND** recovery SHALL succeed

### Requirement: Status JSON exposes the worker CLI session id

`git paw status --json` SHALL include each agent's `cli_session_id` (a string, or `null` when
unassigned) so an operator or external orchestrator can resume a worker by hand. The human-readable
`git paw status` output SHALL be unchanged.

#### Scenario: JSON status carries the id

- **GIVEN** an active session whose `feat/x` entry has `cli_session_id = Some("<uuid>")`
- **WHEN** `git paw status --json` is run
- **THEN** the `feat/x` agent object SHALL contain `"cli_session_id": "<uuid>"`

#### Scenario: JSON status shows null when unassigned

- **GIVEN** an agent whose entry has no session id
- **WHEN** `git paw status --json` is run
- **THEN** that agent object SHALL contain `"cli_session_id": null`

#### Scenario: Human-readable status is unchanged

- **WHEN** `git paw status` is run without `--json`
- **THEN** the output SHALL be byte-identical to the pre-change output for the same session
