## ADDED Requirements

### Requirement: Per-CLI headless invocation template

The `[clis.<name>]` table SHALL accept an optional string `headless_args` giving the arguments that make the CLI run one non-interactive turn reading its prompt from stdin. Resolution for a CLI SHALL consult, in order: (1) the `[clis.<name>]` override; (2) the built-in table, which SHALL contain at minimum `claude` → `-p` and `codex` → `exec`; (3) none. The built-in rows SHALL be verified against each CLI's upstream documentation at implementation time. An empty override string SHALL be rejected at config load with an error naming the key.

#### Scenario: Claude and Codex resolve through the built-in table

- **WHEN** `headless_args` is resolved for `claude` and for `codex` with no overrides
- **THEN** the results SHALL be `-p` and `exec` respectively

#### Scenario: Override wins over the built-in row

- **GIVEN** `[clis.claude]` defining `headless_args = "-p --output-format text"`
- **WHEN** `headless_args` is resolved for `claude`
- **THEN** the result SHALL be `-p --output-format text`

#### Scenario: Unknown CLI resolves none

- **WHEN** `headless_args` is resolved for `my-agent` with no `[clis.my-agent].headless_args`
- **THEN** the result SHALL be none

#### Scenario: Empty override is rejected

- **GIVEN** `[clis.my-agent] headless_args = ""`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming `headless_args`
