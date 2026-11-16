## ADDED Requirements

### Requirement: Doctor reports what each approval level launches

`git paw doctor` SHALL include a check named `approval levels`. It SHALL cover every CLI a launch of the current configuration can use: the configured default, spec-driven and preset worker CLIs, the CLI of every worker and delegate profile, and the orchestrator CLI. For each CLI it SHALL list:

- the CLI's provider family;
- the four levels `manual`, `accept-edits`, `auto` and `full-auto`, each with the exact flag string it launches, or `no flags` when the string is empty;
- which levels come from a `[clis.<id>].approval_args` override rather than the family's built-in row;
- which levels are in effect, and for whom: the worker default, each profile by name, and the orchestrator.

The check SHALL be built from the same resolution the launch builder uses. When a level in effect other than `manual` launches no flags, the check SHALL report ⚠, naming the CLI and the level, saying that its panes run in the CLI's default permission mode, and giving the remedy `[clis.<id>].approval_args`. Otherwise it SHALL report ✓. It SHALL never report ✗. When a Claude-family CLI is in effect at `accept-edits`, the detail SHALL add one line saying that `auto` launches the CLI's own auto mode and naming its flags. The check SHALL be read-only and SHALL NOT invoke any CLI.

#### Scenario: Every level is listed with its flags

- **GIVEN** `default_cli = "claude"` and no worker approval
- **WHEN** `git paw doctor` runs
- **THEN** the `approval levels` check SHALL list for `claude`: `manual` no flags, `accept-edits` `--permission-mode acceptEdits` marked in effect for workers, `auto` `--permission-mode auto`, and `full-auto` `--dangerously-skip-permissions`
- **AND** its detail SHALL say that `auto` launches the CLI's own auto mode
- **AND** it SHALL report ✓

#### Scenario: An override is marked

- **GIVEN** `[clis.claude] approval_args = { "auto" = "--permission-mode auto --model opus" }`
- **WHEN** `git paw doctor` runs
- **THEN** the `auto` row for `claude` SHALL show `--permission-mode auto --model opus` tagged `[clis.claude].approval_args`

#### Scenario: A level in effect with no flags warns

- **GIVEN** a version-2 config with `[workers] approval = "auto"` and `default_cli = "qwen"`
- **WHEN** `git paw doctor` runs
- **THEN** the check SHALL report ⚠ naming `qwen` and `auto`, with a remedy naming `[clis.qwen].approval_args`
- **AND** the exit code SHALL be the warning exit code, not the failure exit code

#### Scenario: Profiles and the orchestrator are marked

- **GIVEN** a worker default of `accept-edits`, `[profiles.deep] kind = "worker"`, `cli = "claude"`, `approval = "auto"`, and an orchestrator `cli = "claude"` with `approval = "full-auto"`
- **WHEN** `git paw doctor` runs
- **THEN** the `claude` rows SHALL mark `accept-edits` for workers, `auto` for profile `deep`, and `full-auto` for the orchestrator

#### Scenario: The check appears in JSON output

- **WHEN** `git paw doctor --json` is run
- **THEN** the output SHALL contain a check named `approval levels` with its status and, per CLI, each level's flags, source and in-effect marks
