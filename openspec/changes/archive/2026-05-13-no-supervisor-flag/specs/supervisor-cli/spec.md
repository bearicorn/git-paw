## MODIFIED Requirements

### Requirement: Supervisor mode resolution chain

The system SHALL determine whether to enter supervisor mode using the following resolution chain, evaluated in order:

1. If `--no-supervisor` flag is present → disable supervisor mode (no prompt, regardless of any other input)
2. If `--supervisor` flag is present → enable supervisor mode (no prompt)
3. If `[supervisor] enabled = true` in config → enable supervisor mode (no prompt)
4. If `[supervisor] enabled = false` in config → disable supervisor mode (no prompt)
5. If `[supervisor]` section is absent (`None`) → prompt "Start in supervisor mode? (y/n)"
6. If `--dry-run` is present and step 5 would apply → assume no supervisor (skip prompt)

`--no-supervisor` and `--supervisor` SHALL be mutually exclusive at parse time (per the `cli-parsing` requirement); the resolver therefore never sees both flags `true` simultaneously.

When supervisor mode is enabled (steps 2 or 3), the system SHALL call `cmd_supervisor()`. When disabled (steps 1, 4, or 6), the system SHALL proceed with normal `cmd_start()`.

#### Scenario: --no-supervisor disables regardless of config (config enabled)

- **GIVEN** a config with `[supervisor] enabled = true`
- **WHEN** `git paw start --no-supervisor` is run
- **THEN** supervisor mode SHALL NOT be entered
- **AND** `cmd_supervisor()` SHALL NOT be called
- **AND** no interactive prompt SHALL be shown

#### Scenario: --no-supervisor with no config section also disables

- **GIVEN** a config with no `[supervisor]` section
- **WHEN** `git paw start --no-supervisor` is run
- **THEN** supervisor mode SHALL NOT be entered
- **AND** no interactive prompt SHALL be shown

#### Scenario: --no-supervisor with --dry-run also disables

- **GIVEN** any config state
- **WHEN** `git paw start --no-supervisor --dry-run` is run
- **THEN** supervisor mode SHALL NOT be entered
- **AND** the dry-run plan SHALL reflect supervisor-disabled state

#### Scenario: --supervisor flag enables regardless of config

- **GIVEN** a config with `[supervisor] enabled = false`
- **WHEN** `git paw start --supervisor` is run
- **THEN** supervisor mode SHALL be enabled
- **AND** `cmd_supervisor()` SHALL be called

#### Scenario: Config enabled = true enables without prompt

- **GIVEN** a config with `[supervisor] enabled = true`
- **WHEN** `git paw start` is run with no flags
- **THEN** supervisor mode SHALL be enabled without any interactive prompt

#### Scenario: Config enabled = false disables without prompt

- **GIVEN** a config with `[supervisor] enabled = false`
- **WHEN** `git paw start` is run with no flags
- **THEN** supervisor mode SHALL NOT be entered
- **AND** no interactive prompt SHALL be shown

#### Scenario: No supervisor section prompts the user

- **GIVEN** a config with no `[supervisor]` section
- **WHEN** `git paw start` is run with no flags
- **THEN** the system SHALL prompt "Start in supervisor mode?"

#### Scenario: dry-run skips supervisor prompt

- **GIVEN** a config with no `[supervisor]` section
- **WHEN** `git paw start --dry-run` is run
- **THEN** no interactive prompt SHALL be shown
- **AND** supervisor mode SHALL NOT be entered
