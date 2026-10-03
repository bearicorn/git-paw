## MODIFIED Requirements

### Requirement: --unattended flag

The `start` subcommand SHALL accept an `--unattended` flag (boolean, default `false`). When passed, the parsed `StartArgs` SHALL have `unattended: bool` set to `true`. When omitted, `unattended` SHALL be `false`.

The flag SHALL appear in `git paw start --help` output with a description naming the use case: drive a supervisor wave to completion in-process (escalate every permission prompt for a human decision without blocking the rest of the wave, detect completion, exit with a summary). The help text SHALL NOT claim that the flag approves prompts, and its long help SHALL state that prompt-free unattended runs need the agent CLI's own allow rules or the `full-auto` approval level inside a sandbox.

`--unattended` engages supervisor mode: when `--unattended` is passed, the supervisor-mode resolution chain SHALL resolve supervisor mode active (equivalent to `--supervisor`) so the dispatch routes to the supervisor launch path. `--unattended` MAY be combined with `--supervisor`, `--from-all-specs` (and its alias `--from-specs`), `--specs`, `--cli`, and `--branches`.

`--unattended` SHALL NOT be combined with `--no-supervisor`; the system SHALL reject any invocation that combines `--unattended` with `--no-supervisor` with a parse error naming both flags, because they express opposing intents.

#### Scenario: --unattended sets the flag

- **GIVEN** the user invokes `git paw start --unattended`
- **WHEN** the CLI is parsed
- **THEN** the parsed `StartArgs.unattended` SHALL be `true`

#### Scenario: --unattended absent leaves flag false

- **GIVEN** the user invokes `git paw start` without `--unattended`
- **WHEN** the CLI is parsed
- **THEN** `StartArgs.unattended` SHALL be `false`

#### Scenario: --unattended combines with other flags

- **GIVEN** `start --unattended --from-specs --cli claude`
- **WHEN** the CLI is parsed
- **THEN** `unattended` SHALL be `true`
- **AND** the spec-mode launch-all state SHALL be set
- **AND** `cli` SHALL be `Some("claude")`
- **AND** parsing SHALL succeed

#### Scenario: --unattended appears in help output

- **WHEN** `git paw start --help` is run
- **THEN** the output SHALL contain `--unattended`
- **AND** the output SHALL describe the flag as driving the wave to completion and escalating permission prompts
- **AND** the output SHALL NOT describe the flag as auto-approving prompts

#### Scenario: --unattended engages supervisor mode

- **GIVEN** `git paw start --unattended --branches feat/a,feat/b` is invoked
- **WHEN** the dispatch resolves the supervisor-mode resolution chain
- **THEN** supervisor mode SHALL resolve active
- **AND** the dispatch SHALL route to the supervisor launch path (`cmd_supervisor`)

#### Scenario: --unattended with --no-supervisor is rejected

- **GIVEN** `start --unattended --no-supervisor`
- **WHEN** the CLI is parsed
- **THEN** parsing SHALL fail with an error mentioning both `--unattended` and `--no-supervisor`
