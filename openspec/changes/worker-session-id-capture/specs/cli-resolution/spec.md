## ADDED Requirements

### Requirement: Per-CLI session-id and resume argument templates

The `[clis.<name>]` table SHALL accept two optional string templates, `session_id_args` (arguments
that assign a session identifier at launch) and `resume_args` (arguments that resume an existing
session), each containing the placeholder `{{SESSION_ID}}`. Resolution for a CLI SHALL consult, in
order: (1) the `[clis.<name>]` override when present; (2) the built-in table; (3) no template. The
built-in table SHALL contain, at minimum, `claude` → `session_id_args = "--session-id {{SESSION_ID}}"`
and `resume_args = "--resume {{SESSION_ID}}"`; every CLI without a built-in row and without an override
SHALL resolve to no template. A template that omits `{{SESSION_ID}}` SHALL be rejected at config load
with an error naming the key. The built-in rows SHALL be verified against the CLI's upstream
documentation at implementation time.

#### Scenario: Claude resolves the built-in templates

- **WHEN** templates are resolved for `claude` with no config override
- **THEN** `session_id_args` SHALL render to `--session-id <id>` and `resume_args` to `--resume <id>`
  for the given id

#### Scenario: Override wins over the built-in row

- **GIVEN** `[clis.claude]` defining `resume_args = "--continue-session {{SESSION_ID}}"`
- **WHEN** the resume template is resolved for `claude`
- **THEN** it SHALL render to `--continue-session <id>`

#### Scenario: Unknown CLI resolves no template

- **WHEN** templates are resolved for `my-agent` with no `[clis.my-agent]` templates
- **THEN** both `session_id_args` and `resume_args` SHALL resolve to none

#### Scenario: Template without the placeholder is rejected

- **GIVEN** `[clis.my-agent]` defining `session_id_args = "--session"` (no `{{SESSION_ID}}`)
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `session_id_args`

### Requirement: Worker session id is assigned at launch

The system SHALL assign each worker a session identifier at launch: when the launch flow attaches a
worker (initial `start`, spec-driven `start`, `git paw add`) and the worker's CLI resolves a
`session_id_args` template, it SHALL generate a fresh version-4-formatted UUID from OS-provided
randomness, append the rendered `session_id_args` to the CLI launch command after any approval flags,
and record the id in the worktree's `cli_session_id`. When the CLI resolves no
`session_id_args` template, the launch command SHALL be unchanged and `cli_session_id` SHALL remain
`None`. The identifier SHALL be generated without adding a dependency.

#### Scenario: Claude worker is launched with an assigned id

- **GIVEN** a worker on `feat/x` with `cli = "claude"` and no override
- **WHEN** the launch command is built
- **THEN** it SHALL contain `--session-id <uuid>` where `<uuid>` matches the v4 UUID shape
- **AND** the saved `WorktreeEntry` for `feat/x` SHALL have `cli_session_id = Some(<uuid>)`

#### Scenario: Each worker gets a distinct id

- **GIVEN** three Claude workers launched in one session
- **WHEN** their `cli_session_id` values are compared
- **THEN** all three SHALL differ

#### Scenario: CLI without an assignment template launches unchanged

- **GIVEN** a worker with `cli = "codex"` and no `[clis.codex].session_id_args`
- **WHEN** the launch command is built
- **THEN** it SHALL be identical to the pre-change command
- **AND** the saved entry's `cli_session_id` SHALL be `None`

#### Scenario: Assignment follows approval flags

- **GIVEN** a Claude worker at `agent_approval = "full-auto"`
- **WHEN** the launch command is built
- **THEN** `--dangerously-skip-permissions` SHALL precede `--session-id <uuid>` in the command
