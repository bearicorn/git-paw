## MODIFIED Requirements

### Requirement: Per-CLI session-id and resume argument templates

The `[clis.<name>]` table SHALL accept two optional string templates, `session_id_args` (arguments
that assign a session identifier at launch) and `resume_args` (arguments that resume an existing
session), each containing the placeholder `{{SESSION_ID}}`. Resolution for a CLI SHALL consult, in
order: (1) the `[clis.<name>]` override when present, keyed by the CLI id; (2) the built-in row of
the CLI's provider family (per `cli-provider`); (3) no template. The `claude` family's built-in row
SHALL contain, at minimum, `session_id_args = "--session-id {{SESSION_ID}}"` and
`resume_args = "--resume {{SESSION_ID}}"`; every family without a built-in row, and every CLI that
resolves the `generic` family without an override, SHALL resolve to no template. A template that
omits `{{SESSION_ID}}` SHALL be rejected at config load with an error naming the key. The built-in
rows SHALL be verified against the CLI's upstream documentation at implementation time.

#### Scenario: Claude resolves the built-in templates

- **WHEN** templates are resolved for `claude` with no config override
- **THEN** `session_id_args` SHALL render to `--session-id <id>` and `resume_args` to `--resume <id>`
  for the given id

#### Scenario: A declared Claude variant resolves the family's templates

- **GIVEN** `[clis.claude-oss]` with `family = "claude"` and no template overrides
- **WHEN** templates are resolved for `claude-oss`
- **THEN** `session_id_args` SHALL render to `--session-id <id>` and `resume_args` to `--resume <id>`

#### Scenario: Override wins over the built-in row

- **GIVEN** `[clis.claude]` defining `resume_args = "--continue-session {{SESSION_ID}}"`
- **WHEN** the resume template is resolved for `claude`
- **THEN** it SHALL render to `--continue-session <id>`

#### Scenario: Unknown CLI resolves no template

- **WHEN** templates are resolved for `my-agent` with no `[clis.my-agent]` templates and no `family`
- **THEN** both `session_id_args` and `resume_args` SHALL resolve to none

#### Scenario: Template without the placeholder is rejected

- **GIVEN** `[clis.my-agent]` defining `session_id_args = "--session"` (no `{{SESSION_ID}}`)
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `session_id_args`
