## MODIFIED Requirements

### Requirement: Agent profiles are declared in `[profiles.<name>]`

The system SHALL accept an optional top-level `[profiles.<name>]` table. Each entry SHALL carry `kind` (`worker` or `delegate`), a one-line `description` and `cli` (a detected CLI id or `[clis.<name>]` key), and MAY carry `args`, `skill`, `approval` (`manual`, `accept-edits`, `auto` or `full-auto`) and, on a delegate, `timeout_seconds` (default 600). A delegate's approval SHALL default to `manual`. An unknown `approval` value SHALL fail the load with an error naming the profile, `approval` and all four accepted values. Whether `cli` and `skill` resolve SHALL be checked on use, not at load.

#### Scenario: A worker and a delegate profile load

- **GIVEN** `[profiles.routine]` with `kind = "worker"`, `description = "Routine implementation"`, `cli = "my-agent"`, `args = "--model small"`
- **AND** `[profiles.explorer]` with `kind = "delegate"`, `description = "Read-only codebase sweep"`, `cli = "my-agent"`, `skill = "explorer"`
- **WHEN** the config is loaded
- **THEN** both profiles SHALL be present with those values
- **AND** `explorer`'s effective approval SHALL be `manual` and its effective timeout 600 seconds

#### Scenario: A profile accepts accept-edits

- **GIVEN** `[profiles.careful]` with `kind = "worker"`, `description = "Edits only"`, `cli = "claude"`, `approval = "accept-edits"`
- **WHEN** the config is loaded and a worker is launched with `careful`
- **THEN** its pane command SHALL contain `--permission-mode acceptEdits`

### Requirement: A delegate profile's args cannot widen its permissions

Configuration loading SHALL fail with an error naming the profile and `args` when a delegate profile's `args`, quotes stripped, carry a word its CLI's delegate denylist names: a flag of the CLI's `accept-edits`, `auto` or `full-auto` approval flags, a sandbox-disabling flag or a family-denied flag, in any form; a config override of a denied key; or a permission-bypass value, compared case-insensitively. The check is best-effort defence in depth: profile `args` are operator-trusted configuration.

#### Scenario: A delegate's args cannot widen its permissions

- **GIVEN** a delegate profile on each family with `args` naming a denied flag — Claude `--permission-mode bypassPermissions`, Codex `-c sandbox_mode=x`, `-sdanger-full-access` or `--ask-for-approval=never`, Qwen `--approval-mode yolo`
- **WHEN** each config is loaded
- **THEN** loading SHALL fail with an error naming `profiles.<name>` and `args`
- **AND** ordinary arguments such as `--model small` SHALL still load

#### Scenario: Each family's denylist covers its permission, sandbox and tool-grant flags

- **WHEN** the delegate denylist is checked per family
- **THEN** it SHALL deny, whatever the value and in `flag value`, `flag=value` or attached single-dash form: Codex `--sandbox`/`-s`, `--full-auto`, `--yolo`, `-a`/`--ask-for-approval` and `-c` overrides of `sandbox_mode`, `approval_policy` and `sandbox_workspace_write.*`; Claude `--permission-mode`, `--allowedTools`/`--allowed-tools`, `--settings`, `--dangerously-skip-permissions`, `--allow-dangerously-skip-permissions`; Qwen `-y`, `--yolo`, `--approval-mode`
- **AND** it SHALL deny every flag of the CLI's `accept-edits`, `auto` and `full-auto` approval flags, an `approval_args` override included

#### Scenario: An accept-edits override flag is denied in a delegate's args

- **GIVEN** `[clis.mine] command = "mine"`, `approval_args = { "accept-edits" = "--edits-ok" }` and a delegate profile on `mine` with `args = "--edits-ok"`
- **WHEN** the config is loaded
- **THEN** loading SHALL fail with an error naming the profile and `args`

#### Scenario: Profile args are operator-trusted configuration

- **GIVEN** a profile's `args`, which the operator writes like `[clis.<name>].args` and whose `config.toml` edits are ask-protected by `git paw setup-cli`
- **WHEN** the denylist does not name a word in them
- **THEN** the profile SHALL load, the denylist being defence in depth and not a complete boundary

### Requirement: Delegate runs are admitted four readers or one writer at a time

Per repository, at most four delegate runs at level `manual` SHALL run at once, and a run at any level above `manual` (`accept-edits` or `auto`) SHALL run only when no other delegate run is active and SHALL exclude every other run while active. A run that cannot be admitted SHALL be refused immediately, never queued, with an error naming the active runs' profiles. Admission SHALL be a pure function of the active runs and the requested level.

#### Scenario: A fifth read-only delegate is refused

- **GIVEN** four active `manual` delegate runs
- **WHEN** a fifth `manual` delegate is invoked
- **THEN** it SHALL be refused immediately with an error naming the four active profiles

#### Scenario: A writing delegate runs alone

- **GIVEN** one active `manual` delegate run
- **WHEN** an `auto` delegate is invoked
- **THEN** it SHALL be refused
- **AND** while an `auto` delegate run is active, any further delegate SHALL be refused

#### Scenario: An accept-edits delegate is a writer

- **GIVEN** one active `accept-edits` delegate run
- **WHEN** a `manual` delegate is invoked
- **THEN** it SHALL be refused with an error naming the active profile

#### Scenario: A dead run does not block admission

- **GIVEN** a recorded active run whose process has exited without cleaning up
- **WHEN** admission is decided for a new run
- **THEN** the dead run SHALL NOT count toward the limit
